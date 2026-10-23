//! Company settings › Release management › Release conditions (Help:
//! "Selected conditions will prevent a Release from being created"): which
//! hold for each item of a Release candidate. Checked when the dialog opens
//! and on Refresh, since they regenerate Part Studios and evaluate
//! Assemblies.
//!
//! "A part number is included in another pending Release candidate" and "An
//! item has a pending task" never hold here: releases are made at once on
//! this computer (no Submit / approval, so no pending candidates) and there
//! are no tasks.

use crate::doc_model::DocModel;
use noshape_model::display::FeatureStatus;
use noshape_model::document::{TabContent, TabId};
use noshape_model::reference::BodyId;
use noshape_model::release::{Issue, PackageKind, ReleaseItem, ReleaseLog};
use std::collections::HashMap;

/// The conditions, as `release_settings::CONDITIONS` lists them.
const FEATURE_ERRORS: usize = 0;
const ROLLED_BACK: usize = 1;
const ASSEMBLY_ERRORS: usize = 2;
const PENDING_REFERENCES: usize = 3;
const DRAWING_UPDATE: usize = 4;
const OBSOLETE_REFERENCES: usize = 7;

/// The conditions that hold for each item.
pub fn condition_issues(doc: &DocModel, items: &[ReleaseItem]) -> Vec<Vec<Issue>> {
    let on = crate::release_dialog::release_settings().conditions;
    let mut studio_errors: HashMap<(TabId, noshape_model::configuration::ConfigValues), bool> = HashMap::new();
    items
        .iter()
        .map(|it| {
            let mut v = Vec::new();
            if it.document.is_some() || it.reverted {
                return v;
            }
            let prevented = |v: &mut Vec<Issue>, text: &str| v.push(Issue::Prevented(text.into()));
            match doc.session.doc.tab(it.tab).map(|t| &t.content) {
                Some(TabContent::PartStudio(ps)) => {
                    if on[FEATURE_ERRORS] {
                        let failed = *studio_errors.entry((it.tab, it.config.clone())).or_insert_with(|| {
                            let mut values = ps.configuration.defaults();
                            values.0.extend(it.config.0.clone());
                            let result = noshape_model::regen::Regenerator::new().regenerate(ps, &values, &doc.session.doc.settings.units, &noshape_model::regen::RegenOptions::default());
                            result.display.features.iter().any(|f| f.status == FeatureStatus::Error)
                        });
                        if failed {
                            prevented(&mut v, "The Part Studio feature list contains errors.");
                        }
                    }
                    if on[ROLLED_BACK] && ps.rollback.is_some_and(|r| r < ps.features.len()) {
                        prevented(&mut v, "The Part Studio feature list is not rolled to the end.");
                    }
                }
                Some(TabContent::Assembly(asm)) => {
                    if on[ASSEMBLY_ERRORS] {
                        let mut asm = asm.clone();
                        if asm.has_configured() {
                            asm.apply_configuration(&it.config);
                        }
                        let result = noshape_model::assembly::eval::AssemblyEvaluator::new().evaluate(&doc.session.doc, &asm, &Default::default(), None);
                        if result.has_errors() {
                            prevented(&mut v, "The Assembly instance list contains errors.");
                        }
                    }
                    let own = &doc.session.path;
                    let pinned: Vec<(crate::derived::ExternalRef, Option<BodyId>)> = asm
                        .instances
                        .iter()
                        .filter_map(|i| {
                            let r = crate::derived::ExternalRef::parse(i.source.external_key()?)?;
                            let body = match &i.source {
                                noshape_model::assembly::InstanceSource::External { body, .. } => body.clone(),
                                _ => None,
                            };
                            Some((r, body))
                        })
                        .collect();
                    // "Pending reference updates": a reference to another
                    // document's version with a newer one out (the update icons).
                    if on[PENDING_REFERENCES] && pinned.iter().any(|(r, _)| r.path != *own && crate::derived::newer_version(r).is_some()) {
                        prevented(&mut v, "The Assembly has pending reference updates.");
                    }
                    if on[OBSOLETE_REFERENCES] && pinned.iter().any(|(r, body)| references_obsolete(&r.path, r.microversion, &r.tab, body.as_ref())) {
                        prevented(&mut v, "The item references obsolete revisions.");
                    }
                }
                Some(TabContent::Drawing(drawing)) => {
                    // The update icon's state: a source changed since the views were made.
                    if on[DRAWING_UPDATE] && noshape_ops::drawing::old_state(&doc.session, drawing).is_some() {
                        prevented(&mut v, "The Drawing has a pending update.");
                    }
                    if on[OBSOLETE_REFERENCES] {
                        let own = &doc.session.path;
                        let obsolete = noshape_model::drawing::views::referenced_sources(drawing).iter().any(|k| {
                            let Some(version) = &k.version else { return false };
                            match crate::derived::ExternalRef::parse(version) {
                                Some(r) => references_obsolete(&r.path, r.microversion, &r.tab, None),
                                None => doc.session.store.list_versions().ok().and_then(|vs| vs.into_iter().find(|v| v.id.to_string() == *version)).is_some_and(|v| references_obsolete(own, v.microversion, &k.tab.to_string(), None)),
                            }
                        });
                        if obsolete {
                            prevented(&mut v, "The item references obsolete revisions.");
                        }
                    }
                }
                _ => {}
            }
            v
        })
        .collect()
}

/// Whether a reference pinned to a document version (`path` at `mv`, the
/// element `tab`, one part `body` or all) points at a release whose revision
/// is obsolete now.
fn references_obsolete(path: &std::path::Path, mv: noshape_store::MicroversionId, tab: &str, body: Option<&BodyId>) -> bool {
    let Ok(store) = noshape_store::Document::open_read_only(path) else { return false };
    let Some(version) = store.list_versions().ok().and_then(|vs| vs.into_iter().find(|v| v.microversion == mv)) else { return false };
    let mut log = store.get_meta(crate::release_dialog::META_KEY).ok().flatten().map(|b| ReleaseLog::from_json(&b)).unwrap_or_default();
    log.auto_obsolete = crate::release_dialog::release_settings().auto_obsolete;
    released_obsolete(&log, &version.id.to_string(), tab, body)
}

/// Whether a release at `version` of the element `tab` (one part `body`, or
/// any) has a revision that is obsolete now.
fn released_obsolete(log: &ReleaseLog, version: &str, tab: &str, body: Option<&BodyId>) -> bool {
    log.at_version(version)
        .into_iter()
        .filter(|p| p.kind == PackageKind::Release)
        .flat_map(|p| &p.items)
        .filter(|i| i.tab.to_string() == tab && body.is_none_or(|b| i.part.as_ref() == Some(b)) && !i.reverted && !i.not_revision_managed)
        .any(|i| log.revisions(&i.part_number).iter().any(|e| e.revision == i.revision && (e.obsoleted_by.is_some() || e.auto_obsolete)))
}

#[cfg(test)]
mod tests {
    use noshape_model::release::{ItemKind, Package, PackageKind, ReleaseItem, ReleaseLog};

    #[test]
    fn a_reference_to_an_obsoleted_release_is_obsolete() {
        let tab = noshape_model::document::TabId(3);
        let item = |rev: &str| ReleaseItem { tab, part: None, name: "Assembly 1".into(), kind: ItemKind::Assembly, part_number: "ASM-1".into(), revision: rev.into(), not_revision_managed: false, reverted: false, config: Default::default(), document: None };
        let pkg = |kind, version: Option<&str>, items| Package { id: 0, kind, name: "r".into(), notes: String::new(), created: 0, version: version.map(String::from), items, re_releasable: false };
        let mut log = ReleaseLog::default();
        log.record(pkg(PackageKind::Release, Some("v1"), vec![item("A")]));
        let t = tab.to_string();
        assert!(!super::released_obsolete(&log, "v1", &t, None));
        log.record(pkg(PackageKind::Obsoletion, None, vec![item("A")]));
        assert!(super::released_obsolete(&log, "v1", &t, None));
        assert!(!super::released_obsolete(&log, "v2", &t, None), "another version");
    }
}
