//! The parts each Part Studio of a workspace makes, kept in the document's
//! metadata for the documents page's search (live documents/135: a search
//! for parts lists each part as its own row). Part names come from
//! regeneration, so they are written when a display is at hand (the
//! thumbnail's, or an open Part Studio view's) and read without regenerating.

use noshape_model::display::{BodyKind, StudioDisplay};
use noshape_model::document::TabId;
use noshape_model::BodyId;
use std::collections::BTreeMap;

/// Store metadata key prefix; the workspace (branch) id follows.
pub const KEY: &str = "search_parts:";

/// Each Part Studio tab's parts (id, name), by tab id.
pub type PartsIndex = BTreeMap<u64, Vec<(BodyId, String)>>;

/// The parts of a display (solid bodies, hidden ones too).
pub fn parts_of(display: &StudioDisplay) -> Vec<(BodyId, String)> {
    display.bodies.iter().filter(|b| b.kind == BodyKind::Solid).map(|b| (b.id.clone(), b.name.clone())).collect()
}

/// The index of a workspace (empty when none was written).
pub fn read(store: &noshape_store::Document, branch: noshape_store::BranchId) -> PartsIndex {
    store.get_meta(&format!("{KEY}{branch}")).ok().flatten().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Replace the entries of `fresh` tabs and drop the tabs not in `tabs`.
pub fn update(store: &mut noshape_store::Document, branch: noshape_store::BranchId, fresh: Vec<(TabId, Vec<(BodyId, String)>)>, tabs: &[TabId]) -> noshape_store::Result<()> {
    let mut index = read(store, branch);
    let before = index.clone();
    for (tab, parts) in fresh {
        index.insert(tab.0, parts);
    }
    index.retain(|t, _| tabs.iter().any(|x| x.0 == *t));
    if index == before {
        return Ok(());
    }
    let bytes = serde_json::to_vec(&index).unwrap_or_default();
    store.set_meta(&format!("{KEY}{branch}"), Some(&bytes))
}
