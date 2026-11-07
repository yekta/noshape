//! The Part Studio's display in the display cache (`crate::display_cache`): its `StudioDisplay`, keyed by the tab,
//! its configuration values and display fineness.

use crate::display_cache as cache;
use noshape_model::display::StudioDisplay;
use std::path::Path;
use std::sync::Arc;

pub(super) use cache::default_dir;

/// The cache key of tab `tab`'s display at configuration `config` (meshed finer: View in high quality).
pub(super) fn key(doc: &noshape_model::Document, tab: noshape_model::document::TabId, config: &impl serde::Serialize, fine: bool) -> Option<String> {
    cache::key(doc, &("part studio", tab, config, fine))
}

/// The display cached under `key`, if any.
pub(super) fn load(dir: &Path, key: &str) -> Option<StudioDisplay> {
    let mut d: StudioDisplay = cache::load(dir, key)?;
    // A pattern instance's shared mesh stands for its body's own (`SharedMesh::of` compares the two Arcs).
    let bodies: std::collections::HashMap<_, _> = d.bodies.iter().map(|b| (b.id.clone(), b.mesh.clone())).collect();
    d.shared_meshes.retain(|id, _| bodies.contains_key(id));
    for (id, s) in d.shared_meshes.iter_mut() {
        s.world = bodies[id].clone();
    }
    Some(d)
}

/// Keep `display` under `key`.
pub(super) fn store(dir: &Path, key: &str, display: &StudioDisplay) -> std::io::Result<()> {
    // Only the shared meshes that still stand for their body's.
    let mut d = display.clone();
    d.shared_meshes.retain(|id, s| d.bodies.iter().any(|b| &b.id == id && Arc::ptr_eq(&b.mesh, &s.world)));
    cache::store(dir, key, &d)
}
