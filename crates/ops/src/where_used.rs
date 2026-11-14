//! Where used (a part's context menu; live2 11-14, feel/whereused.json): the assemblies, drawings and Derived features
//! that use a Part Studio's part, in this document and in the other documents on this computer. Live lists only
//! versions ("Where used for workspaces is not supported"); ours reads this document's workspace too.

use crate::derived::{ExternalRef, external_doc_info, external_snapshot};
use noshape_model::assembly::InstanceSource;
use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::drawing::ViewKind;
use noshape_model::reference::BodyId;
use std::path::{Path, PathBuf};

/// One use of the part.
#[derive(Clone, Debug, PartialEq)]
pub struct Use {
    /// The using document's name and file (this document's own uses: its own).
    pub document: String,
    pub path: PathBuf,
    /// The using tab and what it is: "assembly", "drawing" or "partStudio" (a Derived feature).
    pub element: String,
    pub kind: &'static str,
    /// The version of this document it uses: "Main" (the workspace), a version's name, or "Current state".
    pub version: String,
}

/// Whether `value` (a Derived feature's `partStudio`: a tab id, `tab#part`, or `ext:mv|tab#part|path`) names `tab`
/// of the document at `path`, as this document (`None`) or pinned at a microversion.
fn derived_source(value: &str) -> Option<(Option<ExternalRef>, String)> {
    let value = value.split('\t').next().unwrap_or(value);
    if value.starts_with("ext:") {
        let rest = value.strip_prefix("ext:")?;
        let mut it = rest.splitn(3, '|');
        let (mv, tab, path) = (it.next()?, it.next()?, it.next()?);
        let tab = tab.split('#').next().unwrap_or(tab).to_string();
        let r = ExternalRef::parse(&format!("ext:{mv}|{tab}|{path}"))?;
        return Some((Some(r), tab));
    }
    Some((None, value.split('#').next().unwrap_or(value).to_string()))
}

/// The uses inside `doc` (named `document`, at `path`) of `studio`'s part `body`: plain references when `pinned` is
/// None (this document's own tabs), else references to the document at `pinned` (another document's), each with
/// the version they pin.
fn uses_in(doc: &Document, document: &str, path: &Path, studio: TabId, body: &BodyId, pinned: Option<&Path>, version_of: &dyn Fn(noshape_store::MicroversionId) -> String) -> Vec<Use> {
    let same = |p: &Path| canonical(p) == canonical(pinned.unwrap_or(p));
    let mut out = Vec::new();
    let mut push = |element: &str, kind: &'static str, version: String| {
        let u = Use { document: document.to_string(), path: path.to_path_buf(), element: element.to_string(), kind, version };
        if !out.contains(&u) {
            out.push(u);
        }
    };
    for tab in &doc.tabs {
        match &tab.content {
            TabContent::Assembly(a) => {
                for i in &a.instances {
                    match (&i.source, pinned) {
                        (InstanceSource::Part { studio: s, body: b, .. } | InstanceSource::MirroredPart { studio: s, body: b, .. }, None) if *s == studio && b == body => push(&tab.name, "assembly", "Main".into()),
                        (InstanceSource::PartStudio { studio: s, .. }, None) if *s == studio => push(&tab.name, "assembly", "Main".into()),
                        (InstanceSource::External { key, body: b, .. }, Some(_)) => {
                            let Some(r) = ExternalRef::parse(key) else { continue };
                            if same(&r.path) && r.tab == studio.to_string() && b.as_ref().is_none_or(|b| b == body) {
                                push(&tab.name, "assembly", version_of(r.microversion));
                            }
                        }
                        _ => {}
                    }
                }
            }
            TabContent::Drawing(d) if pinned.is_none() => {
                let shows = d.sheets.iter().flat_map(|s| &s.views).any(|v| matches!(&v.kind, ViewKind::Base { source, part, .. } if *source == studio && part.as_ref().is_none_or(|p| p == body)));
                if shows {
                    push(&tab.name, "drawing", "Main".into());
                }
            }
            TabContent::PartStudio(ps) => {
                for f in ps.features.iter().filter(|f| f.type_id == "importDerived") {
                    let Some((r, tab_id)) = derived_source(f.params.text("partStudio")) else { continue };
                    match (r, pinned) {
                        (None, None) if tab_id == studio.to_string() => push(&tab.name, "partStudio", "Main".into()),
                        (Some(r), Some(_)) if same(&r.path) && tab_id == studio.to_string() => push(&tab.name, "partStudio", version_of(r.microversion)),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn canonical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// Where `studio`'s part `body` of the document `doc` (named `name`, saved at `path`) is used: in its own workspace,
/// then in each of `others` (other documents on this computer) at the version of this document they pin.
pub fn where_used(doc: &Document, name: &str, path: &Path, studio: TabId, body: &BodyId, others: &[PathBuf]) -> Vec<Use> {
    let mut out = uses_in(doc, name, path, studio, body, None, &|_| "Main".into());
    let versions = external_doc_info(path).map(|i| i.versions).unwrap_or_default();
    let version_of = |mv: noshape_store::MicroversionId| versions.iter().find(|v| v.microversion == mv).map_or_else(|| "Current state".into(), |v| v.label.clone());
    let me = canonical(path);
    for other in others.iter().filter(|p| canonical(p) != me) {
        let Ok(info) = external_doc_info(other) else { continue };
        let Some(head) = info.versions.last().map(|v| v.microversion) else { continue };
        let Ok(other_doc) = external_snapshot(other, head) else { continue };
        out.extend(uses_in(&other_doc, &info.name, other, studio, body, Some(path), &version_of));
    }
    out
}
