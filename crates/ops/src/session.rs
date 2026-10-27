//! A document session: the in-memory model document bound to its local
//! `.noshape` file. Every edit is committed as a microversion (there is no
//! Save command, like Onshape); undo/redo, versions, branches and merges go
//! through `noshape-store`.

use anyhow::{Context as _, Result};
use noshape_model::document::{Document, DocumentSettings, META_BLOB, Tab, TabId};
use noshape_store::{BranchId, Document as Store, MicroversionId, UndoStack, VersionId};
use std::path::{Path, PathBuf};

/// What edits on a historical (read-only) view report.
pub const READ_ONLY_MESSAGE: &str = "This is a read-only view of history. Return to the workspace to edit.";

/// What edits on a protected workspace report (Onshape "Workspace
/// protections": changes are made on a branch and merged in).
pub const PROTECTED_MESSAGE: &str = "This workspace is protected. Make changes in another workspace and merge them into this one.";

/// Document settings that make up the tab bar (order, folders, nesting).
const TAB_BAR_FIELDS: [&str; 3] = ["tab_order", "folders", "parents"];

/// The name in a tab blob (the rest is skipped, not built).
fn tab_name(blob: &[u8]) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Named {
        name: String,
    }
    serde_json::from_slice::<Named>(blob).ok().map(|n| n.name)
}

/// A blob's JSON without `fields` (None when it isn't a JSON object).
fn without_fields(blob: &[u8], fields: &[&str]) -> Option<serde_json::Value> {
    let mut v: serde_json::Value = serde_json::from_slice(blob).ok()?;
    let map = v.as_object_mut()?;
    for f in fields {
        map.remove(*f);
    }
    Some(v)
}

/// What undo/redo writes back for blob `name` (`restored`), given the
/// current document: tab operations can't be undone, so tabs keep their
/// current names, deleted tabs stay deleted and the tab bar keeps its
/// order and folders.
fn keep_tab_bar(doc: &Document, name: &str, restored: Option<Vec<u8>>) -> Option<Vec<u8>> {
    let bytes = restored?;
    if name == META_BLOB {
        let Ok(mut settings) = serde_json::from_slice::<DocumentSettings>(&bytes) else { return Some(bytes) };
        let now = &doc.settings;
        if settings.tab_order == now.tab_order && settings.folders == now.folders && settings.parents == now.parents {
            return Some(bytes);
        }
        settings.tab_order = now.tab_order.clone();
        settings.folders = now.folders.clone();
        settings.parents = now.parents.clone();
        return Some(serde_json::to_vec(&settings).unwrap_or(bytes));
    }
    if !name.starts_with("tab/") {
        return Some(bytes);
    }
    let now = doc.tabs.iter().find(|t| Document::blob_name(t.id) == name)?;
    let Ok(mut tab) = serde_json::from_slice::<Tab>(&bytes) else { return Some(bytes) };
    if tab.name == now.name {
        return Some(bytes);
    }
    tab.name = now.name.clone();
    Some(serde_json::to_vec(&tab).unwrap_or(bytes))
}

/// Store meta key of a protected workspace (`workspace_protected:<branch>`).
pub const PROTECTED_KEY: &str = "workspace_protected:";

/// Whether process `pid` still runs (a crash marker of a live process is an
/// evaluation in progress, not a crash).
fn process_alive(pid: u32) -> bool {
    if cfg!(target_os = "linux") {
        return Path::new("/proc").join(pid.to_string()).exists();
    }
    #[cfg(unix)]
    let alive = std::process::Command::new("kill").args(["-0", &pid.to_string()]).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success());
    #[cfg(windows)]
    let alive = std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()));
    #[cfg(not(any(unix, windows)))]
    let alive = false;
    alive
}

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
    /// What opening did to features that had crashed a process while they
    /// were evaluated ([`Session::open`]): one note per feature.
    pub recovered: Vec<String>,
    /// Set while an allowed edit runs on a protected workspace (release
    /// metadata; [`Session::with_protection_lifted`]).
    protection_lifted: bool,
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
        // The document's features may need the FeatureScript host (custom features, std-run features).
        noshape_model::custom::register();
        noshape_model::regen::crash_guard::enable(noshape_model::regen::crash_guard::default_dir());
        Ok(Self { path, store, doc, branch, undo: UndoStack::new(branch).with_limit(500), viewing: None, active_tab: None, view_only: false, recovered: Vec::new(), protection_lifted: false })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let store = Store::open(&path).with_context(|| format!("opening {}", path.display()))?;
        let branch = store.default_branch()?;
        let head = store.head(branch)?;
        let doc = Document::from_blobs(&store.read_all(head)?)?;
        // The document's features may need the FeatureScript host (custom features, std-run features); recovery
        // below regenerates.
        noshape_model::custom::register();
        // Undo works across sessions: rebuild the stack from the history.
        let undo = UndoStack::from_history(&store, branch, 500).unwrap_or_else(|_| UndoStack::new(branch).with_limit(500));
        let mut s = Self { path, store, doc, branch, undo, viewing: None, active_tab: None, view_only: false, recovered: Vec::new(), protection_lifted: false };
        noshape_model::regen::crash_guard::enable(noshape_model::regen::crash_guard::default_dir());
        if let Some(dir) = noshape_model::regen::crash_guard::dir() {
            s.recovered = s.recover_crashed_features(dir);
        }
        Ok(s)
    }

    /// qa DOC-POISON: a feature whose evaluation killed a process (its
    /// crash marker left behind, `noshape_model::regen::crash_guard`) would
    /// kill every process that opens the document. Each Part Studio of this
    /// document holding one gets its rollback bar above it (one undoable
    /// edit, named so the history says why), so the document opens and the
    /// feature can be edited or deleted. Markers of other documents stay.
    pub fn recover_crashed_features(&mut self, dir: &Path) -> Vec<String> {
        use noshape_model::document::TabContent;
        let mut notes = Vec::new();
        for m in noshape_model::regen::crash_guard::markers(dir) {
            if m.pid == std::process::id() || process_alive(m.pid) {
                continue;
            }
            let hit = self.doc.tabs.iter().find_map(|t| match &t.content {
                TabContent::PartStudio(ps) => ps.features.iter().position(|f| f.id.to_string() == m.feature).map(|i| (t.id, i, ps.rollback_index(), ps.features[i].name.clone())),
                _ => None,
            });
            let Some((tab, index, rollback, name)) = hit else { continue };
            if index < rollback {
                let lifted = std::mem::replace(&mut self.protection_lifted, true);
                let r = self.edit_tab(tab, &format!("Roll back above {name}: it stopped noshape while regenerating"), |c| {
                    if let TabContent::PartStudio(ps) = c {
                        ps.rollback = Some(index);
                    }
                });
                self.protection_lifted = lifted;
                if r.is_err() {
                    continue;
                }
                notes.push(format!("{name} stopped noshape the last time it regenerated: the rollback bar is now above it. Edit or delete it, then move the rollback bar back."));
            }
            let _ = std::fs::remove_file(&m.path);
        }
        notes
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

    /// Onshape's workspace protection: the branch takes no model edits (tab
    /// content, tabs, settings, restore, undo / redo); versions, branches and
    /// merges into it are still allowed.
    pub fn branch_protected(&self, b: BranchId) -> bool {
        self.store.get_meta(&format!("{PROTECTED_KEY}{b}")).ok().flatten().is_some()
    }

    /// The current workspace is protected.
    pub fn is_protected(&self) -> bool {
        self.viewing.is_none() && self.branch_protected(self.branch)
    }

    pub fn set_branch_protected(&mut self, b: BranchId, on: bool) -> Result<()> {
        Ok(self.store.set_meta(&format!("{PROTECTED_KEY}{b}"), on.then_some(b"1".as_slice()))?)
    }

    /// Runs an edit a protected workspace allows (release management
    /// metadata).
    pub fn with_protection_lifted<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let was = std::mem::replace(&mut self.protection_lifted, true);
        let r = f(self);
        self.protection_lifted = was;
        r
    }

    fn ensure_unprotected(&self) -> Result<()> {
        anyhow::ensure!(self.protection_lifted || !self.is_protected(), "{PROTECTED_MESSAGE}");
        Ok(())
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
        // Refused before the edit touches the in-memory document.
        self.ensure_unprotected()?;
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
        // Refused before the edit touches the in-memory document.
        self.ensure_unprotected()?;
        let r = f(&mut self.doc);
        self.commit_all(description)?;
        Ok(r)
    }

    /// A structural edit Ctrl+Z can't undo (Onshape: deleting a tab; the
    /// Versions and history panel restores it).
    pub fn edit_document_not_undoable<R>(&mut self, description: &str, f: impl FnOnce(&mut Document) -> R) -> Result<R> {
        anyhow::ensure!(!self.read_only(), "{READ_ONLY_MESSAGE}");
        // Refused before the edit touches the in-memory document.
        self.ensure_unprotected()?;
        let r = f(&mut self.doc);
        self.commit_all_not_undoable(description)?;
        Ok(r)
    }

    /// Edit the document settings (units) as one microversion.
    pub fn edit_settings<R>(&mut self, description: &str, f: impl FnOnce(&mut noshape_model::document::DocumentSettings) -> R) -> Result<R> {
        anyhow::ensure!(!self.read_only(), "{READ_ONLY_MESSAGE}");
        // Refused before the edit touches the in-memory document.
        self.ensure_unprotected()?;
        let r = f(&mut self.doc.settings);
        self.commit(&[], true, description)?;
        Ok(r)
    }

    /// Change the workspace units (one microversion). Sketch dimensions
    /// stored without a unit are pinned to the old one first, so the
    /// geometry doesn't change (`quantity_text::pin_sketch_units`).
    pub fn set_units(&mut self, units: noshape_units::UnitPrefs, description: &str) -> Result<()> {
        anyhow::ensure!(!self.read_only(), "{READ_ONLY_MESSAGE}");
        self.ensure_unprotected()?;
        let old = self.doc.settings.units;
        let tabs = if old.length != units.length || old.angle != units.angle { noshape_model::quantity_text::pin_sketch_units(&mut self.doc, &old) } else { Vec::new() };
        self.doc.settings.units = units;
        self.commit(&tabs, true, description)?;
        Ok(())
    }

    /// Commit changed tabs (and settings) as one microversion.
    pub fn commit(&mut self, tabs: &[TabId], settings: bool, description: &str) -> Result<MicroversionId> {
        anyhow::ensure!(self.viewing.is_none(), "viewing history is read-only");
        self.ensure_unprotected()?;
        let mut changes: Vec<(String, Option<Vec<u8>>)> = Vec::new();
        for &t in tabs {
            changes.push((Document::blob_name(t), self.doc.tab_blob(t)));
        }
        if settings {
            changes.push((META_BLOB.to_string(), Some(self.doc.settings_blob())));
        }
        if self.tab_bar_only(&changes)? {
            return Ok(self.undo.commit_not_undoable(&mut self.store, &changes, description)?);
        }
        Ok(self.undo.commit(&mut self.store, &changes, description)?)
    }

    /// Commit the whole document as an edit Ctrl+Z can't undo (deleting a
    /// tab, as in Onshape; history restores it).
    pub fn commit_all_not_undoable(&mut self, description: &str) -> Result<MicroversionId> {
        self.ensure_unprotected()?;
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

    /// Commit the whole document (used after structural changes). An edit
    /// of the tab bar only (tabs added or removed, renamed, reordered,
    /// foldered) is not undoable, as in Onshape.
    pub fn commit_all(&mut self, description: &str) -> Result<MicroversionId> {
        self.ensure_unprotected()?;
        let changes = self.all_changes()?;
        if self.tab_bar_only(&changes)? {
            return Ok(self.undo.commit_not_undoable(&mut self.store, &changes, description)?);
        }
        Ok(self.undo.commit(&mut self.store, &changes, description)?)
    }

    /// Whether `changes` (against the head) touch nothing but the tab bar:
    /// tabs created or deleted, tab names, and the tab order and folders.
    fn tab_bar_only(&self, changes: &[(String, Option<Vec<u8>>)]) -> Result<bool> {
        let head = self.head()?;
        let mut any = false;
        for (name, new) in changes {
            let old = self.store.read(head, name)?;
            if old == *new {
                continue;
            }
            any = true;
            let (Some(old), Some(new)) = (old, new) else {
                if name.starts_with("tab/") {
                    continue;
                }
                return Ok(false);
            };
            let fields: &[&str] = if name == META_BLOB {
                &TAB_BAR_FIELDS
            } else if name.starts_with("tab/") {
                // A content edit (the common case) keeps the name: no need
                // to compare the whole tab.
                if tab_name(&old) == tab_name(new) {
                    return Ok(false);
                }
                &["name"]
            } else {
                return Ok(false);
            };
            if without_fields(&old, fields) != without_fields(new, fields) {
                return Ok(false);
            }
        }
        Ok(any)
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

    /// The edit the last undo reverted ("Part Studio 1 :: Insert feature : Hole 1").
    pub fn last_undone(&self) -> Option<&str> {
        self.undo.redo_description()
    }

    /// The edit the last redo re-applied.
    pub fn last_redone(&self) -> Option<&str> {
        self.undo.undo_description()
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
        self.ensure_unprotected()?;
        let doc = &self.doc;
        let keep = |name: &str, v: Option<Vec<u8>>| keep_tab_bar(doc, name, v);
        let mv = match self.tab_scope() {
            Some(scope) => self.undo.undo_in_keeping(&mut self.store, &scope, &keep)?,
            None => self.undo.undo_in_keeping(&mut self.store, &|_| true, &keep)?,
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
        self.ensure_unprotected()?;
        let doc = &self.doc;
        let keep = |name: &str, v: Option<Vec<u8>>| keep_tab_bar(doc, name, v);
        let mv = match self.tab_scope() {
            Some(scope) => self.undo.redo_in_keeping(&mut self.store, &scope, &keep)?,
            None => self.undo.redo_in_keeping(&mut self.store, &|_| true, &keep)?,
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
        self.ensure_unprotected()?;
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

    /// Drop every undo/redo step (the history stays). The app calls this when
    /// it opens a document: as in Onshape, where a page load starts every
    /// tab with empty stacks, edits made before can't be undone. A CLI session
    /// keeps the stack `open` rebuilt from the history.
    pub fn forget_undo(&mut self) {
        self.undo = UndoStack::new(self.branch).with_limit(500);
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
        let head = self.store.head(from)?;
        let name = self.store.branch(from)?.name;
        self.merge_picked(head, &name, Some(from), |_| noshape_store::MergePick::Auto)
    }

    /// Onshape's Merge dialog: merge `from` (a workspace head or a version)
    /// into the current workspace with each name's strategy (`pick`: Keep =
    /// `Ours`, Replace = `Theirs`, Merge changes = `Auto`).
    pub fn merge_picked(&mut self, from: MicroversionId, from_name: &str, from_branch: Option<BranchId>, pick: impl FnMut(&str) -> noshape_store::MergePick) -> Result<noshape_store::MergeOutcome> {
        let before = self.head()?;
        let outcome = self.store.merge_with(from, from_name, from_branch, self.branch, pick, |name, base, ours, theirs| {
            if name.starts_with("tab/") {
                noshape_model::merge::merge_tab_blobs(base, ours, theirs)
            } else {
                ours.map(|o| o.to_vec())
            }
        })?;
        if !outcome.up_to_date {
            self.undo.record(before, outcome.microversion, "Merge");
            self.reload(outcome.microversion)?;
        }
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_workspace_takes_merges_not_edits() {
        let dir = std::env::temp_dir().join(format!("noshape-session-protect-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("p.noshape");
        let _ = std::fs::remove_file(&path);
        let mut s = Session::create(&path, Document::new()).unwrap();
        let (main, tab) = (s.branch, s.doc.tabs[0].id);
        s.set_branch_protected(main, true).unwrap();
        assert!(s.is_protected());
        // Model edits are refused, with Onshape's reason.
        let e = s.edit_tab(tab, "Rename", |_| ()).unwrap_err();
        assert!(e.to_string().contains("protected"), "{e}");
        assert!(s.edit_document("Add tab", |_| ()).is_err());
        assert!(s.undo().is_err());
        // Versions and branches are allowed.
        s.create_version("V1", "").unwrap();
        let head = s.head().unwrap();
        let b = s.branch_from(head, "B1").unwrap();
        assert!(!s.is_protected());
        s.doc.tab_mut(tab).unwrap().name = "Changed on B1".into();
        s.commit(&[tab], false, "Rename tab").unwrap();
        // Merging the branch into the protected workspace is allowed.
        s.switch_branch(main).unwrap();
        s.merge_from(b).unwrap();
        assert_eq!(s.doc.tab(tab).unwrap().name, "Changed on B1");
        // Release metadata may be written through the escape hatch.
        s.with_protection_lifted(|s| s.edit_tab(tab, "Release", |_| ())).unwrap();
        s.set_branch_protected(main, false).unwrap();
        s.edit_tab(tab, "Rename", |_| ()).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// qa DOC-POISON: a feature that crashed a process (its marker left by
    /// a process that is gone) is rolled back on open, once, with a note;
    /// the document then opens without evaluating it.
    #[test]
    fn a_feature_that_crashed_is_rolled_back_on_open() {
        use noshape_model::document::TabContent;
        let dir = std::env::temp_dir().join(format!("noshape-session-poison-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("p.noshape");
        let _ = std::fs::remove_file(&path);
        let mut doc = Document::new();
        let units = doc.settings.units;
        let tab = doc.tabs.iter_mut().find(|t| matches!(t.content, TabContent::PartStudio(_))).expect("a Part Studio");
        let (tab_id, TabContent::PartStudio(ps)) = (tab.id, &mut tab.content) else { unreachable!() };
        for _ in 0..3 {
            let f = ps.new_feature("extrude", &units).expect("extrude");
            ps.features.push(f);
        }
        let (bad, name) = (ps.features[1].id, ps.features[1].name.clone());
        drop(Session::create(&path, doc).unwrap());
        // The marker a crashed process left (no such process runs).
        let markers = noshape_model::regen::crash_guard::default_dir();
        std::fs::create_dir_all(&markers).unwrap();
        let marker = markers.join(format!("4294967294-{}.marker", std::process::id()));
        std::fs::write(&marker, format!("4294967294\n{bad}\n{name}\n")).unwrap();
        let s = Session::open(&path).unwrap();
        let TabContent::PartStudio(ps) = &s.doc.tab(tab_id).unwrap().content else { unreachable!() };
        assert_eq!(ps.rollback, Some(1), "the rollback bar above the feature");
        assert!(s.recovered.len() == 1 && s.recovered[0].contains(&name), "{:?}", s.recovered);
        assert!(!marker.exists(), "the marker is cleared");
        let last = s.store.history(s.branch).unwrap()[0].description.clone();
        assert!(last.ends_with(&format!(":: Roll back above {name}: it stopped noshape while regenerating")), "{last}");
        drop(s);
        // Opened again: nothing more to do.
        assert!(Session::open(&path).unwrap().recovered.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_commit_undo_reopen() {
        let dir = std::env::temp_dir().join(format!("noshape-session-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("t.noshape");
        let _ = std::fs::remove_file(&path);
        let mut s = Session::create(&path, Document::new()).unwrap();
        let tab = s.doc.tabs[0].id;
        s.doc.settings.description = "Edited".into();
        s.commit(&[], true, "Edit description").unwrap();
        assert!(s.can_undo());
        s.undo().unwrap();
        assert_eq!(s.doc.settings.description, "");
        s.redo().unwrap();
        assert_eq!(s.doc.settings.description, "Edited");
        s.doc.tab_mut(tab).unwrap().name = "Renamed".into();
        s.commit(&[tab], false, "Rename tab").unwrap();
        drop(s);
        let s2 = Session::open(&path).unwrap();
        assert_eq!(s2.doc.tab(tab).unwrap().name, "Renamed");
        assert_eq!(s2.doc.settings.description, "Edited");
    }

    /// qa5 TABS5 (live: tab rename, reorder, delete and duplicate are not
    /// undoable): Ctrl+Z after a rename does nothing, and undoing an edit
    /// made before it keeps the new name and the tab order.
    #[test]
    fn tab_operations_are_not_undoable() {
        use noshape_model::document::TabContent;
        let dir = std::env::temp_dir().join(format!("noshape-session-tabs-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("t.noshape");
        let _ = std::fs::remove_file(&path);
        let mut s = Session::create(&path, Document::new()).unwrap();
        let tab = s.doc.tabs[0].id;
        s.active_tab = Some(tab);
        // A rename alone: nothing to undo.
        s.edit_document("Rename tab : A", |d| d.tab_mut(tab).unwrap().name = "A".into()).unwrap();
        assert!(!s.can_undo());
        // A content edit, then a rename, a new tab and a reorder.
        s.edit_tab(tab, "Insert feature", |c| {
            if let TabContent::PartStudio(ps) = c {
                ps.features.clear();
            }
        })
        .unwrap();
        s.doc.settings.description = "x".into();
        s.commit(&[tab], true, "Edit").unwrap();
        assert!(s.can_undo());
        s.edit_document("Rename tab : B", |d| d.tab_mut(tab).unwrap().name = "B".into()).unwrap();
        let other = s.edit_document("Insert tab : Part Studio 2", |d| d.add_tab("Part Studio 2", TabContent::PartStudio(Default::default()))).unwrap();
        let order = vec![other, tab];
        s.edit_document("Reorder tabs", |d| d.settings.tab_order = order.clone()).unwrap();
        s.undo().unwrap();
        assert_eq!(s.doc.settings.description, "");
        assert_eq!(s.doc.tab(tab).unwrap().name, "B");
        assert!(s.doc.tab(other).is_some());
        assert_eq!(s.doc.settings.tab_order, order);
        assert!(!s.can_undo());
        s.redo().unwrap();
        assert_eq!(s.doc.settings.description, "x");
        assert_eq!(s.doc.tab(tab).unwrap().name, "B");
        // The rebuilt stack of a reopened document skips them too.
        drop(s);
        let mut s2 = Session::open(&path).unwrap();
        s2.active_tab = Some(tab);
        s2.undo().unwrap();
        assert_eq!(s2.doc.tab(tab).unwrap().name, "B");
        assert_eq!(s2.doc.settings.description, "");
        let _ = std::fs::remove_dir_all(&dir);
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
