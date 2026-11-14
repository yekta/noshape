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
            description: "Release… (a tab's menu, or a part's; only with Company settings' managed workflows on, see company_release_settings): the Release candidate dialog with its items, then Release. Items as the dialog lists them: a Part Studio's parts (or `parts` only), an Assembly with its Bill of Materials, a Drawing with what its views show, a Variable Studio or file itself; `include_drawings` adds the drawings showing them. Each item's part number is its own (Properties) unless `part_numbers` gives one ({item name or part id: number}); its revision is the next of the revision scheme (A, B, …; `revisions` overrides, {item name or part id: revision}). The dialog's checks refuse it with their messages (a part number missing, a revision too low, Company settings' release conditions). Releasing writes the part numbers onto the parts (\"Release <name>\"), records the release at a new version named `name`; with Company settings' automatic obsoletion on, a new revision obsoletes the part number's previous one.",
            fields: vec![
                doc_field(),
                opt("tab", s(), "The tab to release (name or id). Required unless `from_draft` gives the items."),
                opt("tabs", strings(), "More tabs whose items join the candidate (the dialog's + Add items)."),
                opt("from_draft", s(), "Start from this saved draft's items, name and notes (Release drafts › Review Release); the other fields change them."),
                opt("draft", json!({ "type": "boolean" }), "Save draft instead of releasing: the candidate is kept in the document by its name (no checks but the name)."),
                opt("generate_part_numbers", json!({ "type": "boolean" }), "Generate part numbers for the items without one (Company settings' sequential numbering scheme)."),
                opt("revert", strings(), "Items (names or part ids) reverted to their last released revision (the State column's Revert)."),
                opt("parts", strings(), "Part Studio: only these parts (ids or names)."),
                opt("instance", s(), "Assembly: only this instance's (its Release…): its part, another document's part at its version (linked to that version), or a subassembly's contents."),
                opt("name", s(), "Release name (the version's name); required unless `from_draft` names it."),
                opt("notes", s(), "Release notes (the version's description)."),
                opt("part_numbers", json!({ "type": "object" }), "{item name or part id: part number}."),
                opt("revisions", json!({ "type": "object" }), "{item name or part id: revision}."),
                opt("include_drawings", json!({ "type": "boolean" }), "Include associated drawings (those made from another workspace's state are left out and listed as `drawings_left_out`)."),
                opt("approvers", strings(), "Approvers: Submit instead of Release. The candidate's version is made now and it waits, Pending, for release_approve or release_reject. The only person offered is the local user (Preferences' name; no user directory in a local app)."),
                opt("observers", strings(), "Observers (the local user)."),
            ],
            mutates: true,
            destructive: false,
            run: create,
        },
        Op {
            group: "release",
            verb: "approve",
            summary: "Approve a Pending release candidate",
            description: "A submitted candidate's approver › Approve (from Release drafts): it is released at the version made when it was submitted; its items' states become Released.",
            fields: vec![doc_field(), req("name", s(), "The candidate's release name.")],
            mutates: true,
            destructive: false,
            run: approve,
        },
        Op {
            group: "release",
            verb: "reject",
            summary: "Reject a Pending release candidate",
            description: "A submitted candidate's approver › Reject: the candidate stays, Rejected; its items stay as they were (release them with another candidate).",
            fields: vec![doc_field(), req("name", s(), "The candidate's release name.")],
            mutates: true,
            destructive: false,
            run: reject,
        },
        Op {
            group: "release",
            verb: "discard",
            summary: "Discard a Pending or Rejected release candidate",
            description: "Discard Release candidate: \"The Release candidate will be discarded and all items will be returned to their initial states.\" It stays in Release drafts, Discarded; its part numbers are no longer pending.",
            fields: vec![doc_field(), req("name", s(), "The candidate's release name.")],
            mutates: true,
            destructive: true,
            run: discard,
        },
        Op {
            group: "release",
            verb: "comment",
            summary: "Comment on a release draft, candidate or release",
            description: "The Release dialog's Comments: `text` added by the local user (kept with the document; no notifications).",
            fields: vec![doc_field(), req("name", s(), "The draft's, candidate's or release's name."), req("text", s(), "The comment.")],
            mutates: true,
            destructive: false,
            run: comment,
        },
        Op {
            group: "release",
            verb: "drafts",
            summary: "The saved Release candidate drafts",
            description: "Release drafts (the document's panel): each saved candidate's name, notes, time and items, then the submitted candidates with their state (Pending, Rejected), approvers and observers. Reopen a draft with release_create `from_draft`, delete it with release_draft_delete; approve or reject a Pending one with release_approve / release_reject.",
            fields: vec![doc_field()],
            mutates: false,
            destructive: false,
            run: drafts,
        },
        Op {
            group: "release",
            verb: "draft_delete",
            summary: "Delete a saved Release candidate draft",
            description: "Review Release › Save draft ▾ › Delete draft: the draft named `name` is removed.",
            fields: vec![doc_field(), req("name", s(), "The draft's name.")],
            mutates: true,
            destructive: true,
            run: draft_delete,
        },
        Op {
            group: "release",
            verb: "obsolete",
            summary: "Obsolete a released revision of a part number",
            description: "Revision history › Obsolete revision: the Create Obsolete candidate dialog for that revision, then Obsolete. The revision (default: the newest) is recorded obsolete at a new version named `name` (default \"Obsolete <part number> <revision>\"); `notes` (Reason for obsoletion) is required. `re_releasable` keeps the revision free to be released again. A revision already obsoleted is refused.",
            fields: vec![
                doc_field(),
                req("part_number", s(), "The released part number."),
                opt("revision", s(), "The revision to obsolete (default: the newest)."),
                opt("name", s(), "Obsoletion name (the version's name)."),
                req("notes", s(), "Reason for obsoletion."),
                opt("re_releasable", json!({ "type": "boolean" }), "Mark revision as re-releasable."),
            ],
            mutates: true,
            destructive: false,
            run: obsolete,
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
    if let Some(d) = &i.document {
        v["linked"] = json!({ "document": d.name, "version": d.version_name });
    }
    v
}

/// An assembly instance's Release… items (as the app's instance menu makes them).
fn instance_items(od: &crate::OpenDoc, tab: noshape_model::document::TabId, key: &str) -> Result<Vec<noshape_model::release::ReleaseItem>, OpError> {
    use noshape_model::assembly::InstanceSource;
    let Some(TabContent::Assembly(asm)) = od.doc().tab(tab).map(|t| &t.content) else { return Err(OpError::param("instance", "`tab` must be an assembly")) };
    let id = super::assembly::find_instance(asm, key, "instance")?;
    let inst = asm.instance(id).expect("found");
    let name = inst.name.rsplit_once(" <").map_or(inst.name.as_str(), |(n, _)| n).to_string();
    Ok(match &inst.source {
        InstanceSource::Part { studio, body, config } => rel::part_items(&od.session, *studio, std::slice::from_ref(body), &[name], config),
        InstanceSource::External { key, body: Some(body), .. } => {
            let r = crate::derived::ExternalRef::parse(key).ok_or_else(|| OpError::param("instance", "its reference can't be read"))?;
            rel::linked_part_items(&od.session, &r, body, &name)
        }
        InstanceSource::Assembly { assembly, .. } => {
            let provider = crate::derived::DocStudios::new(od.doc().clone(), od.studio_cache.clone());
            rel::tab_items(&od.session, &*provider, *assembly)
        }
        _ => return Err(OpError::param("instance", "a part or subassembly instance")),
    })
}

fn create(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    workflows_on()?;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let provider = crate::derived::DocStudios::new(od.doc().clone(), od.studio_cache.clone());
    let draft = match a.opt_str("from_draft")? {
        Some(n) => Some(rel::release_log(&od.session).drafts.iter().find(|d| d.name == n).cloned().ok_or_else(|| OpError::param("from_draft", &format!("no draft `{n}`")))?),
        None => None,
    };
    let tab = match a.opt_str("tab")? {
        Some(t) => Some(crate::doc::find_tab(od.doc(), t)?),
        None if draft.is_some() => None,
        None => return Err(OpError::param("tab", "the tab to release (or `from_draft`)")),
    };
    let mut items = match (&draft, tab, a.opt_str("instance")?) {
        (_, Some(tab), Some(k)) => instance_items(od, tab, k)?,
        (_, Some(tab), None) => rel::tab_items(&od.session, &*provider, tab),
        (Some(d), None, _) => d.items.clone(),
        (None, None, _) => Vec::new(),
    };
    // A draft's items with the tab's: the tab's join them.
    if let (Some(d), Some(_)) = (&draft, tab) {
        let extra: Vec<_> = std::mem::take(&mut items).into_iter().filter(|i| !d.items.iter().any(|x| x.same_object(i))).collect();
        items = d.items.iter().cloned().chain(extra).collect();
    }
    for (i, t) in a.opt_array("tabs")?.into_iter().flatten().enumerate() {
        let id = crate::doc::find_tab(od.doc(), t.as_str().unwrap_or_default()).map_err(|_| OpError::param(&format!("tabs[{i}]"), "no such tab"))?;
        let extra: Vec<_> = rel::tab_items(&od.session, &*provider, id).into_iter().filter(|x| !items.iter().any(|i| i.same_object(x))).collect();
        items.extend(extra);
    }
    if items.is_empty() {
        return Err(OpError::param("tab", "nothing to release in this tab"));
    }
    if let Some(list) = a.opt_array("parts")? {
        let Some(tab) = tab else { return Err(OpError::param("parts", "parts of a Part Studio `tab`")) };
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
    let mut left_out = Vec::new();
    if a.bool_or("include_drawings", false)? {
        let (found, skipped) = rel::associated_drawings_split(&od.session, &*provider, &items);
        left_out = skipped;
        let extra: Vec<_> = found.into_iter().filter(|d| !items.iter().any(|i| i.same_object(d))).collect();
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
    // The header's "Generate missing part numbers".
    if a.bool_or("generate_part_numbers", false)? {
        for it in items.iter_mut().filter(|it| it.part_number.trim().is_empty()) {
            let ty = it.kind.prefix_index();
            let Some(n) = rel::next_number(|s| s.release_objects.generate(ty)) else {
                return Err(OpError::param("generate_part_numbers", "Company settings › Numbering schemes has no sequential part number scheme"));
            };
            if !it.not_revision_managed {
                it.revision = log.next_revision(&n);
            }
            it.part_number = n;
        }
    }
    for (i, v) in a.opt_array("revert")?.into_iter().flatten().enumerate() {
        let k = v.as_str().unwrap_or_default();
        let it = items.iter_mut().find(|it| it.part.as_ref().is_some_and(|b| crate::refs::body_id(b) == k) || it.name == k).ok_or_else(|| OpError::param(&format!("revert[{i}]"), &format!("no item `{k}`")))?;
        let Some(rev) = log.latest_revision(&it.part_number) else { return Err(OpError::param(&format!("revert[{i}]"), &format!("`{k}` has no released revision"))) };
        it.reverted = true;
        it.revision = rev;
    }
    let name = a.opt_str("name")?.map(str::to_string).or_else(|| draft.as_ref().map(|d| d.name.clone())).unwrap_or_default().trim().to_string();
    if name.is_empty() {
        return Err(OpError::param("name", "a release name"));
    }
    let notes = a.opt_str("notes")?.map(str::to_string).or_else(|| draft.as_ref().map(|d| d.notes.clone())).unwrap_or_default();
    let created = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let people = |field: &str| -> Result<Vec<String>, OpError> {
        let me = noshape_store::local_user_name();
        let mut out: Vec<String> = Vec::new();
        for (i, v) in a.opt_array(field)?.into_iter().flatten().enumerate() {
            let n = v.as_str().unwrap_or_default().trim().to_string();
            if n != me {
                return Err(OpError::param(&format!("{field}[{i}]"), &format!("only the local user can be chosen: `{me}`")));
            }
            if !out.contains(&n) {
                out.push(n);
            }
        }
        Ok(out)
    };
    let (mut approvers, mut observers) = (people("approvers")?, people("observers")?);
    // A reopened draft keeps its people and comments unless given.
    if let Some(d) = &draft {
        if a.opt_array("approvers")?.is_none() {
            approvers = d.approvers.clone();
        }
        if a.opt_array("observers")?.is_none() {
            observers = d.observers.clone();
        }
    }
    let comments = draft.as_ref().map(|d| d.comments.clone()).unwrap_or_default();
    // Save draft: kept by its name, no checks but the name (the dialog's).
    if a.bool_or("draft", false)? {
        let pkg = Package { id: 0, kind: PackageKind::Release, name: name.clone(), notes, created, version: None, items, re_releasable: false, approvers, observers, state: None, decided_by: None, comments };
        let mut log = rel::release_log(&od.session);
        log.save_draft(pkg.clone());
        if let Some(old) = draft.as_ref().filter(|d| d.name != name) {
            log.delete_draft(&old.name);
        }
        rel::save_log(&mut od.session, &log).map_err(|e| OpError::new("release", format!("{e:#}")))?;
        return Ok(json!({ "draft": name, "items": pkg.items.iter().map(item_json).collect::<Vec<_>>(), "drawings_left_out": left_out }).into());
    }
    // Company settings › Release management › "Require a note in the Release dialog".
    let settings = crate::release_settings::release_settings();
    if settings.require_note && notes.trim().is_empty() {
        return Err(OpError::param("notes", "Required field must not be empty."));
    }
    if settings.no_creator_approver && !approvers.is_empty() {
        return Err(OpError::param("approvers", "Company settings don't allow the creator as an approver"));
    }
    if settings.require_approver && approvers.is_empty() {
        return Err(OpError::param("approvers", "Company settings require an approver"));
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
    let submitting = !approvers.is_empty();
    let pkg = Package { id: 0, kind: PackageKind::Release, name, notes, created, version: None, items, re_releasable: false, approvers, observers, state: None, decided_by: None, comments };
    // With approvers: Submit (Pending at its version); else Release at once.
    let pkg = if submitting { rel::submit(&mut od.session, pkg) } else { rel::commit(&mut od.session, pkg) }.map_err(|e| OpError::new("release", format!("{e:#}")))?;
    let state = pkg.state.map_or("Released", |s| s.label());
    Ok(json!({ "release": pkg.name, "state": state, "version": pkg.version, "items": pkg.items.iter().map(item_json).collect::<Vec<_>>(), "drawings_left_out": left_out }).into())
}

fn approve(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let name = a.str("name")?;
    if rel::release_log(&od.session).candidate(name).is_none_or(|c| c.state != Some(noshape_model::release::CandidateState::Pending)) {
        return Err(OpError::param("name", &format!("no pending release candidate `{name}`")));
    }
    let pkg = rel::approve(&mut od.session, name).map_err(|e| OpError::new("release", format!("{e:#}")))?;
    Ok(json!({ "release": pkg.name, "state": "Released", "approved_by": pkg.decided_by, "version": pkg.version, "items": pkg.items.iter().map(item_json).collect::<Vec<_>>() }).into())
}

fn discard(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let name = a.str("name")?;
    rel::discard(&mut od.session, name).map_err(|_| OpError::param("name", &format!("no pending or rejected release candidate `{name}`")))?;
    Ok(json!({ "release": name, "state": "Discarded" }).into())
}

fn comment(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let (name, text) = (a.str("name")?, a.str("text")?.trim().to_string());
    if text.is_empty() {
        return Err(OpError::param("text", "a comment"));
    }
    rel::comment(&mut od.session, name, &text).map_err(|_| OpError::param("name", &format!("no release, draft or candidate `{name}`")))?;
    Ok(json!({ "release": name, "comment": text }).into())
}

fn reject(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let name = a.str("name")?;
    rel::reject(&mut od.session, name).map_err(|_| OpError::param("name", &format!("no pending release candidate `{name}`")))?;
    Ok(json!({ "release": name, "state": "Rejected", "rejected_by": noshape_store::local_user_name() }).into())
}

fn drafts(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let log = rel::release_log(&od.session);
    let drafts: Vec<Value> = log.drafts.iter().map(|d| json!({ "name": d.name, "notes": d.notes, "created": d.created, "items": d.items.iter().map(item_json).collect::<Vec<_>>() })).collect();
    let comments = |p: &Package| p.comments.iter().map(|c| json!({ "author": c.author, "created": c.created, "text": c.text })).collect::<Vec<_>>();
    let candidates: Vec<Value> = log.candidates.iter().map(|c| json!({ "name": c.name, "state": c.state.map_or("Pending", |s| s.label()), "approvers": c.approvers, "observers": c.observers, "decided_by": c.decided_by, "notes": c.notes, "created": c.created, "comments": comments(c), "items": c.items.iter().map(item_json).collect::<Vec<_>>() })).collect();
    Ok(json!({ "drafts": drafts, "candidates": candidates }).into())
}

fn draft_delete(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let name = a.str("name")?;
    let mut log = rel::release_log(&od.session);
    if !log.drafts.iter().any(|d| d.name == name) {
        return Err(OpError::param("name", &format!("no draft `{name}`; drafts: {}", log.drafts.iter().map(|d| d.name.clone()).collect::<Vec<_>>().join(", "))));
    }
    log.delete_draft(name);
    rel::save_log(&mut od.session, &log).map_err(|e| OpError::new("release", format!("{e:#}")))?;
    Ok(json!({ "deleted": name }).into())
}

/// Release… exists only with Company settings' managed workflows on (live2 2026-11-14).
fn workflows_on() -> Result<(), OpError> {
    if crate::release_settings::managed_workflows() {
        return Ok(());
    }
    Err(OpError::invalid("Release management is off: turn on Enable managed workflows in Company settings › Release management (company_release_settings)"))
}

fn obsolete(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    workflows_on()?;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let pn = a.str("part_number")?;
    let log = rel::release_log(&od.session);
    let revs = log.revisions(pn);
    let Some(newest) = revs.last() else { return Err(OpError::param("part_number", "no released revision of this part number")) };
    let entry = match a.opt_str("revision")? {
        Some(r) => revs.iter().find(|e| e.revision == r).ok_or_else(|| OpError::param("revision", &format!("{pn} has no released revision {r}")))?,
        None => newest,
    };
    if entry.obsoleted_by.is_some() {
        return Err(OpError::param("revision", &format!("{pn} revision {} is already obsolete", entry.revision)));
    }
    let notes = a.str("notes")?.trim().to_string();
    if notes.is_empty() {
        return Err(OpError::param("notes", "Required field must not be empty."));
    }
    let Some(item) = log.packages[entry.release].items.iter().find(|i| i.part_number == pn).cloned() else { return Err(OpError::param("part_number", "its release lists no such item")) };
    let item = noshape_model::release::ReleaseItem { revision: entry.revision.clone(), ..item };
    let name = a.opt_str("name")?.map_or_else(|| format!("Obsolete {pn} {}", item.revision), str::to_string);
    let created = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let re_releasable = a.bool_or("re_releasable", false)?;
    let pkg = Package { id: 0, kind: PackageKind::Obsoletion, name, notes, created, version: None, items: vec![item], re_releasable, approvers: Vec::new(), observers: Vec::new(), state: None, decided_by: None, comments: Vec::new() };
    let pkg = rel::commit(&mut od.session, pkg).map_err(|e| OpError::new("release", format!("{e:#}")))?;
    Ok(json!({ "obsoletion": pkg.name, "version": pkg.version, "re_releasable": pkg.re_releasable, "items": pkg.items.iter().map(item_json).collect::<Vec<_>>() }).into())
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
