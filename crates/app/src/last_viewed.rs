//! The workspace and tab a document was last viewed on, per user (live ws-open-1119: a document reopens on the
//! workspace last viewed, from Recently opened, Owned by me or a link alike, and on the same tab; the document's
//! default workspace stays Main). Kept in the user's config folder, not in the document.

use noshape_model::document::TabId;
use noshape_store::BranchId;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Default)]
pub struct Viewed {
    pub branch: Option<BranchId>,
    pub tab: Option<TabId>,
}

fn file(dir: &Path) -> PathBuf {
    dir.join("last-viewed.json")
}

fn load(dir: &Path) -> Map<String, Value> {
    std::fs::read_to_string(file(dir)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn key(path: &Path) -> String {
    crate::files::canonical(path).to_string_lossy().into_owned()
}

fn parse(v: &Value) -> Viewed {
    Viewed { branch: serde_json::from_value(v["branch"].clone()).ok(), tab: v["tab"].as_u64().map(TabId) }
}

/// Where `path` was last viewed.
pub fn get(path: &Path) -> Viewed {
    get_in(&noshape_scene::config_dir(), path)
}

/// Note the workspace and tab `path` shows now (a write only when they changed).
pub fn note(path: &Path, branch: BranchId, tab: Option<TabId>) {
    note_in(&noshape_scene::config_dir(), path, branch, tab);
}

fn get_in(dir: &Path, path: &Path) -> Viewed {
    load(dir).get(&key(path)).map(parse).unwrap_or_default()
}

fn note_in(dir: &Path, path: &Path, branch: BranchId, tab: Option<TabId>) {
    let mut all = load(dir);
    let k = key(path);
    let was = all.get(&k).map(parse).unwrap_or_default();
    let tab = tab.or(was.tab);
    if was.branch == Some(branch) && was.tab == tab {
        return;
    }
    all.insert(k, json!({ "branch": branch, "tab": tab.map(|t| t.0) }));
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(file(dir), Value::Object(all).to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document noted on a workspace and tab reads them back; a later note without a tab keeps the tab; another
    /// document is apart.
    #[test]
    fn a_document_reopens_where_it_was_last_viewed() {
        let dir = std::env::temp_dir().join(format!("noshape-last-viewed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (a, b) = (dir.join("a.noshape"), dir.join("b.noshape"));
        let (b1, main) = (BranchId::generate(), BranchId::generate());
        note_in(&dir, &a, b1, Some(TabId(3)));
        let v = get_in(&dir, &a);
        assert!(v.branch == Some(b1) && v.tab == Some(TabId(3)));
        note_in(&dir, &a, main, None);
        let v = get_in(&dir, &a);
        assert!(v.branch == Some(main) && v.tab == Some(TabId(3)), "the tab kept");
        assert!(get_in(&dir, &b).branch.is_none(), "another document apart");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
