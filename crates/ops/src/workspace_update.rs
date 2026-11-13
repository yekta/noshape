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
    /// A Drawing's views and tables of another document's version (the drawing tab, that document at that version;
    /// pinned ones are left out).
    Drawing(TabId, ExternalRef),
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
            TabContent::Drawing(d) => {
                for x in drawing_refs(d) {
                    if let Some(v) = derived::newer_version(&x) {
                        let doc_name = derived::external_doc_info(&x.path).map(|i| i.name).unwrap_or_default();
                        let from = version_label(&x).unwrap_or_default();
                        out.push(Outdated { label: format!("{doc_name} in {}", t.name), from, to: v, kind: OutdatedKind::Drawing(t.id, x) });
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// A drawing's references to other documents' versions that aren't pinned (Pin reference): one per document and
/// version (the tab part of each key is left empty).
pub fn drawing_refs(d: &noshape_model::drawing::Drawing) -> Vec<ExternalRef> {
    use noshape_model::drawing::TableKind;
    let mut out: Vec<ExternalRef> = Vec::new();
    let views = d.sheets.iter().flat_map(|s| &s.views).filter_map(|v| v.pinned.as_ref().filter(|p| !p.locked).map(|p| p.id.as_str()));
    let tables = d.sheets.iter().flat_map(|s| &s.tables).filter_map(|t| match &t.kind {
        TableKind::Bom { source: Some(s), .. } => s.version.as_deref(),
        TableKind::CutList { source: Some(s), .. } => s.version.as_deref(),
        _ => None,
    });
    for k in views.chain(tables) {
        let Some(r) = ExternalRef::parse(k) else { continue };
        let r = ExternalRef { tab: String::new(), ..r };
        if !out.iter().any(|o| o.path == r.path && o.microversion == r.microversion) {
            out.push(r);
        }
    }
    out
}

/// The version name of another document's microversion ("V1").
pub fn version_label(r: &ExternalRef) -> Option<String> {
    derived::external_doc_info(&r.path).ok()?.versions.into_iter().find(|v| v.microversion == r.microversion && v.label != "Current state").map(|v| v.label)
}

/// Point a drawing's references to `path` at `from` (views and tables; pinned views stay) at version `to` of that
/// document (`doc_name`: its name, for the views' "<document> V2"). Returns how many changed.
pub fn repoint_drawing(d: &mut noshape_model::drawing::Drawing, path: &std::path::Path, from: noshape_store::MicroversionId, to: &VersionChoice, doc_name: &str) -> usize {
    use noshape_model::drawing::TableKind;
    let moved = |k: &str| ExternalRef::parse(k).filter(|r| r.path == path && r.microversion == from).map(|r| ExternalRef { microversion: to.microversion, ..r }.key());
    let mut n = 0;
    for v in d.sheets.iter_mut().flat_map(|s| s.views.iter_mut()) {
        if let Some(p) = v.pinned.as_mut().filter(|p| !p.locked)
            && let Some(key) = moved(&p.id)
        {
            (p.id, p.name) = (key, format!("{doc_name} {}", to.label));
            n += 1;
        }
    }
    for t in d.sheets.iter_mut().flat_map(|s| s.tables.iter_mut()) {
        let version = match &mut t.kind {
            TableKind::Bom { source: Some(s), .. } => &mut s.version,
            TableKind::CutList { source: Some(s), .. } => &mut s.version,
            _ => continue,
        };
        if let Some(key) = version.as_deref().and_then(moved) {
            *version = Some(key);
            n += 1;
        }
    }
    n
}

/// Pin reference / Unpin reference on the views of `source` (and `part`) showing another document's version.
/// Returns how many changed.
pub fn pin_drawing_reference(d: &mut noshape_model::drawing::Drawing, source: TabId, part: Option<&noshape_model::BodyId>, pin: bool) -> usize {
    use noshape_model::drawing::ViewKind;
    let mut n = 0;
    for v in d.sheets.iter_mut().flat_map(|s| s.views.iter_mut()) {
        let ours = matches!(&v.kind, ViewKind::Base { source: s, part: p, .. } if *s == source && p.as_ref() == part);
        if let Some(p) = v.pinned.as_mut().filter(|p| ours && p.locked != pin) {
            p.locked = pin;
            n += 1;
        }
    }
    n
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
        OutdatedKind::Drawing(tab, x) => {
            let name = derived::external_doc_info(&x.path).map(|i| i.name).unwrap_or_default();
            if let Some(TabContent::Drawing(d)) = doc.tab_mut(*tab).map(|t| &mut t.content) {
                repoint_drawing(d, &x.path, x.microversion, &o.to, &name);
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
            // (Pinned ones (Pin reference) give no notice.)
            TabContent::Drawing(d) => out.extend(drawing_refs(d).into_iter().map(|x| (t.id, x))),
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
