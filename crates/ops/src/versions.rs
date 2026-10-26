//! Version and workspace properties and Publish FeatureScript… (the
//! Versions and history panel), shared by the app and the agent API.

use noshape_model::document::{Document, TabContent};

/// Store meta key of a version published with Publish FeatureScript…
/// (`<key><version id>` = "1").
pub const PUBLISHED_FS_KEY: &str = "published_featurescript:";

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
        ("Document contains a PDF", doc.tabs.iter().any(|t| matches!(&t.content, TabContent::File(f) if f.kind() == noshape_model::file_tab::FileKind::Pdf))),
        ("Feature Studios contain no FeatureScript errors or warnings", studios.iter().all(|c| c.diagnostics.iter().all(|d| !matches!(d.severity, Severity::Error | Severity::Warning)))),
        ("All custom features have descriptions", features.iter().all(|f| !f.description.trim().is_empty())),
        ("All custom features are used in at least one Part Studio", features.iter().all(|f| used(&f.type_id))),
        ("All Feature Studios are committed.", doc.tabs.iter().all(|t| !matches!(&t.content, TabContent::FeatureStudio(f) if f.has_uncommitted()))),
    ]
}
