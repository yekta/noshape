//! Session undo/redo on top of the never-destructive microversion history.

use crate::{BranchId, Document, MicroversionId, Result, StoreError};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    /// State before the edit.
    before: MicroversionId,
    /// State the edit produced.
    after: MicroversionId,
    description: String,
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
            if let Some(rest) = d.strip_prefix("Undo ")
                && stack.undo.last().is_some_and(|e| e.description == rest)
                && stack.expected_head == Some(parent)
            {
                let e = stack.undo.pop().expect("non-empty");
                stack.redo.push(e);
                stack.expected_head = Some(m.id);
            } else if let Some(rest) = d.strip_prefix("Redo ")
                && stack.redo.last().is_some_and(|e| e.description == rest)
                && stack.expected_head == Some(parent)
            {
                let e = stack.redo.pop().expect("non-empty");
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
        self.record(before, after, description);
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
