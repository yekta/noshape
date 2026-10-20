//! A document session: the in-memory model document bound to its local
//! `.noshape` file. Every edit is committed as a microversion (there is no
//! Save command, like Onshape); undo/redo, versions, branches and merges go
//! through `noshape-store`.

use anyhow::{Context as _, Result};
use noshape_model::document::{Document, META_BLOB, TabId};
use noshape_store::{BranchId, Document as Store, MicroversionId, UndoStack, VersionId};
use std::path::{Path, PathBuf};

/// What edits on a historical (read-only) view report.
pub const READ_ONLY_MESSAGE: &str = "This is a read-only view of history. Return to the workspace to edit.";

pub struct Session {
    pub path: PathBuf,
    pub store: Store,
    pub doc: Document,
    pub branch: BranchId,
    undo: UndoStack,
    /// When set, the document shows a historical microversion read-only.
    pub viewing: Option<MicroversionId>,
    /// Tab whose edits Ctrl+Z / Ctrl+Y revert (Onshape: undo is per tab).
    pub active_tab: Option<TabId>,
    /// Onshape's "Switch to view only mode" (☰ menu).
    pub view_only: bool,
}

impl Session {
    pub fn create(path: impl AsRef<Path>, doc: Document) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let blobs = doc.to_blobs();
        let initial: Vec<(String, Vec<u8>)> = blobs.into_iter().collect();
        let mut store = Store::create(&path, &initial).with_context(|| format!("creating {}", path.display()))?;
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".into());
        store.set_name(&name)?;
        let branch = store.default_branch()?;
        Ok(Self { path, store, doc, branch, undo: UndoStack::new(branch).with_limit(500), viewing: None, active_tab: None, view_only: false })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let store = Store::open(&path).with_context(|| format!("opening {}", path.display()))?;
        let branch = store.default_branch()?;
        let head = store.head(branch)?;
        let doc = Document::from_blobs(&store.read_all(head)?)?;
        // Undo works across sessions: rebuild the stack from the history.
        let undo = UndoStack::from_history(&store, branch, 500).unwrap_or_else(|_| UndoStack::new(branch).with_limit(500));
        Ok(Self { path, store, doc, branch, undo, viewing: None, active_tab: None, view_only: false })
    }

    pub fn name(&self) -> String {
        self.store.meta().map(|m| m.name).unwrap_or_else(|_| "Untitled".into())
    }

    pub fn head(&self) -> Result<MicroversionId> {
        Ok(self.store.head(self.branch)?)
    }

    /// The name of the version being viewed ("V1"), when the viewed point in
    /// history is a version.
    pub fn viewing_version(&self) -> Option<String> {
        let mv = self.viewing?;
        if let Some(v) = self.store.list_versions().ok()?.into_iter().find(|v| v.microversion == mv) {
            return Some(v.name);
        }
        // The first point of the history is the "Start" version (live:
        // "Versions are view only. Viewing Start.").
        let graph = self.store.graph().ok()?;
        graph.microversions.iter().find(|m| m.id == mv && m.parents.is_empty()).map(|_| "Start".to_string())
    }

    /// No edits: viewing a point in history, or Onshape's "view only mode".
    pub fn read_only(&self) -> bool {
        self.viewing.is_some() || self.view_only
    }

    /// Edit one tab's content and commit it as one microversion described
    /// "<tab name> :: <description>", the way every UI edit is recorded.
    /// `None` when the tab doesn't exist.
    pub fn edit_tab<R>(&mut self, tab: TabId, description: &str, f: impl FnOnce(&mut noshape_model::document::TabContent) -> R) -> Result<Option<R>> {
        anyhow::ensure!(!self.read_only(), "{READ_ONLY_MESSAGE}");
        let tab_name = self.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
        let Some(t) = self.doc.tab_mut(tab) else { return Ok(None) };
        let r = f(&mut t.content);
        self.commit(&[tab], false, &format!("{tab_name} :: {description}"))?;
        Ok(Some(r))
    }

    /// A structural document edit (tabs added, removed, renamed, settings),
    /// committed as one undoable microversion.
    pub fn edit_document<R>(&mut self, description: &str, f: impl FnOnce(&mut Document) -> R) -> Result<R> {
        anyhow::ensure!(!self.read_only(), "{READ_ONLY_MESSAGE}");
        let r = f(&mut self.doc);
        self.commit_all(description)?;
        Ok(r)
    }

    /// A structural edit Ctrl+Z can't undo (Onshape: deleting a tab; the
    /// Versions and history panel restores it).
    pub fn edit_document_not_undoable<R>(&mut self, description: &str, f: impl FnOnce(&mut Document) -> R) -> Result<R> {
        anyhow::ensure!(!self.read_only(), "{READ_ONLY_MESSAGE}");
        let r = f(&mut self.doc);
        self.commit_all_not_undoable(description)?;
        Ok(r)
    }

    /// Edit the document settings (units) as one microversion.
    pub fn edit_settings<R>(&mut self, description: &str, f: impl FnOnce(&mut noshape_model::document::DocumentSettings) -> R) -> Result<R> {
        anyhow::ensure!(!self.read_only(), "{READ_ONLY_MESSAGE}");
        let r = f(&mut self.doc.settings);
        self.commit(&[], true, description)?;
        Ok(r)
    }

    /// Commit changed tabs (and settings) as one microversion.
    pub fn commit(&mut self, tabs: &[TabId], settings: bool, description: &str) -> Result<MicroversionId> {
        anyhow::ensure!(self.viewing.is_none(), "viewing history is read-only");
        let mut changes: Vec<(String, Option<Vec<u8>>)> = Vec::new();
        for &t in tabs {
            changes.push((Document::blob_name(t), self.doc.tab_blob(t)));
        }
        if settings {
            changes.push((META_BLOB.to_string(), Some(self.doc.settings_blob())));
        }
        Ok(self.undo.commit(&mut self.store, &changes, description)?)
    }

    /// Commit the whole document as an edit Ctrl+Z can't undo (deleting a
    /// tab, as in Onshape; history restores it).
    pub fn commit_all_not_undoable(&mut self, description: &str) -> Result<MicroversionId> {
        let changes = self.all_changes()?;
        Ok(self.undo.commit_not_undoable(&mut self.store, &changes, description)?)
    }

    fn all_changes(&self) -> Result<Vec<(String, Option<Vec<u8>>)>> {
        let head = self.head()?;
        let existing: Vec<String> = self.store.names(head)?;
        let blobs = self.doc.to_blobs();
        let mut changes: Vec<(String, Option<Vec<u8>>)> = blobs.into_iter().map(|(k, v)| (k, Some(v))).collect();
        for name in existing {
            if !changes.iter().any(|(k, _)| *k == name) {
                changes.push((name, None));
            }
        }
        Ok(changes)
    }

    /// Commit the whole document (used after structural changes).
    pub fn commit_all(&mut self, description: &str) -> Result<MicroversionId> {
        let head = self.head()?;
        let existing: Vec<String> = self.store.names(head)?;
        let blobs = self.doc.to_blobs();
        let mut changes: Vec<(String, Option<Vec<u8>>)> = blobs.into_iter().map(|(k, v)| (k, Some(v))).collect();
        for name in existing {
            if !changes.iter().any(|(k, _)| *k == name) {
                changes.push((name, None));
            }
        }
        Ok(self.undo.commit(&mut self.store, &changes, description)?)
    }

    fn reload(&mut self, mv: MicroversionId) -> Result<()> {
        self.doc = Document::from_blobs(&self.store.read_all(mv)?)?;
        Ok(())
    }

    /// Edits undo/redo may revert from the active tab: those that changed
    /// its data, or only document-level data (no tab).
    fn tab_scope(&self) -> Option<impl Fn(&[String]) -> bool + use<>> {
        let blob = Document::blob_name(self.active_tab?);
        Some(move |names: &[String]| names.contains(&blob) || names.iter().all(|n| !n.starts_with("tab/")))
    }

    pub fn can_undo(&self) -> bool {
        match self.tab_scope() {
            Some(scope) => self.undo.can_undo_in(&self.store, &scope),
            None => self.undo.can_undo(),
        }
    }
    pub fn can_redo(&self) -> bool {
        match self.tab_scope() {
            Some(scope) => self.undo.can_redo_in(&self.store, &scope),
            None => self.undo.can_redo(),
        }
    }

    pub fn undo(&mut self) -> Result<bool> {
        let mv = match self.tab_scope() {
            Some(scope) => self.undo.undo_in(&mut self.store, &scope)?,
            None => self.undo.undo(&mut self.store)?,
        };
        match mv {
            Some(mv) => {
                self.reload(mv)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub fn redo(&mut self) -> Result<bool> {
        let mv = match self.tab_scope() {
            Some(scope) => self.undo.redo_in(&mut self.store, &scope)?,
            None => self.undo.redo(&mut self.store)?,
        };
        match mv {
            Some(mv) => {
                self.reload(mv)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Show a historical microversion read-only (None returns to the branch head).
    pub fn view_microversion(&mut self, mv: Option<MicroversionId>) -> Result<()> {
        let target = match mv {
            Some(m) => m,
            None => self.head()?,
        };
        self.reload(target)?;
        self.viewing = mv;
        Ok(())
    }

    pub fn restore(&mut self, to: MicroversionId) -> Result<()> {
        let before = self.head()?;
        let after = self.store.restore(self.branch, to)?;
        self.undo.record(before, after, "Restore");
        self.viewing = None;
        self.reload(after)
    }

    /// The document as it was at a version (`id` as text), for drawing views
    /// pinned to it (drawings, asb). None when there is no such version.
    pub fn document_at_version(&self, id: &str) -> Result<Option<Document>> {
        let Some(v) = self.store.list_versions()?.into_iter().find(|v| v.id.to_string() == id) else { return Ok(None) };
        Ok(Some(Document::from_blobs(&self.store.read_all(v.microversion)?)?))
    }

    /// The documents at the versions the drawings pin views to (for work
    /// off the main thread, e.g. drawing thumbnails).
    pub fn pinned_documents(&self) -> std::collections::HashMap<String, Document> {
        noshape_model::drawing::views::pinned_versions(&self.doc).into_iter().filter_map(|id| Some((id.clone(), self.document_at_version(&id).ok().flatten()?))).collect()
    }

    pub fn create_version(&mut self, name: &str, description: &str) -> Result<VersionId> {
        let head = self.head()?;
        Ok(self.store.create_version(head, name, description)?)
    }

    /// Create a branch from a microversion and switch to it.
    pub fn branch_from(&mut self, from: MicroversionId, name: &str) -> Result<BranchId> {
        let b = self.store.create_branch(from, name)?;
        self.switch_branch(b)?;
        Ok(b)
    }

    pub fn switch_branch(&mut self, b: BranchId) -> Result<()> {
        self.branch = b;
        self.undo = UndoStack::new(b).with_limit(500);
        self.viewing = None;
        let head = self.head()?;
        self.reload(head)
    }

    /// Merge another branch into the current one, feature-by-feature for Part Studios.
    pub fn merge_from(&mut self, from: BranchId) -> Result<noshape_store::MergeOutcome> {
        let before = self.head()?;
        let outcome = self.store.merge(from, self.branch, |name, base, ours, theirs| {
            if name.starts_with("tab/") {
                noshape_model::merge::merge_tab_blobs(base, ours, theirs)
            } else {
                ours.map(|o| o.to_vec())
            }
        })?;
        self.undo.record(before, outcome.microversion, "Merge");
        self.reload(outcome.microversion)?;
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_commit_undo_reopen() {
        let dir = std::env::temp_dir().join(format!("noshape-session-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("t.noshape");
        let _ = std::fs::remove_file(&path);
        let mut s = Session::create(&path, Document::new()).unwrap();
        let tab = s.doc.tabs[0].id;
        s.doc.tab_mut(tab).unwrap().name = "Renamed".into();
        s.commit(&[tab], false, "Rename tab").unwrap();
        assert!(s.can_undo());
        s.undo().unwrap();
        assert_eq!(s.doc.tab(tab).unwrap().name, "Part Studio 1");
        s.redo().unwrap();
        assert_eq!(s.doc.tab(tab).unwrap().name, "Renamed");
        drop(s);
        let s2 = Session::open(&path).unwrap();
        assert_eq!(s2.doc.tab(tab).unwrap().name, "Renamed");
    }

    #[test]
    fn document_at_a_version() {
        let dir = std::env::temp_dir().join(format!("noshape-session-v-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("v.noshape");
        let _ = std::fs::remove_file(&path);
        let mut s = Session::create(&path, Document::new()).unwrap();
        let tab = s.doc.tabs[0].id;
        let v = s.create_version("V1", "").unwrap();
        s.doc.tab_mut(tab).unwrap().name = "Later".into();
        s.commit(&[tab], false, "Rename tab").unwrap();
        // The version keeps the old name; unknown ids give None.
        let at = s.document_at_version(&v.to_string()).unwrap().unwrap();
        assert_eq!(at.tab(tab).unwrap().name, "Part Studio 1");
        assert!(s.document_at_version("no-such-version").unwrap().is_none());
    }

    #[test]
    fn pinned_drawing_views_get_their_version() {
        use noshape_model::document::TabContent;
        let dir = std::env::temp_dir().join(format!("noshape-session-pin-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("p.noshape");
        let _ = std::fs::remove_file(&path);
        let mut s = Session::create(&path, noshape_model::demo::demo_document_with_drawing()).unwrap();
        assert!(s.pinned_documents().is_empty());
        let v = s.create_version("V1", "").unwrap().to_string();
        let tab = s.doc.tabs.iter().find(|t| matches!(t.content, TabContent::Drawing(_))).unwrap().id;
        if let Some(TabContent::Drawing(d)) = s.doc.tab_mut(tab).map(|t| &mut t.content) {
            d.sheets[0].views[0].pinned = Some(noshape_model::drawing::PinnedVersion { id: v.clone(), name: "V1".into() });
        }
        s.commit(&[tab], false, "Pin view reference").unwrap();
        let docs = s.pinned_documents();
        assert_eq!(docs.keys().collect::<Vec<_>>(), vec![&v]);
    }
}
