//! Shared document entity: wraps the [`Session`] and broadcasts changes to
//! every view of the document (tabs, feature lists, version graph).

use crate::session::Session;
use gpui_kit::*;
use noshape_model::document::{TabContent, TabId};
use noshape_model::partstudio::PartStudio;

#[derive(Clone, Debug)]
pub enum DocEvent {
    /// A tab's content changed (regenerate).
    TabChanged(TabId),
    /// Tabs added/removed/renamed/reordered.
    TabsChanged,
    /// The whole document was replaced (undo, branch switch, history view).
    Reloaded,
    /// Versions/branches changed.
    HistoryChanged,
    /// The workspace units changed (from `old`); open dialogs stay open.
    UnitsChanged { old: noshape_units::UnitPrefs },
    Error(String),
    /// A user-facing notice (not an error), e.g. a merge result.
    Info(String),
    /// A window action an agent call asked for (live mode: Switch to
    /// reference and the like).
    Ui(noshape_ops::UiRequest),
}

pub struct DocModel {
    pub session: Session,
    /// Regeneration caches of Part Studios used as Derived sources.
    pub studio_cache: crate::derived::StudioCache,
    /// A tab edit applied but not committed yet, with its description: an
    /// assembly Insert session (live undoes all its instances in one step).
    /// Any other edit, undo or redo commits it first.
    pending: PendingEdit,
}

/// An applied, uncommitted tab edit and its description (see
/// [`DocModel::edit_tab_deferred`]).
#[derive(Default)]
struct PendingEdit(Option<(TabId, String)>);

impl PendingEdit {
    /// Record an applied edit of `tab`: it joins the pending one (whose
    /// description stays), or a pending edit of another tab is committed first.
    fn note(&mut self, session: &mut Session, tab: TabId, description: impl FnOnce() -> String) -> anyhow::Result<()> {
        if self.0.as_ref().is_some_and(|(t, _)| *t != tab) {
            self.flush(session)?;
        }
        if self.0.is_none() {
            self.0 = Some((tab, description()));
        }
        Ok(())
    }

    /// Commit the pending edit (if any) as one microversion.
    fn flush(&mut self, session: &mut Session) -> anyhow::Result<()> {
        if let Some((tab, desc)) = self.0.take() {
            session.commit(&[tab], false, &desc)?;
        }
        Ok(())
    }
}

impl EventEmitter<DocEvent> for DocModel {}

impl DocModel {
    /// The app's model of an opened document: every tab starts with empty
    /// undo/redo stacks (Onshape's page load; the history stays).
    pub fn new(mut session: Session) -> Self {
        session.forget_undo();
        Self { session, studio_cache: Default::default(), pending: PendingEdit::default() }
    }

    /// Provider giving Derived features access to the other Part Studios.
    pub fn derived_provider(&self) -> std::sync::Arc<crate::derived::DocStudios> {
        crate::derived::DocStudios::new(self.session.doc.clone(), self.studio_cache.clone())
    }

    /// The document's Feature Studios, compiled (cached by source); also
    /// registers their custom feature schemas.
    pub fn custom_features(&self) -> std::sync::Arc<noshape_model::custom::DocumentScripts> {
        noshape_model::custom::DocumentScripts::from_document(&self.session.doc)
    }

    pub fn name(&self) -> String {
        self.session.name()
    }

    pub fn studio(&self, tab: TabId) -> Option<&PartStudio> {
        match &self.session.doc.tab(tab)?.content {
            TabContent::PartStudio(ps) => Some(ps),
            _ => None,
        }
    }

    /// Edit a Part Studio and commit the change as one microversion.
    pub fn edit_studio<R>(&mut self, tab: TabId, description: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut PartStudio) -> R) -> Option<R> {
        self.flush_pending(cx);
        if self.session.read_only() {
            cx.emit(DocEvent::Error("This is a read-only view of history. Return to the workspace to edit.".into()));
            return None;
        }
        let tab_name = self.session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
        let r = match &mut self.session.doc.tab_mut(tab)?.content {
            TabContent::PartStudio(ps) => f(ps),
            _ => return None,
        };
        let desc = format!("{tab_name} :: {description}");
        if let Err(e) = self.session.commit(&[tab], false, &desc) {
            cx.emit(DocEvent::Error(format!("Could not save change: {e:#}")));
        }
        cx.emit(DocEvent::TabChanged(tab));
        cx.notify();
        Some(r)
    }

    /// Edit any tab content (assembly, drawing...).
    pub fn edit_tab<R>(&mut self, tab: TabId, description: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut TabContent) -> R) -> Option<R> {
        if self.session.read_only() {
            return None;
        }
        self.flush_pending(cx);
        let tab_name = self.session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
        let r = f(&mut self.session.doc.tab_mut(tab)?.content);
        if let Err(e) = self.session.commit(&[tab], false, &format!("{tab_name} :: {description}")) {
            cx.emit(DocEvent::Error(format!("Could not save change: {e:#}")));
        }
        cx.emit(DocEvent::TabChanged(tab));
        cx.notify();
        Some(r)
    }

    /// Edit a tab without committing: the change joins the pending edit
    /// (the first one's description is kept) until [`Self::flush_pending`].
    pub fn edit_tab_deferred<R>(&mut self, tab: TabId, description: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut TabContent) -> R) -> Option<R> {
        if self.session.read_only() {
            return None;
        }
        let r = f(&mut self.session.doc.tab_mut(tab)?.content);
        let tab_name = self.session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
        if let Err(e) = self.pending.note(&mut self.session, tab, || format!("{tab_name} :: {description}")) {
            cx.emit(DocEvent::Error(format!("Could not save change: {e:#}")));
        }
        cx.emit(DocEvent::TabChanged(tab));
        cx.notify();
        Some(r)
    }

    /// Commit the pending edit (if any) as one microversion.
    pub fn flush_pending(&mut self, cx: &mut Context<Self>) {
        if let Err(e) = self.pending.flush(&mut self.session) {
            cx.emit(DocEvent::Error(format!("Could not save change: {e:#}")));
        }
    }

    /// An edit is waiting to be committed (see [`Self::edit_tab_deferred`]).
    pub fn has_pending(&self) -> bool {
        self.pending.0.is_some()
    }

    /// Undo is possible (a pending edit counts: Undo commits then reverts it).
    pub fn can_undo(&self) -> bool {
        self.pending.0.is_some() || self.session.can_undo()
    }

    /// Redo is possible (not over a pending edit, whose commit clears it).
    pub fn can_redo(&self) -> bool {
        self.pending.0.is_none() && self.session.can_redo()
    }

    /// Structural document edit (tabs added/removed/renamed).
    pub fn edit_document(&mut self, description: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut noshape_model::Document)) {
        if self.session.read_only() {
            return;
        }
        self.flush_pending(cx);
        f(&mut self.session.doc);
        if let Err(e) = self.session.commit_all(description) {
            cx.emit(DocEvent::Error(format!("Could not save change: {e:#}")));
        }
        cx.emit(DocEvent::TabsChanged);
        cx.notify();
    }

    /// A structural edit Ctrl+Z can't undo (Onshape: deleting a tab; the
    /// Versions and history panel restores it).
    pub fn edit_document_not_undoable(&mut self, description: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut noshape_model::Document)) {
        if self.session.read_only() {
            return;
        }
        self.flush_pending(cx);
        f(&mut self.session.doc);
        if let Err(e) = self.session.commit_all_not_undoable(description) {
            cx.emit(DocEvent::Error(format!("Could not save change: {e:#}")));
        }
        cx.emit(DocEvent::TabsChanged);
        cx.notify();
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        self.flush_pending(cx);
        match self.session.undo() {
            Ok(true) => cx.emit(DocEvent::Reloaded),
            Ok(false) => {}
            Err(e) => cx.emit(DocEvent::Error(format!("Undo failed: {e:#}"))),
        }
        cx.notify();
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        self.flush_pending(cx);
        match self.session.redo() {
            Ok(true) => cx.emit(DocEvent::Reloaded),
            Ok(false) => {}
            Err(e) => cx.emit(DocEvent::Error(format!("Redo failed: {e:#}"))),
        }
        cx.notify();
    }

    /// Another process writing this file (the document open in a second
    /// window, an agent): its edits show here within ~1.5 s, as live shows
    /// another writer's edit in an open tab (qa TWO-PROCESS-LOST-EDIT). The
    /// poll is one query while nothing changed; it stops with the document.
    pub fn watch_external(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
                let alive = this.update(cx, |d: &mut DocModel, cx| {
                    // An edit waiting to be committed merges on its commit instead.
                    if d.has_pending() {
                        return;
                    }
                    match d.session.sync_external() {
                        Ok(true) => d.reloaded(cx),
                        Ok(false) => {}
                        Err(e) => log::warn!("checking for other writers: {e:#}"),
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn reloaded(&mut self, cx: &mut Context<Self>) {
        self.pending = PendingEdit::default();
        cx.emit(DocEvent::Reloaded);
        cx.emit(DocEvent::HistoryChanged);
        cx.notify();
    }
}

impl Drop for DocModel {
    /// A pending edit is saved even if the app quits mid-session.
    fn drop(&mut self) {
        let _ = self.pending.flush(&mut self.session);
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingEdit, Session};
    use noshape_model::Document;

    /// Live: a reopened document has nothing to undo, though its history
    /// (and the CLI's undo) keeps the edits.
    #[test]
    fn a_reopened_document_starts_with_nothing_to_undo() {
        let dir = std::env::temp_dir().join(format!("noshape-reopen-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("r.noshape");
        let _ = std::fs::remove_file(&path);
        let mut s = Session::create(&path, Document::new()).unwrap();
        s.doc.settings.description = "Edited".into();
        s.commit(&[], true, "Edit description").unwrap();
        drop(s);
        let cli = Session::open(&path).unwrap();
        assert!(cli.can_undo());
        let app = super::DocModel::new(Session::open(&path).unwrap());
        assert!(!app.can_undo() && !app.can_redo());
        assert_eq!(app.session.doc.settings.description, "Edited");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// qa5 UNDO1: an Insert session's edits (insert, drop) are one undo step.
    #[test]
    fn deferred_edits_are_one_undo_step() {
        let dir = std::env::temp_dir().join(format!("noshape-pending-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("p.noshape");
        let _ = std::fs::remove_file(&path);
        let mut s = Session::create(&path, Document::new()).unwrap();
        let tab = s.doc.tabs[0].id;
        // A content edit (a rename alone isn't undoable, qa5 TABS5).
        let rollback = |s: &mut Session, at: usize| {
            if let Some(noshape_model::TabContent::PartStudio(ps)) = s.doc.tab_mut(tab).map(|t| &mut t.content) {
                ps.rollback = Some(at);
            }
        };
        let mut p = PendingEdit::default();
        rollback(&mut s, 0);
        p.note(&mut s, tab, || "Insert".into()).unwrap();
        rollback(&mut s, 1);
        p.note(&mut s, tab, || "Place".into()).unwrap();
        assert!(!s.can_undo());
        p.flush(&mut s).unwrap();
        assert!(s.can_undo());
        s.undo().unwrap();
        assert!(matches!(&s.doc.tab(tab).unwrap().content, noshape_model::TabContent::PartStudio(ps) if ps.rollback.is_none()));
        assert!(!s.can_undo());
        let _ = std::fs::remove_file(&path);
    }
}
