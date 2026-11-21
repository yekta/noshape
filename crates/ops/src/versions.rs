//! Version and workspace properties and Publish FeatureScript… (the
//! Versions and history panel), shared by the app and the agent API.

use noshape_model::document::{Document, TabContent};

/// Store meta key of a version published with Publish FeatureScript…
/// (`<key><version id>` = "1").
pub const PUBLISHED_FS_KEY: &str = "published_featurescript:";

/// Whether the version at `microversion` of the document at `path` was published with Publish FeatureScript… (the
/// Add custom features toast: "… from published version V4.").
pub fn is_published_fs(path: &std::path::Path, microversion: noshape_store::MicroversionId) -> bool {
    let Ok(store) = noshape_store::Document::open_read_only(path) else { return false };
    let Ok(graph) = store.graph() else { return false };
    let start = graph.microversions.iter().any(|m| m.id == microversion && m.parents.is_empty()) && store.get_meta(&format!("{PUBLISHED_FS_KEY}{START_KEY}")).ok().flatten().is_some();
    start || graph.versions.iter().filter(|v| v.microversion == microversion).any(|v| store.get_meta(&format!("{PUBLISHED_FS_KEY}{}", v.id)).ok().flatten().is_some())
}

/// The published key's suffix for the document's Start (a version without a version id).
pub const START_KEY: &str = "start";

/// What Publish FeatureScript… publishes: a version, or with `None` the document's Start: its published key, name and
/// state.
pub fn publish_target(store: &noshape_store::Document, version: Option<noshape_store::VersionId>) -> Option<(String, String, noshape_store::MicroversionId)> {
    match version {
        Some(id) => store.version(id).ok().map(|v| (format!("{PUBLISHED_FS_KEY}{id}"), v.name, v.microversion)),
        None => {
            let root = store.graph().ok()?.microversions.iter().find(|m| m.parents.is_empty())?.id;
            Some((format!("{PUBLISHED_FS_KEY}{START_KEY}"), store.start_name(), root))
        }
    }
}

/// Store meta key of a workspace's description (`<key><branch id>`).
pub const WORKSPACE_DESCRIPTION_KEY: &str = "workspace_description:";

/// The Publish FeatureScript requirements for a document (at a version),
/// in Onshape's order ("Document is public" is a cloud sharing state and
/// is not listed).
pub fn publish_checklist(doc: &Document) -> Vec<(&'static str, bool)> {
    use noshape_model::custom::studio::Severity;
    let scripts = noshape_model::custom::DocumentScripts::from_document(doc);
    let studios: Vec<_> = scripts.studios().map(|(_, _, c)| c.clone()).collect();
    let features: Vec<_> = studios.iter().flat_map(|c| c.features.iter()).collect();
    let used = |type_id: &str| doc.tabs.iter().any(|t| matches!(&t.content, TabContent::PartStudio(ps) if ps.features.iter().any(|f| f.type_id == type_id)));
    vec![
        ("Document must define at least one custom feature or table", studios.iter().any(|c| !c.features.is_empty() || !c.tables.is_empty())),
        ("Document must contain a PDF", doc.tabs.iter().any(|t| matches!(&t.content, TabContent::File(f) if f.kind() == noshape_model::file_tab::FileKind::Pdf))),
        ("Feature Studios contain no FeatureScript errors or warnings", studios.iter().all(|c| c.diagnostics.iter().all(|d| !matches!(d.severity, Severity::Error | Severity::Warning)))),
        ("All custom features have descriptions", features.iter().all(|f| !f.description.trim().is_empty())),
        ("All custom features are used in at least one Part Studio", features.iter().all(|f| used(&f.type_id))),
        ("All Feature Studios are committed.", doc.tabs.iter().all(|t| !matches!(&t.content, TabContent::FeatureStudio(f) if f.has_uncommitted()))),
    ]
}
