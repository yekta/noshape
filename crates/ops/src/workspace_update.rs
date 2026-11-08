//! The Reference manager's Update to latest: references of this workspace to other documents' versions that have a
//! newer version (Derived features, inserted Variable Studios), updated together. Shared by the app's dialog
//! (crates/app/src/doc_dialogs.rs, opened from a tab's "Newer versions of linked documents available" icon) and the
//! agent API (document_update_references).

use crate::derived::{self, ExternalRef, VersionChoice};
use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::reference::FeatureId;

/// One outdated reference: what it is, its version now and the newest.
#[derive(Clone, Debug)]
pub struct Outdated {
    pub label: String,
    pub from: String,
    pub to: VersionChoice,
    pub kind: OutdatedKind,
}

#[derive(Clone, Debug)]
pub enum OutdatedKind {
    /// A Derived feature (tab, feature id) pinned to `ExternalRef`.
    Derived(TabId, FeatureId, ExternalRef),
    /// A Variable Studio table from another document (element / studio tab, studio id).
    Variables(TabId, TabId),
}

/// The history text of [`apply`].
pub const DESCRIPTION: &str = "Update references";

/// ☰ › Update workspace…'s toast: the workspace's feature library is the one noshape ships (live's words).
pub const LATEST_MESSAGE: &str = "Workspace is already at the latest feature library version.";

/// References of this workspace to other documents' versions with a newer
/// version available.
pub fn outdated(doc: &Document) -> Vec<Outdated> {
    use noshape_model::variables::VariableStudioRefs;
    let mut out = Vec::new();
    for t in &doc.tabs {
        let mut studio_refs = |owner: TabId, refs: &[noshape_model::variables::StudioRef]| {
            for r in refs {
                let Some(e) = &r.external else { continue };
                let Ok(mv) = e.microversion.parse() else { continue };
                let x = ExternalRef { path: e.path.clone(), microversion: mv, tab: String::new() };
                if let Some(v) = derived::newer_version(&x) {
                    let doc_name = derived::external_doc_info(&e.path).map(|i| i.name).unwrap_or_default();
                    out.push(Outdated { label: format!("{doc_name} (Variable Studio) in {}", t.name), from: e.version.clone(), to: v, kind: OutdatedKind::Variables(owner, r.studio) });
                }
            }
        };
        match &t.content {
            TabContent::PartStudio(ps) => {
                let VariableStudioRefs { inserted, .. } = &ps.variable_studios;
                studio_refs(t.id, inserted);
                for f in ps.features.iter().filter(|f| f.type_id == "importDerived") {
                    let Some(x) = ExternalRef::parse(f.params.text("partStudio")) else { continue };
                    if let Some(v) = derived::newer_version(&x) {
                        out.push(Outdated { label: format!("{} in {}", derived::describe_external(&x), t.name), from: String::new(), to: v, kind: OutdatedKind::Derived(t.id, f.id, x) });
                    }
                }
            }
            TabContent::Assembly(a) => studio_refs(t.id, &a.variable_studios.inserted),
            TabContent::VariableStudio(v) => studio_refs(t.id, &v.inserted),
            _ => {}
        }
    }
    out
}

/// Point one outdated reference at its newest version.
pub fn apply(o: &Outdated, doc: &mut Document) {
    match &o.kind {
        OutdatedKind::Derived(tab, fid, x) => {
            if let Some(TabContent::PartStudio(ps)) = doc.tab_mut(*tab).map(|t| &mut t.content)
                && let Some(f) = ps.feature_mut(*fid)
            {
                let key = ExternalRef { microversion: o.to.microversion, ..x.clone() }.key();
                f.params.set("partStudio", noshape_model::params::ParamValue::Text(key));
            }
        }
        OutdatedKind::Variables(owner, studio) => {
            let refs: Option<&mut Vec<noshape_model::variables::StudioRef>> = match doc.tab_mut(*owner).map(|t| &mut t.content) {
                Some(TabContent::PartStudio(ps)) => Some(&mut ps.variable_studios.inserted),
                Some(TabContent::Assembly(a)) => Some(&mut a.variable_studios.inserted),
                Some(TabContent::VariableStudio(v)) => Some(&mut v.inserted),
                _ => None,
            };
            if let Some(r) = refs.and_then(|refs| refs.iter_mut().find(|r| r.studio == *studio))
                && let Some(e) = &mut r.external
            {
                e.microversion = o.to.microversion.to_string();
                e.version = o.to.label.clone();
            }
        }
    }
}

/// Each tab's references to other documents' versions: Derived features,
/// inserted Variable Studios, Assembly instances and Drawing views (the tab
/// bar's "Newer versions of linked documents available" icon).
pub fn version_refs(doc: &Document) -> Vec<(TabId, ExternalRef)> {
    let mut out = Vec::new();
    for t in &doc.tabs {
        let mut studios = |refs: &[noshape_model::variables::StudioRef]| {
            for r in refs {
                let Some(e) = &r.external else { continue };
                let Ok(microversion) = e.microversion.parse() else { continue };
                out.push((t.id, ExternalRef { path: e.path.clone(), microversion, tab: String::new() }));
            }
        };
        match &t.content {
            TabContent::PartStudio(ps) => {
                studios(&ps.variable_studios.inserted);
                out.extend(ps.features.iter().filter(|f| f.type_id == "importDerived").filter_map(|f| ExternalRef::parse(f.params.text("partStudio"))).map(|x| (t.id, x)));
            }
            TabContent::Assembly(a) => {
                studios(&a.variable_studios.inserted);
                out.extend(a.instances.iter().filter_map(|i| ExternalRef::parse(i.source.external_key()?)).map(|x| (t.id, x)));
            }
            TabContent::VariableStudio(v) => studios(&v.inserted),
            TabContent::Drawing(d) => out.extend(noshape_model::drawing::views::referenced_sources(d).iter().filter_map(|k| ExternalRef::parse(k.version.as_deref()?)).map(|x| (t.id, x))),
            _ => {}
        }
    }
    out
}

/// The tabs of `refs` with a reference to a document that has a newer
/// version (reads the other documents: call off the UI thread).
pub fn behind_tabs(refs: &[(TabId, ExternalRef)]) -> std::collections::HashSet<TabId> {
    let mut newer: std::collections::HashMap<(std::path::PathBuf, noshape_store::MicroversionId), bool> = Default::default();
    refs.iter().filter(|(_, r)| *newer.entry((r.path.clone(), r.microversion)).or_insert_with(|| derived::newer_version(r).is_some())).map(|(t, _)| *t).collect()
}

#[cfg(test)]
mod tests {
    use super::{behind_tabs, version_refs};
    use crate::derived::ExternalRef;
    use noshape_model::assembly::{Assembly, Instance, InstanceId, InstanceSource, Placement};
    use noshape_model::document::{Document, TabContent};

    /// The tab bar's out-of-date icon: an Assembly with an instance of
    /// another document at V1 is behind once that document has a V2.
    #[test]
    fn an_assembly_on_an_old_version_is_behind() {
        let dir = std::env::temp_dir().join(format!("noshape-behind-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src_path = dir.join("source.noshape");
        let mut src = crate::session::Session::create(&src_path, Document::new()).unwrap();
        src.create_version("V1", "").unwrap();
        let v1 = src.head().unwrap();
        let tab = src.doc.tabs[0].id;
        src.doc.tabs[0].name = "Plate".into();
        src.commit_all("Rename tab").unwrap();
        src.create_version("V2", "").unwrap();
        let v2 = src.head().unwrap();
        drop(src);
        let at = |mv| {
            let key = ExternalRef { path: src_path.clone(), microversion: mv, tab: tab.to_string() }.key();
            let mut doc = Document::new();
            let mut asm = Assembly::default();
            asm.instances.push(Instance::new(InstanceId(1), "Plate <1>".into(), InstanceSource::External { key, body: None, config: Default::default() }, Placement::default()));
            let t = doc.tabs.iter_mut().find(|t| matches!(t.content, TabContent::Assembly(_))).unwrap();
            t.content = TabContent::Assembly(Box::new(asm));
            let id = t.id;
            (doc, id)
        };
        let (doc, asm_tab) = at(v1);
        let refs = version_refs(&doc);
        assert_eq!(refs.iter().map(|(t, _)| *t).collect::<Vec<_>>(), [asm_tab]);
        assert!(behind_tabs(&refs).contains(&asm_tab));
        let (doc, _) = at(v2);
        assert!(behind_tabs(&version_refs(&doc)).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
