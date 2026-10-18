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
    Error(String),
}

pub struct DocModel {
    pub session: Session,
    /// Regeneration caches of Part Studios used as Derived sources.
    pub studio_cache: crate::derived::StudioCache,
}

impl EventEmitter<DocEvent> for DocModel {}

impl DocModel {
    pub fn new(session: Session) -> Self {
        Self { session, studio_cache: Default::default() }
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
        let tab_name = self.session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
        let r = f(&mut self.session.doc.tab_mut(tab)?.content);
        if let Err(e) = self.session.commit(&[tab], false, &format!("{tab_name} :: {description}")) {
            cx.emit(DocEvent::Error(format!("Could not save change: {e:#}")));
        }
        cx.emit(DocEvent::TabChanged(tab));
        cx.notify();
        Some(r)
    }

    /// Structural document edit (tabs added/removed/renamed).
    pub fn edit_document(&mut self, description: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut noshape_model::Document)) {
        if self.session.read_only() {
            return;
        }
        f(&mut self.session.doc);
        if let Err(e) = self.session.commit_all(description) {
            cx.emit(DocEvent::Error(format!("Could not save change: {e:#}")));
        }
        cx.emit(DocEvent::TabsChanged);
        cx.notify();
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        match self.session.undo() {
            Ok(true) => cx.emit(DocEvent::Reloaded),
            Ok(false) => {}
            Err(e) => cx.emit(DocEvent::Error(format!("Undo failed: {e:#}"))),
        }
        cx.notify();
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        match self.session.redo() {
            Ok(true) => cx.emit(DocEvent::Reloaded),
            Ok(false) => {}
            Err(e) => cx.emit(DocEvent::Error(format!("Redo failed: {e:#}"))),
        }
        cx.notify();
    }

    pub fn reloaded(&mut self, cx: &mut Context<Self>) {
        cx.emit(DocEvent::Reloaded);
        cx.emit(DocEvent::HistoryChanged);
        cx.notify();
    }
}
