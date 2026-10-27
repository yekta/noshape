//! Derived-part provider and external document references live in the
//! headless operations layer, shared with the agent API (CLI / MCP).

pub use noshape_ops::derived::*;

/// A document's versions (named ones newest first, then "Start") and name.
pub fn document_versions(path: &std::path::Path) -> Option<(String, Vec<VersionChoice>)> {
    let store = noshape_store::Document::open_read_only(path).ok()?;
    let graph = store.graph().ok()?;
    let mut versions: Vec<VersionChoice> = graph.versions.iter().rev().map(|v| VersionChoice { label: v.name.clone(), microversion: v.microversion }).collect();
    if let Some(root) = graph.microversions.iter().find(|m| m.parents.is_empty()) {
        versions.push(VersionChoice { label: "Start".into(), microversion: root.id });
    }
    let name = store.meta().map(|m| m.name).unwrap_or_else(|_| path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    Some((name, versions))
}
