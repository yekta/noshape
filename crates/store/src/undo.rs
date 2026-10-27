//! Session undo/redo on top of the never-destructive microversion history.

use crate::{BranchId, Document, MicroversionId, Result, StoreError};

/// Edits that Ctrl+Z can't revert: Onshape's tab operations (live qa5
/// 10-21: rename, reorder, delete, duplicate; new tabs and folders start
/// with an empty stack). A deleted tab comes back only by restoring from the
/// Versions and history panel. They are not recorded; earlier edits stay
/// undoable.
pub const NOT_UNDOABLE_PREFIXES: &[&str] = &[
    "Delete tab",
    "Rename tab",
    "Reorder tabs",
    "Duplicate tab",
    "Insert tab",
    "Paste tab",
    "Create folder",
    "Move to folder",
    "Unpack folder",
    "Add selection to folder",
    "Move tab to document",
    "Create Drawing of",
    "Import ",
    "Store ",
];

/// What [`UndoStack::undo_in_keeping`] / [`UndoStack::redo_in_keeping`] write
/// back for a blob: given its name and the value undo/redo would restore.
pub type KeepBlob<'a> = dyn Fn(&str, Option<Vec<u8>>) -> Option<Vec<u8>> + 'a;

/// Whether a change description is one of [`NOT_UNDOABLE_PREFIXES`].
pub fn is_not_undoable(description: &str) -> bool {
    // Edits of a tab read "<tab name> :: <edit>" (a tab may be called "Import 1").
    !description.contains(" :: ") && NOT_UNDOABLE_PREFIXES.iter().any(|p| description.starts_with(p))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    /// State before the edit.
    before: MicroversionId,
    /// State the edit produced.
    after: MicroversionId,
    description: String,
    /// Names the edit changed (None: not known yet, read from the store).
    changed: Option<Vec<String>>,
}

/// Undo/redo for one branch during one editing session.
///
/// Semantics (Onshape-like, history is never destroyed):
///
/// * Every edit made through [`UndoStack::commit`] (or registered with
///   [`UndoStack::record`]) pushes an entry `(before, after, description)`
///   and clears the redo list.
/// * [`UndoStack::undo`] pops the newest entry and creates a **new**
///   microversion on the branch whose content equals `before`, described
///   `"Undo <description>"`. The entry moves to the redo list.
/// * [`UndoStack::redo`] pops the newest redo entry and creates a new
///   microversion whose content equals `after` (`"Redo <description>"`).
/// * Undo and redo restore whole document states (all blob names), so they
///   are exact as long as every edit of the branch goes through the stack.
///   If the branch head was moved by something the stack did not see
///   (another window, a merge or restore that was not recorded), the next
///   undo/redo returns [`StoreError::UndoStale`] and clears the stack rather
///   than silently discarding that change.
/// * Undo and redo are themselves microversions ("Undo …" / "Redo …"), so
///   [`UndoStack::from_history`] rebuilds the stack of a reopened document
///   by replaying the branch's history: undo works across sessions.
#[derive(Clone, Debug)]
pub struct UndoStack {
    branch: BranchId,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    /// Branch head after the last operation known to the stack.
    expected_head: Option<MicroversionId>,
    limit: usize,
}

impl UndoStack {
    /// Default maximum number of undo steps.
    pub const DEFAULT_LIMIT: usize = 1000;

    pub fn new(branch: BranchId) -> Self {
        UndoStack {
            branch,
            undo: Vec::new(),
            redo: Vec::new(),
            expected_head: None,
            limit: Self::DEFAULT_LIMIT,
        }
    }

    /// Rebuilds the stack a session would have after making the branch's
    /// recent edits (at most `limit` undo steps), by replaying its
    /// first-parent history: ordinary microversions are edits, "Undo X" and
    /// "Redo X" move the matching entry between the lists. History made on
    /// other branches (before this branch was created) is not replayed.
    pub fn from_history(doc: &Document, branch: BranchId, limit: usize) -> Result<Self> {
        let mut stack = UndoStack::new(branch).with_limit(limit);
        // Undo/redo pairs can push and pop many times; look back further than `limit`.
        let window = limit.saturating_mul(4).max(64);
        let mut hist = doc.history_from(doc.head(branch)?, Some(window))?;
        hist.reverse();
        for m in &hist {
            let Some(&parent) = m.parents.first() else { continue };
            if m.branch != branch {
                stack.clear();
                continue;
            }
            let d = m.description.as_str();
            if is_not_undoable(d) {
                stack.expected_head = Some(m.id);
                continue;
            }
            // Scoped undo/redo may revert an entry below the newest one.
            let undo_at = d.strip_prefix("Undo ").and_then(|rest| stack.undo.iter().rposition(|e| e.description == rest));
            let redo_at = d.strip_prefix("Redo ").and_then(|rest| stack.redo.iter().rposition(|e| e.description == rest));
            if let Some(i) = undo_at.filter(|_| stack.expected_head == Some(parent)) {
                let e = stack.undo.remove(i);
                stack.redo.push(e);
                stack.expected_head = Some(m.id);
            } else if let Some(i) = redo_at.filter(|_| stack.expected_head == Some(parent)) {
                let e = stack.redo.remove(i);
                stack.undo.push(e);
                stack.expected_head = Some(m.id);
            } else {
                stack.record(parent, m.id, d);
            }
        }
        stack.expected_head = Some(doc.head(branch)?);
        Ok(stack)
    }

    /// Sets the maximum number of undo steps (oldest are dropped).
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit.max(1);
        self
    }

    pub fn branch(&self) -> BranchId {
        self.branch
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Description of the edit [`UndoStack::undo`] would revert.
    pub fn undo_description(&self) -> Option<&str> {
        self.undo.last().map(|e| e.description.as_str())
    }

    /// Description of the edit [`UndoStack::redo`] would re-apply.
    pub fn redo_description(&self) -> Option<&str> {
        self.redo.last().map(|e| e.description.as_str())
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.expected_head = None;
    }

    /// Commits an edit that can't be undone (see [`NOT_UNDOABLE_PREFIXES`]):
    /// it is not recorded; the redo list is dropped as after any edit.
    pub fn commit_not_undoable<N: AsRef<str>, B: AsRef<[u8]>>(&mut self, doc: &mut Document, changes: &[(N, Option<B>)], description: &str) -> Result<MicroversionId> {
        let before = doc.head(self.branch)?;
        let after = doc.commit(self.branch, changes, description)?;
        if self.expected_head.is_some_and(|h| h != before) {
            self.undo.clear();
        }
        self.redo.clear();
        self.expected_head = Some(after);
        Ok(after)
    }

    fn changed_names(doc: &Document, e: &Entry) -> Result<Vec<String>> {
        match &e.changed {
            Some(c) => Ok(c.clone()),
            None => Ok(doc.diff(e.before, e.after)?.into_iter().map(|c| c.name).collect()),
        }
    }

    /// Whether some edit `in_scope` (given the names it changed) can be undone.
    pub fn can_undo_in(&self, doc: &Document, in_scope: &dyn Fn(&[String]) -> bool) -> bool {
        self.undo.iter().any(|e| Self::changed_names(doc, e).is_ok_and(|c| in_scope(&c)))
    }

    pub fn can_redo_in(&self, doc: &Document, in_scope: &dyn Fn(&[String]) -> bool) -> bool {
        self.redo.iter().any(|e| Self::changed_names(doc, e).is_ok_and(|c| in_scope(&c)))
    }

    /// Onshape-style per-tab undo: reverts the newest edit `in_scope`
    /// (given the names it changed) by restoring just the names it changed
    /// (names a newer edit changed again are kept), as a new microversion
    /// "Undo <description>". Edits of other tabs are left as they are.
    pub fn undo_in(&mut self, doc: &mut Document, in_scope: &dyn Fn(&[String]) -> bool) -> Result<Option<MicroversionId>> {
        self.undo_in_keeping(doc, in_scope, &|_, v| v)
    }

    /// [`Self::undo_in`], with each restored blob passed through `keep`
    /// (name, restored value) first, so parts of the current state an edit
    /// can't take back (tab names, the tab bar) stay as they are.
    pub fn undo_in_keeping(&mut self, doc: &mut Document, in_scope: &dyn Fn(&[String]) -> bool, keep: &KeepBlob<'_>) -> Result<Option<MicroversionId>> {
        self.check_fresh(doc)?;
        let mut found = None;
        for i in (0..self.undo.len()).rev() {
            let c = Self::changed_names(doc, &self.undo[i])?;
            if in_scope(&c) {
                found = Some((i, c));
                break;
            }
        }
        let Some((i, names)) = found else { return Ok(None) };
        let mut newer: std::collections::HashSet<String> = std::collections::HashSet::new();
        for e in &self.undo[i + 1..] {
            newer.extend(Self::changed_names(doc, e)?);
        }
        let mut e = self.undo.remove(i);
        e.changed = Some(names.clone());
        let mut changes: Vec<(String, Option<Vec<u8>>)> = Vec::new();
        for n in names.into_iter().filter(|n| !newer.contains(n)) {
            let v = keep(&n, doc.read(e.before, &n)?);
            changes.push((n, v));
        }
        let mv = doc.commit(self.branch, &changes, &format!("Undo {}", e.description))?;
        self.expected_head = Some(mv);
        self.redo.push(e);
        Ok(Some(mv))
    }

    /// Re-applies the newest undone edit `in_scope` (names it changed set
    /// back to what the edit produced).
    pub fn redo_in(&mut self, doc: &mut Document, in_scope: &dyn Fn(&[String]) -> bool) -> Result<Option<MicroversionId>> {
        self.redo_in_keeping(doc, in_scope, &|_, v| v)
    }

    /// [`Self::redo_in`] with `keep` as in [`Self::undo_in_keeping`].
    pub fn redo_in_keeping(&mut self, doc: &mut Document, in_scope: &dyn Fn(&[String]) -> bool, keep: &KeepBlob<'_>) -> Result<Option<MicroversionId>> {
        self.check_fresh(doc)?;
        let mut found = None;
        for i in (0..self.redo.len()).rev() {
            let c = Self::changed_names(doc, &self.redo[i])?;
            if in_scope(&c) {
                found = Some((i, c));
                break;
            }
        }
        let Some((i, names)) = found else { return Ok(None) };
        let e = self.redo.remove(i);
        let mut changes: Vec<(String, Option<Vec<u8>>)> = Vec::new();
        for n in names {
            let v = keep(&n, doc.read(e.after, &n)?);
            changes.push((n, v));
        }
        let mv = doc.commit(self.branch, &changes, &format!("Redo {}", e.description))?;
        self.expected_head = Some(mv);
        self.undo.push(e);
        Ok(Some(mv))
    }

    /// Commits through [`Document::commit`] and records the edit. A commit
    /// that changes nothing is not recorded.
    pub fn commit<N: AsRef<str>, B: AsRef<[u8]>>(
        &mut self,
        doc: &mut Document,
        changes: &[(N, Option<B>)],
        description: &str,
    ) -> Result<MicroversionId> {
        let before = doc.head(self.branch)?;
        let after = doc.commit(self.branch, changes, description)?;
        // A commit that changed nothing makes no microversion and no undo
        // step; the previous step (whose `after` is that same head) keeps
        // its names (qa UNDO-NOOP-SHIFT: they were overwritten with the
        // empty diff, and undo then skipped the edit).
        if before == after {
            return Ok(after);
        }
        self.record(before, after, description);
        if let Some(e) = self.undo.last_mut().filter(|e| e.after == after) {
            e.changed = Some(doc.diff(before, after)?.into_iter().map(|c| c.name).collect());
        }
        Ok(after)
    }

    /// Records an edit performed by other means (e.g. a restore or merge
    /// into this branch) so it can be undone. `before` is the branch head
    /// before the edit and `after` the head it produced. If `before` is not
    /// the head the stack last saw, older entries are discarded.
    pub fn record(&mut self, before: MicroversionId, after: MicroversionId, description: &str) {
        if before == after {
            return;
        }
        if self.expected_head.is_some_and(|h| h != before) {
            self.undo.clear();
        }
        self.redo.clear();
        self.undo.push(Entry {
            before,
            after,
            description: description.to_string(),
            changed: None,
        });
        if self.undo.len() > self.limit {
            let excess = self.undo.len() - self.limit;
            self.undo.drain(..excess);
        }
        self.expected_head = Some(after);
    }

    fn check_fresh(&mut self, doc: &Document) -> Result<()> {
        let head = doc.head(self.branch)?;
        if self.expected_head.is_some_and(|h| h != head) {
            self.clear();
            return Err(StoreError::UndoStale);
        }
        Ok(())
    }

    /// Reverts the newest edit by committing its `before` state. Returns the
    /// new microversion, or `None` if there is nothing to undo.
    pub fn undo(&mut self, doc: &mut Document) -> Result<Option<MicroversionId>> {
        if self.undo.is_empty() {
            return Ok(None);
        }
        self.check_fresh(doc)?;
        let e = self.undo.pop().expect("non-empty");
        let mv = doc.restore_with_description(
            self.branch,
            e.before,
            &format!("Undo {}", e.description),
        )?;
        self.expected_head = Some(mv);
        self.redo.push(e);
        Ok(Some(mv))
    }

    /// Re-applies the newest undone edit. Returns the new microversion, or
    /// `None` if there is nothing to redo.
    pub fn redo(&mut self, doc: &mut Document) -> Result<Option<MicroversionId>> {
        if self.redo.is_empty() {
            return Ok(None);
        }
        self.check_fresh(doc)?;
        let e = self.redo.pop().expect("non-empty");
        let mv =
            doc.restore_with_description(self.branch, e.after, &format!("Redo {}", e.description))?;
        self.expected_head = Some(mv);
        self.undo.push(e);
        Ok(Some(mv))
    }
}
