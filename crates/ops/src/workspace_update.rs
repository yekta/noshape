//! ☰ › Update workspace…: references of this workspace to other documents'
//! versions that have a newer version (Derived features, inserted Variable
//! Studios), updated together. Shared by the app's dialog
//! (crates/app/src/doc_dialogs.rs, which moves onto this module after the app
//! split) and the agent API (document_update_workspace).

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
pub const DESCRIPTION: &str = "Update workspace";

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
