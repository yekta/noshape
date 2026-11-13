//! Document window: title bar, active tab content and the bottom tab bar.

use crate::actions::*;
use crate::assembly_view::AssemblyView;
use crate::doc_model::{DocEvent, DocModel};
use crate::gpu::Gpu;
use crate::history_panel::HistoryPanel;
use crate::part_studio::PartStudioView;
use crate::variable_studio::VariableStudioView;
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use crate::ui::{TipExt as _, TipSide};
use noshape_model::document::{TabContent, TabId};
use noshape_ops::docedit;
use std::collections::HashMap;

// What an agent's call asks of the window (live mode).
#[path = "agent_window.rs"]
mod agent_window;

enum TabView {
    Variables(Entity<VariableStudioView>),
    PartStudio(Entity<PartStudioView>),
    Drawing(Entity<crate::drawing_view::DrawingEditor>),
    Assembly(Entity<AssemblyView>),
    FeatureStudio(Entity<crate::feature_studio::FeatureStudioView>),
    MaterialLibrary(Entity<crate::material_library_view::MaterialLibraryView>),
    /// Imported / stored files (see `file_tab_view`).
    File(Entity<crate::file_tab_view::FileTabView>),
}

pub struct Workspace {
    /// Tabs with a reference to an older version of another document (the
    /// tab's "Newer versions of linked documents available" icon), the
    /// references they were found from, and the check running.
    behind: std::collections::HashSet<TabId>,
    behind_refs: Vec<(TabId, crate::derived::ExternalRef)>,
    behind_task: Option<Task<()>>,
    pub doc: Entity<DocModel>,
    gpu: Gpu,
    active: Option<TabId>,
    views: HashMap<TabId, TabView>,
    focus: FocusHandle,
    /// Cached chrome (navbar, rail, tab bar): see `cached_slice`.
    slices: Option<WsSlices>,
    /// The navbar logo (Onshape's documents link) as a Tab stop: where Tab
    /// past a feature dialog's last control lands (live psb 10-19 h).
    logo_focus: FocusHandle,
    /// The top bar's `{ }` button: where Tab goes after a dialog's last
    /// control, or from the page (live: Share, in the same spot).
    notices_focus: FocusHandle,
    /// The navbar and the tab bar: keys pressed while their buttons have
    /// the focus still work as shortcuts (`forward_chrome_key`).
    chrome_focus: [FocusHandle; 2],
    history: Option<Entity<HistoryPanel>>,
    /// The Release drafts panel (left rail).
    release_drafts: Option<Entity<crate::release_dialog::ReleaseDraftsPanel>>,
    /// Previously active tabs, most recent first (Ctrl+Space).
    recent: Vec<TabId>,
    /// The Ctrl+Space tab switcher while Ctrl is held: the session's tabs,
    /// most recently used first (the active one), and the selected one.
    switcher: Option<(Vec<TabId>, usize)>,
    /// Tab manager (Alt+T), docked left like Onshape's.
    tab_manager: Option<(Entity<crate::tab_manager::TabManager>, Subscription)>,
    /// Pictures for the tab hover card.
    tab_thumbs: Entity<crate::tab_thumbs::TabThumbs>,
    /// The pointer is over the navbar, which shows Onshape's selection-count
    /// cursor too (`.navbar` gets the canvas's selection cursor).
    navbar_hovered: bool,
    /// The tab strip's scroll, and whether the active tab still has to be
    /// scrolled into view (live: activating a hidden tab, opening the Tab
    /// manager).
    tab_scroll: ScrollHandle,
    reveal_active_tab: bool,
    cursor_sync: crate::cursor::CursorSync,
    /// Left rail › Document notes or Performance.
    doc_panel: Option<(DocPanel, Vec<Subscription>)>,
    /// Tab being renamed (tab › Rename, double-click).
    renaming_tab: Option<(TabId, Entity<gpui_kit::component::input::InputState>, Subscription)>,
    /// The navbar's document name being renamed inline (a click on it, ☰ › Rename document).
    renaming_doc: Option<(Entity<gpui_kit::component::input::InputState>, Subscription, std::time::Instant)>,
    /// A tab being dragged along the bar (live qa5 TABS1).
    tab_gap: Option<TabGap>,
    /// ☰ › Workspace units… panel.
    units_panel: Option<(Entity<crate::doc_dialogs::WorkspaceUnits>, Subscription)>,
    /// The tab-bar folder being shown (None: all tabs at the root).
    open_folder: Option<TabId>,
    /// Onshape's "FeatureScript notices" panel, under the tab bar.
    fs_notices: Option<(Entity<crate::fs_notices::FsNoticesPanel>, Subscription)>,
    /// The Notifications bell's panel is open.
    notifications_open: bool,
    /// Onshape's "Search Feature Studios" pane, under the tab bar.
    fs_search: Option<(Entity<crate::fs_search::FsSearchPane>, Subscription)>,
    /// Create Drawing dialog (see `drawing_create`).
    create_drawing: Option<(Entity<crate::drawing_create::CreateDrawingDialog>, Subscription)>,
    _thumbnailer: Entity<crate::start_thumbnail::Thumbnailer>,
    /// The last document message, shown for tabs that have no message bubble
    /// of their own (Variable / Feature Studio, Material Library, file tabs).
    toast: Option<(crate::ui::AlertKind, String)>,
    _subs: Vec<Subscription>,
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Workspace {
    pub fn new(doc: Entity<DocModel>, gpu: Gpu, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.subscribe_in(&doc, window, |this: &mut Self, _, ev: &DocEvent, window, cx| match ev {
            DocEvent::TabsChanged | DocEvent::Reloaded | DocEvent::UnitsChanged { .. } => {
                // Register custom feature schemas of the (new) Feature Studios (in the background).
                crate::doc_model::compile_custom_features(&this.doc, cx);
                this.sync_tabs(window, cx);
                this.refresh_behind(false, cx);
                cx.notify();
            }
            // Tabs without their own message display show Onshape's bubble here.
            DocEvent::Error(msg) | DocEvent::Info(msg) => {
                let info = matches!(ev, DocEvent::Info(_));
                let kind = if info { crate::ui::AlertKind::Info } else { crate::ui::AlertKind::Error };
                this.toast = Some((kind, msg.clone()));
                // An info bubble hides by itself after ≈4.4 s (live TOAST-AUTOHIDE); an error stays until ×.
                if info {
                    let msg = msg.clone();
                    // A notice with a link stays longer (live 2351: the Merge toast ≈ 35 s).
                    let ms = if this.doc.read(cx).notice_link(&msg).is_some() { 35_000 } else { 4400 };
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(std::time::Duration::from_millis(ms)).await;
                        let _ = this.update(cx, |this, cx| {
                            if this.toast.as_ref().is_some_and(|(_, m)| *m == msg) {
                                this.toast = None;
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                }
                cx.notify();
            }
            // An agent's Switch to reference / Open Feature Studio in linked
            // document (live mode), as the menu items do.
            DocEvent::Ui(req) => this.agent_request(req.clone(), window, cx),
            _ => {
                this.refresh_behind(false, cx);
                cx.notify()
            }
        })];
        // When the focused element goes away (a menu, a prompt, a closed
        // panel), shortcuts keep working: focus returns to the active tab.
        let focus_lost = cx.on_focus_lost(window, |this: &mut Self, window, cx| this.focus_active(window, cx));
        let active = doc.read(cx).session.doc.ordered_tabs().first().map(|t| t.id);
        noshape_render::perf::milestone("workspace: subscribed");
        // Compiled in the background, not before the window's first frame (~50 ms of a document with custom features).
        crate::doc_model::compile_custom_features(&doc, cx);
        noshape_render::perf::milestone("workspace: custom features compiling");
        let thumbnailer = cx.new(|cx| crate::start_thumbnail::Thumbnailer::new(doc.clone(), gpu.clone(), cx));
        let tab_thumbs = cx.new(|_| crate::tab_thumbs::TabThumbs::new(doc.clone()));
        noshape_render::perf::milestone("workspace: thumbnailers");
        let mut this = Self { behind: Default::default(), behind_refs: Vec::new(), behind_task: None, tab_thumbs, tab_scroll: ScrollHandle::new(), reveal_active_tab: true, navbar_hovered: false, cursor_sync: Default::default(), doc, gpu, active, views: HashMap::new(), focus: cx.focus_handle(), slices: None, logo_focus: cx.focus_handle().tab_stop(true), notices_focus: cx.focus_handle().tab_stop(true), chrome_focus: [cx.focus_handle(), cx.focus_handle()], history: None, release_drafts: None, recent: Vec::new(), switcher: None, tab_manager: None, doc_panel: None, renaming_tab: None, renaming_doc: None, tab_gap: None, units_panel: None, open_folder: None, fs_notices: None, notifications_open: false, fs_search: None, create_drawing: None, _thumbnailer: thumbnailer, toast: None, _subs: subs };
        this._subs.push(focus_lost);
        // Another document may have a new version when the window comes back.
        this._subs.push(cx.observe_window_activation(window, |this: &mut Self, window, cx| {
            if window.is_window_active() {
                this.refresh_behind(true, cx);
            }
        }));
        this.refresh_behind(true, cx);
        noshape_render::perf::milestone("workspace: behind refreshed");
        this.sync_tabs(window, cx);
        noshape_render::perf::milestone("workspace: tabs synced");
        // Focus the active tab view so its shortcuts work immediately: the one active by then (Edit in context from
        // another document activates its Part Studio as the window opens).
        if this.active.is_some() {
            cx.defer_in(window, move |this: &mut Self, window, cx| {
                if let Some(a) = this.active {
                    this.activate(a, window, cx);
                }
            });
        }
        if crate::smoke::enabled() {
            crate::smoke::start(window, cx);
        }
        if crate::ui_script::enabled() {
            let weak = cx.entity().downgrade();
            noshape_app_core::ui_script::register_page(window.window_handle().window_id(), move |window, cx| weak.update(cx, |w, cx| crate::ui_script::window_state(w, window, cx)).unwrap_or(serde_json::Value::Null));
            if crate::ui_script::page_start().is_none() {
                crate::ui_script::start(window, cx);
            }
        }
        // Live mode: agents' calls see this window's tab, cameras, selection
        // and, in a Part Studio, its dialog, lists and feature list.
        let me = cx.entity().downgrade();
        let handle = window.window_handle();
        this.doc.update(cx, |dm, _| {
            dm.window_snapshot = Some(std::rc::Rc::new(move |cx: &mut App| {
                let Some(ws) = me.upgrade() else { return Default::default() };
                let mut s = ws.read(cx).window_snapshot(cx);
                s.ui = handle.update(cx, |_, window, cx| ws.read(cx).active_ui_state(window, cx)).ok().flatten();
                s
            }))
        });
        this
    }

    /// The active Part Studio's on-screen state as the interaction suite
    /// records it (tests/ui-parity/README.md): focus, dialog, open list,
    /// selection, feature list, message bar.
    fn active_ui_state(&self, window: &Window, cx: &App) -> Option<serde_json::Value> {
        match self.views.get(&self.active?)? {
            TabView::PartStudio(v) => Some(v.read(cx).ui_state(window, cx)),
            _ => None,
        }
    }

    /// What this window shows, for live-mode calls (`DocModel::window_snapshot`).
    fn window_snapshot(&self, cx: &App) -> noshape_ops::window::WindowSnapshot {
        let mut s = noshape_ops::window::WindowSnapshot { active_tab: self.active, ..Default::default() };
        for (tab, view) in &self.views {
            let viewport = match view {
                TabView::PartStudio(v) => {
                    let v = v.read(cx);
                    s.selection.insert(*tab, v.selection.iter().map(noshape_ops::refs::to_id).collect());
                    v.viewport.clone()
                }
                TabView::Assembly(v) => {
                    let v = v.read(cx);
                    s.selection.insert(*tab, v.selected_instances().iter().map(|i| format!("inst:{}", i.0)).collect());
                    v.viewport.clone()
                }
                TabView::Drawing(v) => {
                    // As the drawing tools name them (drawing_delete's views, annotations, tables…).
                    use noshape_scene::drawing_tools::Sel;
                    let v = v.read(cx);
                    s.sheets.insert(*tab, noshape_ops::window::SheetView { zoom: v.zoom, center: v.center, size: v.size_px() });
                    let ids = v.selection.iter().filter_map(|s| match s {
                        Sel::View(id) => Some(format!("view:{}", id.0)),
                        Sel::Annotation(id) => Some(format!("annotation:{id}")),
                        Sel::Table(id) => Some(format!("table:{id}")),
                        Sel::Break(view, i) => Some(format!("break:{}.{i}", view.0)),
                        Sel::Sketch(id) => Some(format!("sketch:{id}")),
                        Sel::Hatch(id) => Some(format!("hatch:{id}")),
                        Sel::Image(id) => Some(format!("image:{id}")),
                        _ => None,
                    });
                    s.selection.insert(*tab, ids.collect());
                    continue;
                }
                _ => continue,
            };
            let vp = viewport.read(cx);
            s.cameras.insert(*tab, (vp.camera, vp.size()));
        }
        s
    }

    fn sync_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tabs: Vec<(TabId, &'static str)> = self.doc.read(cx).session.doc.ordered_tabs().iter().map(|t| (t.id, kind_of(&t.content))).collect();
        self.views.retain(|id, _| tabs.iter().any(|(t, _)| t == id));
        if self.active.is_none_or(|a| !tabs.iter().any(|(t, _)| *t == a)) {
            // A version opened without the tab that was showing: its first tab, and live's notice.
            if self.active.is_some() && self.doc.read(cx).session.viewing_version().is_some() {
                self.doc.update(cx, |_, cx| cx.emit(DocEvent::Info("Can't open tab. The tab does not exist in this document version.".into())));
            }
            self.active = tabs.first().map(|t| t.0);
            // The session follows (undo scope, Versions and history's wrench):
            // another workspace may lack the tab that was open.
            let active = self.active;
            self.doc.update(cx, |d, _| d.session.active_tab = active);
            if let Some(h) = &self.history {
                h.update(cx, |_, cx| cx.notify());
            }
        }
        // Views are created lazily for the active tab.
        if let Some(active) = self.active {
            self.ensure_view(active, window, cx);
        }
    }

    fn ensure_view(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        if self.views.contains_key(&tab) {
            return;
        }
        let kind = self.doc.read(cx).session.doc.tab(tab).map(|t| kind_of(&t.content));
        let view = match kind {
            Some("partstudio") => {
                let doc = self.doc.clone();
                let gpu = self.gpu.clone();
                let v = cx.new(|cx| PartStudioView::new(doc, tab, gpu, window, cx));
                noshape_render::perf::milestone("workspace: Part Studio view built");
                // In-context banner › Go to assembly.
                self._subs.push(cx.subscribe_in(&v, window, |this: &mut Self, _, ev: &crate::part_studio::PartStudioEvent, window, cx| match ev {
                    crate::part_studio::PartStudioEvent::ActivateTab(t) => this.activate(*t, window, cx),
                    crate::part_studio::PartStudioEvent::GoToOtherAssembly(o, t) => {
                        if let Err(e) = crate::files::open_document_at(o.path.clone(), *t, this.gpu.clone(), cx) {
                            log::warn!("Go to assembly: {}: {e}", o.path.display());
                        }
                    }
                    crate::part_studio::PartStudioEvent::VersionAndUpdate(o, t) => this.version_and_update(o.clone(), *t, cx),
                    crate::part_studio::PartStudioEvent::InsertInAssembly { assembly, studio, parts, sketches, at } => {
                        let (assembly, studio, parts, sketches, at) = (*assembly, *studio, parts.clone(), sketches.clone(), *at);
                        this.activate(assembly, window, cx);
                        if let Some(TabView::Assembly(v)) = this.views.get(&assembly) {
                            v.update(cx, |v, cx| v.insert_from_context(studio, parts, sketches, at, cx));
                        }
                    }
                }));
                TabView::PartStudio(v)
            }
            Some("assembly") => {
                let doc = self.doc.clone();
                let gpu = self.gpu.clone();
                let v = cx.new(|cx| AssemblyView::new(doc, tab, gpu, window, cx));
                // "Switch to Part Studio / Assembly" from the instance context menu.
                self._subs.push(cx.subscribe_in(&v, window, |this: &mut Self, _, ev: &crate::assembly_view::AssemblyViewEvent, window, cx| match ev {
                    crate::assembly_view::AssemblyViewEvent::ActivateTab(t) => this.activate(*t, window, cx),
                    crate::assembly_view::AssemblyViewEvent::ShowPart(t, body) => {
                        this.activate(*t, window, cx);
                        if let Some(TabView::PartStudio(v)) = this.views.get(t) {
                            v.update(cx, |v, cx| v.select_part(body.clone(), cx));
                        }
                    }
                    crate::assembly_view::AssemblyViewEvent::EditInContext(t, ctx) => {
                        this.activate(*t, window, cx);
                        if let Some(TabView::PartStudio(v)) = this.views.get(t) {
                            let ctx = (**ctx).clone();
                            v.update(cx, |v, cx| v.set_assembly_context(Some(ctx), cx));
                        }
                    }
                    crate::assembly_view::AssemblyViewEvent::EditInContextOther(o) => {
                        if let Err(e) = crate::files::open_in_context(o.path.clone(), o.studio, o.ctx.clone(), o.install.clone(), this.gpu.clone(), cx) {
                            log::warn!("Edit in context: {}: {e}", o.path.display());
                        }
                    }
                }));
                TabView::Assembly(v)
            }
            Some("drawing") => {
                let (doc, gpu) = (self.doc.clone(), self.gpu.clone());
                TabView::Drawing(cx.new(|cx| crate::drawing_view::DrawingEditor::new(doc, tab, gpu, window, cx)))
            }
            Some("variables") => {
                let doc = self.doc.clone();
                TabView::Variables(cx.new(|cx| VariableStudioView::new(doc, tab, window, cx)))
            }
            Some("featurestudio") => {
                let doc = self.doc.clone();
                let v = cx.new(|cx| crate::feature_studio::FeatureStudioView::new(doc, tab, window, cx));
                self._subs.push(cx.subscribe_in(&v, window, |this: &mut Self, _, ev: &crate::feature_studio::FeatureStudioEvent, window, cx| match ev {
                    crate::feature_studio::FeatureStudioEvent::OpenSearch => this.open_fs_search(window, cx),
                    crate::feature_studio::FeatureStudioEvent::NoticesChanged => {
                        if let Some((p, _)) = &this.fs_notices {
                            p.update(cx, |p, cx| p.refresh(cx));
                        }
                        cx.notify();
                    }
                    crate::feature_studio::FeatureStudioEvent::OpenStd { module, line, column } => {
                        if let Err(e) = crate::std_source::open(module, *line, *column, this.gpu.clone(), cx) {
                            log::warn!("Could not open the standard library: {e}");
                        }
                    }
                    crate::feature_studio::FeatureStudioEvent::FindSymbol { name, at, rename } => {
                        this.open_fs_search(window, cx);
                        if let Some((pane, _)) = &this.fs_search {
                            let (name, at, rename) = (name.clone(), at.clone(), *rename);
                            pane.update(cx, |p, cx| p.search_symbol(name, at, rename, window, cx));
                        }
                    }
                }));
                TabView::FeatureStudio(v)
            }
            Some("materiallibrary") => {
                let doc = self.doc.clone();
                TabView::MaterialLibrary(cx.new(|cx| crate::material_library_view::MaterialLibraryView::new(doc, tab, window, cx)))
            }
            Some(k) if k.starts_with("file") => {
                let doc = self.doc.clone();
                TabView::File(cx.new(|cx| crate::file_tab_view::FileTabView::new(doc, tab, cx)))
            }
            _ => return,
        };
        self.views.insert(tab, view);
    }

    /// Show the Feature Studio tab named `name` with the caret at a 1-based
    /// line and column (std source opened by ctrl+click).
    pub fn goto_studio(&mut self, name: &str, line: usize, column: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.doc.read(cx).session.doc.tabs.iter().find(|t| t.name == name).map(|t| t.id) else { return };
        self.activate(tab, window, cx);
        if let Some(TabView::FeatureStudio(v)) = self.views.get(&tab) {
            v.update(cx, |v, cx| v.goto_line(line, column, window, cx));
        }
    }

    /// Tab › Show code: the Part Studio shown, its code panel docked on the right (live part-studio/615).
    fn show_code(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        self.activate(tab, window, cx);
        if let Some(TabView::PartStudio(v)) = self.views.get(&tab) {
            v.update(cx, |v, cx| v.show_code(window, cx));
        }
    }

    pub(crate) fn activate(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        self.reveal_active_tab = true;
        if let Some(prev) = self.active.filter(|p| *p != tab) {
            self.recent.retain(|t| *t != prev && *t != tab);
            self.recent.insert(0, prev);
            self.recent.truncate(16);
            crate::compare_mode::tab_opened(&self.doc, tab, cx);
        }
        self.active = Some(tab);
        // The tab bar shows the folder the tab is in.
        let folder = self.doc.read(cx).session.doc.parent_of(tab);
        if self.open_folder.is_some() || folder.is_some() {
            self.open_folder = folder;
        }
        // Ctrl+Z / Ctrl+Y act on this tab's edits (Onshape).
        self.doc.update(cx, |d, _| d.session.active_tab = Some(tab));
        if let Some((tm, _)) = &self.tab_manager {
            tm.update(cx, |t, cx| t.set_active(Some(tab), cx));
        }
        if let Some((DocPanel::WhereUsed(w), _)) = &self.doc_panel {
            w.update(cx, |w, cx| w.set_tab(Some(tab), cx));
        }
        // Versions and history's Repair wrench is there for Part Studios only
        // (live ui2/950 vs document/22).
        if let Some(h) = &self.history {
            h.update(cx, |_, cx| cx.notify());
        }
        self.ensure_view(tab, window, cx);
        self.focus_active(window, cx);
        cx.notify();
    }

    /// The active tab's Part Studio view, if it is one (smoke test).
    pub(crate) fn active_part_studio(&self) -> Option<Entity<PartStudioView>> {
        match self.active.and_then(|t| self.views.get(&t)) {
            Some(TabView::PartStudio(v)) => Some(v.clone()),
            _ => None,
        }
    }

    /// The active tab's drawing editor, if it is one (ui-parity driver).
    pub(crate) fn active_drawing(&self) -> Option<Entity<crate::drawing_view::DrawingEditor>> {
        match self.active.and_then(|t| self.views.get(&t)) {
            Some(TabView::Drawing(v)) => Some(v.clone()),
            _ => None,
        }
    }

    /// Whether the active tab is a Material Library or a file (ui-parity driver: live reads an empty feature list there).
    pub(crate) fn active_is_library_or_file(&self, cx: &App) -> bool {
        self.active.and_then(|t| self.doc.read(cx).session.doc.tab(t)).is_some_and(|t| matches!(t.content, TabContent::MaterialLibrary(_) | TabContent::File(_)))
    }

    /// The active tab's Feature Studio, if it is one (ui-parity driver).
    pub(crate) fn active_feature_studio(&self) -> Option<Entity<crate::feature_studio::FeatureStudioView>> {
        match self.active.and_then(|t| self.views.get(&t)) {
            Some(TabView::FeatureStudio(v)) => Some(v.clone()),
            _ => None,
        }
    }

    /// The active tab's Assembly view, if it is one (ui-parity driver).
    pub(crate) fn active_assembly(&self) -> Option<Entity<AssemblyView>> {
        match self.active.and_then(|t| self.views.get(&t)) {
            Some(TabView::Assembly(v)) => Some(v.clone()),
            _ => None,
        }
    }

    /// An open Assembly tab's view, active or not (the ui-parity driver reads the start's assembly from a Part Studio
    /// edited in context, as live's recorder reads it over REST).
    pub(crate) fn any_assembly(&self) -> Option<Entity<AssemblyView>> {
        self.active_assembly().or_else(|| self.views.values().find_map(|v| match v {
            TabView::Assembly(v) => Some(v.clone()),
            _ => None,
        }))
    }

    /// Opens the first tab named `name` (ui-parity driver); false if none.
    pub(crate) fn activate_named(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(tab) = self.doc.read(cx).session.doc.tabs.iter().find(|t| t.name == name).map(|t| t.id) else { return false };
        self.activate(tab, window, cx);
        true
    }

    /// The ui-parity state's `document` (tests/ui-parity/README.md "Documents, tabs and preferences"): the tab bar as
    /// shown, the open tab folder, the tab being renamed and the open left panel.
    pub(crate) fn ui_document_state(&self, cx: &App) -> serde_json::Value {
        let d = &self.doc.read(cx).session.doc;
        let open = self.open_folder.filter(|f| d.is_folder(*f));
        let tabs: Vec<serde_json::Value> = d
            .children(open)
            .into_iter()
            .filter_map(|id| {
                let (name, kind) = match (d.tab(id), d.folder(id)) {
                    (Some(t), _) => (t.name.clone(), ui_tab_kind(&t.content)),
                    (None, Some(f)) => (f.name.clone(), "folder"),
                    _ => return None,
                };
                Some(serde_json::json!({ "name": name, "kind": kind, "active": self.active == Some(id) }))
            })
            .collect();
        let renaming = self.renaming_tab.as_ref().and_then(|(id, ..)| d.tab(*id).map(|t| t.name.clone()).or_else(|| d.folder(*id).map(|f| f.name.clone())));
        let panel = if self.tab_manager.is_some() {
            Some("Tabs")
        } else if self.history.is_some() {
            Some("Versions and history")
        } else if self.units_panel.is_some() {
            Some("Workspace units")
        } else if self.release_drafts.is_some() {
            Some("Release drafts")
        } else {
            match &self.doc_panel {
                Some((DocPanel::Notes(_), _)) => Some("Document notes"),
                Some((DocPanel::Performance(_), _)) => Some("Performance"),
                Some((DocPanel::WhereUsed(_), _)) => Some("Where used"),
                None => None,
            }
        };
        // The navbar's label after the name: the version viewed, else the workspace (a history entry and view only mode too:
        // live ver-crow-open, doc-menu-view-only).
        let session = &self.doc.read(cx).session;
        let workspace = match session.viewing_version() {
            Some(v) => v,
            None => session.shown_workspace_name(),
        };
        let history = self.history.as_ref().map(|h| h.read(cx).ui_rows(cx));
        let compare = noshape_history_ui::compare_mode::ui_state(&self.doc, self.active, cx);
        // While comparing the navbar shows the compare bar, not the workspace (live ver-compare-*).
        let workspace = compare.is_none().then_some(workspace);
        serde_json::json!({
            "workspace": workspace,
            "history": history,
            "compare": compare,
            "tabs": tabs,
            "folder": open.and_then(|f| d.folder(f).map(|x| x.name.clone())),
            "renaming": renaming,
            "panel": panel,
            "switcher": self.switcher.as_ref().map(|(ids, sel)| serde_json::json!({ "rows": ids.iter().filter_map(|i| d.tab(*i).map(|t| t.name.clone())).collect::<Vec<_>>(), "selected": sel })),
        })
    }

    /// The active tab's Variable Studio state (ui-parity driver), if it is one.
    pub(crate) fn ui_variables_state(&self, cx: &App) -> Option<serde_json::Value> {
        match self.active.and_then(|t| self.views.get(&t)) {
            Some(TabView::Variables(v)) => Some(v.read(cx).ui_state(cx)),
            _ => None,
        }
    }

    /// The active Feature Studio's editor state (ui-parity driver), if it is one.
    pub(crate) fn ui_feature_studio_state(&self, cx: &App) -> Option<serde_json::Value> {
        // Comparing: the compare's left (base) code pane, at its top, with nothing to commit or undo.
        if let Some(text) = self.active.and_then(|t| noshape_history_ui::compare_mode::code_base(&self.doc, t, cx)) {
            return Some(serde_json::json!({
                "featurestudio": { "caret": [0, 0], "line": text.split('\n').next().unwrap_or(""), "lines": text.split('\n').count(), "committed": null, "notices": 0 },
                "unsaved": false,
                "comparing": true,
                "list": null,
            }));
        }
        match self.active.and_then(|t| self.views.get(&t)) {
            Some(TabView::FeatureStudio(v)) => {
                let st = v.read(cx).ui_state(cx);
                // Live's reader counts the notices pane's rows when it is open, else the gutter's marked rows.
                let notices = if self.fs_notices.is_some() { st.notices } else { st.notice_lines };
                let list = st.completion.map(|rows| serde_json::json!({ "kind": "autocomplete", "rows": rows, "meta": st.completion_kinds.clone().unwrap_or_default(), "highlighted": gpui_kit::component::input::COMPLETION_SELECTED_ROW.load(std::sync::atomic::Ordering::Relaxed), "disabled": [] }));
                Some(serde_json::json!({
                    "featurestudio": { "caret": [st.caret.0, st.caret.1], "line": st.line, "lines": st.lines, "committed": st.committed, "notices": notices },
                    "unsaved": st.unsaved,
                    "list": list,
                }))
            }
            _ => None,
        }
    }

    /// Repaints the active tab's view and the chrome (the ui-parity driver's marks; cached slices too).
    pub(crate) fn ui_redraw(&mut self, cx: &mut Context<Self>) {
        match self.active.and_then(|t| self.views.get(&t)) {
            Some(TabView::FeatureStudio(v)) => v.update(cx, |v, cx| v.ui_redraw(cx)),
            Some(TabView::Variables(v)) => v.update(cx, |_, cx| cx.notify()),
            _ => {}
        }
        if let Some(s) = &self.slices {
            for slice in [&s.title, &s.rail, &s.tabs] {
                slice.update(cx, |_, cx| cx.notify());
            }
        }
        if let Some((tm, _)) = &self.tab_manager {
            tm.update(cx, |_, cx| cx.notify());
        }
        if let Some(h) = &self.history {
            h.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }

    /// The active tab is a Part Studio, Assembly or Drawing whose view isn't made yet, or a Feature Studio still saving or
    /// compiling what was typed (the ui-parity driver waits).
    pub(crate) fn ui_view_pending(&self, cx: &App) -> bool {
        let Some(id) = self.active else { return false };
        if let Some(TabView::FeatureStudio(v)) = self.views.get(&id) {
            return v.read(cx).ui_busy();
        }
        let studio = self.doc.read(cx).session.doc.tab(id).is_some_and(|t| matches!(t.content, TabContent::PartStudio(_) | TabContent::Assembly(_) | TabContent::Drawing(_)));
        studio && !self.views.contains_key(&id)
    }

    /// The active tab's text field with the keyboard (the inline tab rename): `state.focus` for the ui-parity driver
    /// when no studio reads it.
    pub(crate) fn ui_rename_focus(&self, window: &Window, cx: &App) -> Option<serde_json::Value> {
        // The navbar's name field (live: a "field" with the name, all selected).
        if let Some((st, ..)) = self.renaming_doc.as_ref() {
            let st = st.read(cx);
            if st.focus_handle(cx).is_focused(window) {
                let mut f = noshape_app_core::ui_script::field_focus("", st);
                f["kind"] = serde_json::json!("field");
                f["label"] = serde_json::Value::Null;
                return Some(f);
            }
        }
        let (_, st, _) = self.renaming_tab.as_ref()?;
        let st = st.read(cx);
        st.focus_handle(cx).is_focused(window).then(|| {
            let mut f = noshape_app_core::ui_script::field_focus("", st);
            f["kind"] = serde_json::json!("other");
            f.as_object_mut().map(|o| o.remove("label"));
            f
        })
    }

    /// A key no binding took while a navbar or tab bar control has the focus
    /// (after Tab left a dialog, or a click on a tab), or the window itself (a
    /// click in a panel, live ver-history-keyboard: ↓ rotates the view after a
    /// history row's click): Onshape's shortcuts are document-wide, so it goes
    /// to the active tab's editor. Tab, Enter, Space and Esc stay with the
    /// focused control; typing in a field too.
    fn forward_chrome_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let chrome = self.chrome_focus.iter().any(|f| f.contains_focused(window, cx)) || self.focus.is_focused(window);
        if !chrome || window.context_stack().iter().any(|c| c.contains("Input")) {
            return;
        }
        let k = &ev.keystroke;
        if matches!(k.key.as_str(), "tab" | "enter" | "space" | "escape" | "shift" | "control" | "alt" | "platform" | "function") {
            return;
        }
        let Some(editor) = self.active_focus(cx) else { return };
        cx.stop_propagation();
        let (keystroke, chrome) = (k.clone(), window.focused(cx));
        // Replayed once this dispatch is over and no entity is being updated
        // (the workspace sees the replayed key too).
        window.defer(cx, move |window, cx| {
            window.focus(&editor, cx);
            window.dispatch_keystroke(keystroke, cx);
            // The control keeps the focus unless the shortcut moved it (live).
            if let Some(chrome) = chrome.filter(|_| editor.is_focused(window)) {
                window.focus(&chrome, cx);
            }
        });
    }

    /// The active tab's editor's focus handle.
    fn active_focus(&self, cx: &App) -> Option<FocusHandle> {
        Some(match self.active.and_then(|t| self.views.get(&t))? {
            TabView::PartStudio(v) => v.read(cx).focus_handle(cx),
            TabView::Drawing(v) => v.read(cx).focus.clone(),
            TabView::Assembly(v) => v.read(cx).focus_handle(cx),
            TabView::FeatureStudio(v) => v.read(cx).editor_focus(cx),
            _ => return None,
        })
    }

    /// Give keyboard focus to the active tab's editor.
    fn focus_active(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(fh) = self.active_focus(cx) {
            window.focus(&fh, cx);
        }
    }

    pub(crate) fn add_tab(&mut self, kind: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let mut new_id = None;
        let after = self.active;
        self.doc.update(cx, |doc, cx| {
            // Any other kind ("variables") is a Variable Studio.
            let (base, content) = docedit::new_tab_content(kind).or_else(|| docedit::new_tab_content("variablestudio")).expect("tab kinds");
            doc.edit_document(&docedit::insert_tab_description(base), cx, |d| {
                new_id = Some(docedit::insert_tab(d, base, content, after));
            });
        });
        if let Some(id) = new_id {
            self.activate(id, window, cx);
            if let Some(TabView::FeatureStudio(v)) = self.views.get(&id) {
                v.clone().update(cx, |v, cx| v.start_new(window, cx));
            }
        }
    }

    /// Onshape: deleting a tab has no confirmation and no toast, and Ctrl+Z
    /// doesn't bring it back (Versions and history > Restore does). Only
    /// when other tabs use it does a "Delete tabs" warning ask first.
    fn delete_tab(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let doc = self.doc.read(cx);
        if doc.session.doc.tabs.len() <= 1 {
            return;
        }
        let name = doc.session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
        let users: Vec<(String, &'static str)> = doc.session.doc.tab_users(tab).into_iter().filter_map(|u| doc.session.doc.tab(u).map(|t| (t.name.clone(), kind_of(&t.content)))).collect();
        let model = self.doc.clone();
        let ws = cx.entity().downgrade();
        // Afterwards the keyboard goes back to the active tab (Ctrl+Z etc.).
        let delete = move |window: &mut Window, cx: &mut App| {
            model.update(cx, |doc, cx| {
                doc.edit_document_not_undoable(&format!("Delete tab : {name}"), cx, |d| {
                    d.remove_tab(tab);
                });
            });
            let _ = ws.update(cx, |w, cx| {
                if let Some(a) = w.active.filter(|a| *a != tab) {
                    w.activate(a, window, cx);
                }
            });
        };
        if users.is_empty() {
            let name = self.doc.read(cx).session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
            self.doc.update(cx, |doc, cx| {
                doc.edit_document_not_undoable(&format!("Delete tab : {name}"), cx, |d| {
                    d.remove_tab(tab);
                });
            });
            if let Some(a) = self.active.filter(|a| *a != tab) {
                self.activate(a, window, cx);
            }
            return;
        }
        use gpui_kit::component::WindowExt as _;
        let delete = std::rc::Rc::new(delete);
        window.open_dialog(cx, move |dialog, _, cx| {
            let delete = delete.clone();
            // Live (part-studio/1584): a 598 px "Delete tabs" modal, a yellow
            // warning box 10 px in, then a row per tab that uses this one
            // (its tab icon and name), blue Delete then gray Cancel.
            let n = users.len();
            let warning = format!("Deleting this tab will result in failed or missing features in {n} {}", if n == 1 { "tab" } else { "tabs" });
            noshape_app_core::ui_script::note_message(warning.clone());
            let rows = users.iter().map(|(name, kind)| {
                h_flex().h(px(26.)).pl(px(23.)).gap(px(3.)).text_size(px(13.33)).text_color(p().text).child(crate::assets::icon(tab_icon(kind), 14., cx)).child(name.clone())
            });
            crate::os_ui::modal_doc_page(dialog, "Delete tabs", 598.)
                .child(
                    crate::os_ui::modal_body()
                        .pt(px(17.))
                        .pb(px(7.))
                        .gap(px(13.))
                        .child(crate::ui::alert(crate::ui::AlertKind::Warning).h(px(53.)).pl(px(15.)).gap(px(3.)).text_size(px(13.33)).child(crate::assets::icon("warning-outline", 18., cx)).child(warning))
                        .child(gpui_kit::component::v_flex().children(rows)),
                )
                .footer(
                    crate::os_ui::modal_footer()
                        .child(crate::os_ui::modal_ok(crate::os_ui::primary_button("ok", "Delete")))
                        .child(crate::os_ui::modal_cancel(crate::os_ui::cancel_button("cancel", "Cancel", cx))),
                )
                .on_ok(move |_, window, cx| {
                    delete(window, cx);
                    true
                })
        });
    }

    /// Tab › Rename: edit the name in the tab; Enter or leaving commits.
    fn start_tab_rename(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::component::input::{InputEvent, InputState};
        let d = &self.doc.read(cx).session.doc;
        let name = d.tab(tab).map(|t| t.name.clone()).or_else(|| d.folder(tab).map(|f| f.name.clone())).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) && this.renaming_tab.is_some() {
                let name = st.read(cx).value().trim().to_string();
                this.renaming_tab = None;
                if !name.is_empty() {
                    // Live (qa5 TABS5): Ctrl+Z doesn't undo a tab rename.
                    this.doc.update(cx, |d, cx| {
                        d.edit_document_not_undoable(&format!("Rename tab : {name}"), cx, |doc| {
                            if let Some(t) = doc.tab_mut(tab) {
                                t.name = name.clone();
                            } else {
                                doc.rename_folder(tab, &name);
                            }
                        })
                    });
                }
                cx.notify();
            }
        });
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.renaming_tab = Some((tab, st, sub));
        cx.notify();
    }

    /// The navbar's name becomes a field, all selected (live: a click on it, ☰ › Rename document); Enter or leaving
    /// it renames the document, Esc keeps the name.
    fn start_doc_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::component::input::{InputEvent, InputState};
        if let Some((st, ..)) = &self.renaming_doc {
            st.update(cx, |s, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
            return;
        }
        let name = self.doc.read(cx).name();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if !matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) || this.renaming_doc.is_none() {
                return;
            }
            this.renaming_doc = None;
            let name = st.read(cx).value().trim().to_string();
            if !name.is_empty() && name != this.doc.read(cx).name() {
                let r = this.doc.update(cx, |d, _| d.session.store.set_name(&name));
                this.doc.update(cx, |d, cx| match r {
                    Ok(()) => d.reloaded(cx),
                    Err(e) => cx.emit(DocEvent::Error(format!("{e:#}"))),
                });
            }
            cx.notify();
        });
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.renaming_doc = Some((st, sub, std::time::Instant::now()));
        cx.notify();
    }

    /// Tab › Select as document thumbnail, and live's toast saying so (doc-tab-menu-thumbnail).
    fn select_thumbnail(&mut self, tab: TabId, cx: &mut Context<Self>) {
        self.doc.update(cx, |d, cx| {
            if let Err(e) = d.session.store.set_meta("thumbnail_tab", Some(tab.0.to_string().as_bytes())) {
                cx.emit(DocEvent::Error(format!("Could not set the document thumbnail: {e:#}")));
                return;
            }
            let name = d.session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
            cx.emit(DocEvent::HistoryChanged);
            cx.emit(DocEvent::Info(format!("Document thumbnail set to {name}")));
        });
    }

    /// "+" › Paste <element>: the element copied with Copy to clipboard, after the open tab, which stays open (live
    /// doc-tab-menu-copy-paste); a name already there gets " Copy <n>".
    fn paste_tab(&mut self, cx: &mut Context<Self>) {
        let Some(t) = cx.try_global::<crate::doc_dialogs::CopiedTab>().and_then(|c| c.0.clone()) else { return };
        let after = self.active;
        self.doc.update(cx, |doc, cx| {
            doc.edit_document(&format!("Paste tab : {}", t.name), cx, |d| {
                let name = if d.tabs.iter().any(|x| x.name == t.name) { docedit::copy_name(d, &t.name) } else { t.name.clone() };
                let id = d.add_tab(&name, t.content.clone());
                place_after(d, id, after);
            });
        });
    }

    fn duplicate_tab(&mut self, tab: TabId, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            let Some(name) = doc.session.doc.tab(tab).map(|t| t.name.clone()) else { return };
            doc.edit_document(&docedit::duplicate_tab_description(&name), cx, |d| {
                docedit::duplicate_tab(d, tab);
            });
        });
    }

    /// Onshape's document top bar (§3.1): 40 px `#eaeaea`, ☰ document menu,
    /// document name 22 px / 600 and the workspace name in gray.
    fn render_title_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let doc = self.doc.read(cx);
        let name = doc.name();
        let read_only = doc.session.read_only();
        let viewing_version = doc.session.viewing_version();
        // The current branch (Onshape shows the workspace name, e.g. "Main").
        let branch_name = doc.session.shown_workspace_name();
        let protected = doc.session.is_protected();
        TitleBar::new()
            .h(px(crate::ui::NAVBAR_H))
            .bg(pal.panel_inactive)
            .border_b_0()
            .child(
                h_flex()
                    .id("navbar")
                    .w_full()
                    .h_full()
                    .pr_2()
                    .gap(px(4.))
                    .on_hover(cx.listener(|this, h: &bool, _, cx| {
                        this.navbar_hovered = *h;
                        cx.notify();
                    }))
                    // Over its links and buttons their own cursor shows.
                    .on_mouse_move(cx.listener(|this, _: &MouseMoveEvent, _, cx| {
                        if this.navbar_hovered && this.selection_cursor_count(cx) > 0 {
                            cx.notify();
                        }
                    }))
                    .child(
                        // Onshape: the logo block ends at x 139, the logo 14 px in
                        // (the title bar already pads 12 px on the left).
                        h_flex()
                            .id("navbar-logo")
                            .track_focus(&self.logo_focus)
                            // A focused link: the browser's focus ring (a 2 px
                            // border kept transparent otherwise, offset so the
                            // logo doesn't move).
                            .ml(px(-2.))
                            .rounded(px(3.))
                            .border_2()
                            .border_color(transparent_black())
                            .focus(|s| s.border_color(gpui_kit::rgb(0x1651b0)))
                            .on_key_down(|ev: &KeyDownEvent, _, cx| {
                                if ev.keystroke.key == "enter" {
                                    crate::files::show_start_window(cx);
                                }
                            })
                            .flex_none()
                            // Live (part-studio/181): the ☰ ink starts at x 147.
                            .w(px(125.))
                            .gap(px(10.))
                            .cursor_pointer()
                            // Onshape: the logo goes to the documents page.
                            .on_click(|_, _, cx| crate::files::show_start_window(cx))
                            .child(crate::assets::icon("noshape-logo", 24., cx))
                            // The wordmark: #4a4a4a, white in dark mode (live dk-921).
                            .child(div().text_size(px(21.)).font_weight(FontWeight::NORMAL).text_color(crate::theme::themed(0x4a4a4a, 0xffffff)).child("Noshape")),
                    )
                    .child(self.render_doc_menu(cx))
                    .child(match self.renaming_doc.as_ref() {
                        Some((st, _, started)) => div()
                            .flex_none()
                            .w(px(300.))
                            // A double-click's second press (on the field the first one opened) keeps it all selected.
                            .capture_any_mouse_down({
                                let started = *started;
                                move |_, _, cx| {
                                    if started.elapsed() < std::time::Duration::from_millis(500) {
                                        cx.stop_propagation();
                                    }
                                }
                            })
                            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                                // Esc keeps the name: the field goes before its blur would rename.
                                if ev.keystroke.key == "escape" {
                                    cx.stop_propagation();
                                    this.renaming_doc = None;
                                    window.focus(&this.focus, cx);
                                    cx.notify();
                                }
                            }))
                            .child(gpui_kit::component::input::Input::new(st).h(px(30.)).text_size(px(22.)).font_weight(FontWeight::BOLD).px(px(3.)).text_color(pal.text))
                            .map(|d| noshape_app_core::ui_script::mark(d, || "navbar:name".into()))
                            .into_any_element(),
                        None => div()
                            .id("navbar-doc-name")
                            .flex_none()
                            .pr(px(4.))
                            .py(px(2.))
                            .text_size(px(22.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(pal.text)
                            .truncate()
                            .cursor_text()
                            .child(crate::ui::user_text(name))
                            .when(!read_only && viewing_version.is_none(), |d| {
                                // A double-click's second press can reach the name before the field is drawn: it
                                // stays here (the navbar would take the keyboard) and selects the field's text again.
                                d.on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                    if this.renaming_doc.is_some() {
                                        cx.stop_propagation();
                                    }
                                }))
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.start_doc_rename(window, cx)))
                            })
                            .map(|d| noshape_app_core::ui_script::mark(d, || "navbar:name".into()))
                            .into_any_element(),
                    })
                    // Live: on the name's baseline, 11 px after it. The name and this
                    // label show the I-beam (dom/cursors-controls.json).
                    // Live (navigation/301): a version shows its name ("V1", #999) in the
                    // branch's place and no banner (its view-only bar is in the toolbar row).
                    // A protected workspace: a lock before its name (live wp-929).
                    .when(protected && viewing_version.is_none(), |d| d.child(div().flex_none().mt(px(6.)).child(crate::assets::icon("lock", 14., cx))))
                    .child(div().flex_none().mt(px(6.)).text_size(px(16.)).text_color(pal.text_faint).cursor_text().map(|d| noshape_app_core::ui_script::mark(d, || "navbar:workspace".into())).child(match &viewing_version {
                        // Version and workspace names are the user's; a history entry keeps the workspace's.
                        Some(v) => crate::ui::user_text(v.clone()).into_any_element(),
                        None => crate::ui::user_text(branch_name).into_any_element(),
                    }))
                    .when(read_only && viewing_version.is_none(), |d| d.child(div().ml_2().px_2().rounded(px(2.)).bg(pal.warning).text_xs().text_color(pal.text).child("VIEWING HISTORY")))
                    .child(div().flex_1())
                    .children(self.render_notices_button(cx))
                    .child(self.render_bell(cx))
                    .child(self.render_help_menu(cx)),
            )
            .into_any_element()
    }

    /// The active Part Studio's or Assembly's selection-count cursor number.
    fn selection_cursor_count(&self, cx: &App) -> usize {
        match self.active.and_then(|t| self.views.get(&t)) {
            Some(TabView::PartStudio(v)) => v.read(cx).selection_cursor_count(cx),
            Some(TabView::Assembly(v)) => v.read(cx).selection_cursor_count(cx),
            _ => 0,
        }
    }

    /// The Notifications bell (live: right of the navbar, a blue unread
    /// count; EX11): its panel lists export and import results.
    fn render_bell(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let ws = cx.entity();
        crate::notifications::bell_button(
            self.notifications_open,
            move |_, cx| {
                ws.update(cx, |this, cx| {
                    this.notifications_open = !this.notifications_open;
                    cx.notify();
                })
            },
            cx,
        )
    }

    /// Help ▾ at the right of the navbar (Onshape's `?▾`).
    fn render_help_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        Button::new("help-menu")
            .custom(navbar_variant(cx))
            .map(|d| noshape_app_core::ui_script::mark(d, || "button:Help".into()))
            // Live (part-studio/00): a filled dark "?" disc and a solid caret.
            .child(h_flex().gap(px(6.)).child(crate::assets::icon("help", 20., cx)).child(crate::ui::glyph("ui-caret-down", 9., pal.text)))
            .h(px(crate::ui::NAVBAR_H))
            .px(px(6.))
            .rounded(px(0.))
            .os_dropdown_menu("help-menu-dd", false, |menu, _, _| crate::help_menu::items(menu))
            .into_any_element()
    }

    /// Onshape's ☰ document menu (live, document/32): a Bootstrap dropdown
    /// under the button. Rename document, Move to…, Document details…,
    /// Restore deleted workspaces… | Copy workspace…, Update workspace…,
    /// Workspace units…, Workspace properties…, Switch to view only mode |
    /// Close document.
    fn render_doc_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let ws = cx.entity();
        let view_only = self.doc.read(cx).session.view_only;
        // A window on a version (live navbar-version-v1): the workspace's
        // items give way to Copy version… and Version properties….
        let at_version = {
            let s = &self.doc.read(cx).session;
            s.viewing.map(|mv| (mv, s.store.list_versions().ok().and_then(|vs| vs.into_iter().find(|v| v.microversion == mv)).map(|v| v.id)))
        };
        Button::new("doc-menu")
            .custom(navbar_variant(cx))
            // Live: the ☰ ink spans y 13–27.
            .child(div().mt(px(1.)).child(crate::assets::icon("menu", 20., cx)))
            .tooltip("Document menu")
            .map(|d| noshape_app_core::ui_script::mark(d, || "button:Document menu".into()))
            .w(px(32.))
            .h(px(crate::ui::NAVBAR_H))
            .p_0()
            .rounded(px(0.))
            .on_click(move |e, window, cx| {
                use crate::doc_dialogs as dd;
                use crate::os_menu::OsMenuItem as I;
                let doc = ws.read(cx).doc.clone();
                let x = e.position().x;
                let (d2, d3, d4, d5, d6, d7, d8) = (doc.clone(), doc.clone(), doc.clone(), doc.clone(), doc.clone(), doc.clone(), doc.clone());
                let (w, w2, w3) = (ws.clone(), ws.clone(), ws.clone());
                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                    if let Some((mv, version)) = at_version {
                        let m = m
                            .dropdown()
                            .item(I::new("Rename document").on_click({
                                let w3 = w3.clone();
                                move |_, wi, cx| w3.update(cx, |t, cx| t.start_doc_rename(wi, cx))
                            }))
                            .item(I::new("Move to…").on_click(move |_, wi, cx| dd::move_to(d2.clone(), wi, cx)))
                            .item(I::new("Restore deleted workspaces…").on_click(move |_, wi, cx| dd::restore_deleted_workspaces(d4.clone(), wi, cx)))
                            .separator()
                            // Versions and history closes behind it (live ver-view-version-copy, doc-document-menu-copy-version).
                            .item(I::new("Copy version…").on_click({
                                let w3 = w3.clone();
                                move |_, wi, cx| {
                                    w3.update(cx, |t, cx| {
                                        t.history = None;
                                        cx.notify();
                                    });
                                    dd::copy_version(d5.clone(), mv, wi, cx)
                                }
                            }));
                        let m = match version {
                            Some(id) => m.item(I::new("Version properties…").on_click(move |_, wi, cx| noshape_history_ui::version_properties::open(d7.clone(), noshape_history_ui::version_properties::Item::Version(id), wi, cx))),
                            None => m,
                        };
                        return m
                            .separator()
                            .item(I::new("Print…").on_click(move |_, wi, cx| w2.update(cx, |t, cx| t.print_active(wi, cx))))
                            .separator()
                            .item(I::new("Close document").on_click(close_document));
                    }
                    // View only mode (live doc-menu-view-only): only what doesn't edit the workspace.
                    if view_only {
                        return m
                            .dropdown()
                            .item(I::new("Copy workspace…").on_click(move |_, wi, cx| dd::copy_workspace(d5.clone(), wi, cx)))
                            .item(I::new("Switch to edit mode").on_click(move |_, _, cx| {
                                d8.update(cx, |d, cx| {
                                    d.session.view_only = false;
                                    cx.notify();
                                })
                            }))
                            .separator()
                            .item(I::new("Print…").on_click(move |_, wi, cx| w2.update(cx, |t, cx| t.print_active(wi, cx))))
                            .separator()
                            .item(I::new("Close document").on_click(close_document));
                    }
                    m.dropdown()
                        .item(I::new("Rename document").on_click({
                                let w3 = w3.clone();
                                move |_, wi, cx| w3.update(cx, |t, cx| t.start_doc_rename(wi, cx))
                            }))
                        .item(I::new("Move to…").on_click(move |_, wi, cx| dd::move_to(d2.clone(), wi, cx)))
                        .item(I::new("Document details…").on_click(move |_, wi, cx| dd::document_details(d3.clone(), wi, cx)))
                        .item(I::new("Restore deleted workspaces…").on_click(move |_, wi, cx| dd::restore_deleted_workspaces(d4.clone(), wi, cx)))
                        .separator()
                        .item(I::new("Copy workspace…").on_click(move |_, wi, cx| dd::copy_workspace(d5.clone(), wi, cx)))
                        .item(I::new("Update workspace…").on_click(move |_, _, cx| dd::update_workspace(d6.clone(), cx)))
                        .item(I::new("Workspace units…").on_click(move |_, wi, cx| w.update(cx, |t, cx| t.open_units_panel(wi, cx))))
                        .item(I::new("Workspace properties…").on_click(move |_, wi, cx| dd::workspace_properties(d7.clone(), wi, cx)))
                        .item(I::new("Switch to view only mode").on_click(move |_, _, cx| {
                            d8.update(cx, |d, cx| {
                                d.session.view_only = true;
                                cx.notify();
                            })
                        }))
                        .separator()
                        .item(I::new("Print…").on_click(move |_, wi, cx| w2.update(cx, |t, cx| t.print_active(wi, cx))))
                        .separator()
                        .item(I::new("Close document").on_click(close_document))
                });
                crate::os_menu::open_anchored(point(x - px(16.), px(crate::ui::NAVBAR_H)), Anchor::TopLeft, menu, window, cx);
            })
            .into_any_element()
    }

    /// Look again for tabs behind newer versions of linked documents: when
    /// the references changed, or always (`force`). The other documents are
    /// read off the UI thread.
    fn refresh_behind(&mut self, force: bool, cx: &mut Context<Self>) {
        let refs: Vec<(TabId, crate::derived::ExternalRef)> = {
            let s = &self.doc.read(cx).session;
            noshape_ops::workspace_update::version_refs(&s.doc).into_iter().filter(|(_, r)| r.path != s.path).collect()
        };
        if !force && refs == self.behind_refs {
            return;
        }
        self.behind_refs = refs.clone();
        if refs.is_empty() {
            self.behind_task = None;
            if !self.behind.is_empty() {
                self.behind.clear();
                cx.notify();
            }
            return;
        }
        self.behind_task = Some(cx.spawn(async move |this, cx| {
            let behind = cx.background_spawn(async move { noshape_ops::workspace_update::behind_tabs(&refs) }).await;
            let _ = this.update(cx, |t, cx| {
                if t.behind != behind {
                    t.behind = behind;
                    cx.notify();
                }
            });
        }));
    }

    /// ☰ › Workspace units…: Onshape's panel at the feature dialog's place.
    /// Live (qa4 F24, "qa4 F17/F24 placement checks"): an open sketch or
    /// feature dialog is accepted at once and the panel takes its place
    /// (✕ doesn't bring it back).
    fn open_units_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(studio) = self.active_studio() {
            studio.update(cx, |s, cx| s.accept_open_dialog(window, cx));
        }
        let doc = self.doc.clone();
        let panel = cx.new(|cx| crate::doc_dialogs::WorkspaceUnits::new(doc, cx));
        let sub = cx.subscribe(&panel, |this: &mut Self, _, _: &crate::doc_dialogs::WorkspaceUnitsEvent, cx| {
            this.units_panel = None;
            cx.notify();
        });
        self.units_panel = Some((panel, sub));
        cx.notify();
    }

    fn active_studio(&self) -> Option<Entity<PartStudioView>> {
        match self.active.and_then(|a| self.views.get(&a)) {
            Some(TabView::PartStudio(v)) => Some(v.clone()),
            _ => None,
        }
    }

    /// Document menu › Print…: a drawing opens as a print-friendly PDF.
    /// ☰ › Print…: a drawing's sheets; a Part Studio's or an Assembly's Print setup (live studios 2349).
    fn print_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.active.and_then(|a| self.views.get(&a)) {
            Some(TabView::Drawing(v)) => v.update(cx, |v, cx| v.print(cx)),
            Some(TabView::PartStudio(v)) => v.update(cx, |v, cx| v.open_print(window, cx)),
            Some(TabView::Assembly(v)) => v.update(cx, |v, cx| v.open_print(window, cx)),
            _ => {}
        }
    }

    /// Drawing tab › Change to version…: the drawing's Reference manager
    /// (live drawing/293).
    fn change_drawing_version(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        self.activate(tab, window, cx);
        let Some(TabView::Drawing(v)) = self.views.get(&tab) else { return };
        v.update(cx, |v, cx| v.open_change_to_version(cx));
    }

    /// Tab › Export…: Onshape's Export modal (live, drawing/40 layout): File
    /// name, Format select; Export saves the file.
    fn open_export(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        self.activate(tab, window, cx);
        match self.views.get(&tab) {
            Some(TabView::Assembly(v)) => v.update(cx, |v, cx| v.open_export_all(window, cx)),
            // Onshape's "Export Drawing" modal (drawing_export).
            Some(TabView::Drawing(v)) => v.update(cx, |v, cx| v.open_export_dialog(window, cx)),
            // Onshape's Export modal (part_studio/export.rs).
            Some(TabView::PartStudio(v)) => v.update(cx, |v, cx| v.open_part_export(Vec::new(), window, cx)),
            _ => {}
        }
    }

    /// "+" › Import…: the OS picker (several files), then Onshape's "Import
    /// to Onshape" modal (import_dialog.rs); the new tab opens.
    /// "+" › Import…: any file. CAD files go through the Import dialog
    /// (Part Studio plus the file in "CAD Imports"); other files, DXF / DWG
    /// too, a file tab (Onshape).
    fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ws = cx.entity();
        let handle = window.window_handle();
        let (doc, after) = (self.doc.clone(), self.active);
        crate::files::request_open_paths("Import files", "All files", &["*"], window, cx, move |paths, cx| {
            let is = |p: &std::path::PathBuf, exts: &[&str]| p.extension().is_some_and(|e| exts.contains(&e.to_string_lossy().to_lowercase().as_str()));
            let (cad, rest): (Vec<_>, Vec<_>) = paths.into_iter().partition(|p| is(p, IMPORT_EXTS));
            let mut last = None;
            for p in rest {
                let Ok(bytes) = std::fs::read(&p) else { continue };
                let (stem, file) = import_names(&p);
                let _ = stem;
                // Live (behaviours "Imported DWG/DXF drawings"): a DXF / DWG
                // becomes a plain file tab; its menu's "Create Drawing of …"
                // makes the drawing.
                doc.update(cx, |d, cx| {
                    d.edit_document(&format!("Import {file}"), cx, |d| {
                        let f = crate::file_tab_view::add_file_tab(d, noshape_model::file_tab::FileTab::new(&file, &bytes, noshape_model::file_tab::now()), None, after);
                        last = Some((f, file.clone()));
                    })
                });
            }
            // Live doc-plus-import-png: the open tab stays, the file's tab goes after it, and the import's toast links to it.
            if let Some((id, file)) = last.clone() {
                doc.update(cx, |d, cx| {
                    let me = cx.entity().downgrade();
                    d.info_with_link(format!("{file} has been imported."), "Open document", move |_, cx| {
                        let _ = me.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::Ui(noshape_ops::UiRequest::ActivateTab(id))));
                    }, cx);
                });
            }
            let _ = ws;
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    if !cad.is_empty() {
                        // Live: the view stays on the open tab (the new tabs go after it).
                        crate::import_dialog::open(doc, cad, after, true, |_, _, _| {}, window, cx)
                    }
                });
            });
        });
    }

    /// DXF / DWG file tab › "Create Drawing of <file>…" (live, no dialog): a
    /// new drawing "<file> Drawing N" right after the file tab: the file's
    /// model space as the sheet "Model", then its layouts; it opens on the
    /// first layout, else on Model.
    fn create_cad_drawing(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.doc.read(cx).session.doc.tab(id).cloned() else { return };
        let TabContent::File(f) = &t.content else { return };
        let Some(bytes) = f.bytes() else { return };
        match noshape_model::drawing::imported::drawing_from_cad(&bytes) {
            // The drawing opens on its `active_sheet` (the first layout, else Model).
            Ok((dr, _)) => {
                let mut new_id = None;
                self.doc.update(cx, |d, cx| {
                    d.edit_document(&format!("Create Drawing of {}", t.name), cx, |d| {
                        let name = (1..).map(|n| format!("{} Drawing {n}", t.name)).find(|n| !d.tabs.iter().any(|x| &x.name == n)).unwrap_or_default();
                        let nt = d.add_tab(&name, TabContent::Drawing(dr));
                        place_after(d, nt, Some(id));
                        new_id = Some(nt);
                    })
                });
                if let Some(nt) = new_id {
                    self.activate(nt, window, cx);
                }
            }
            Err(e) => self.doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::Error(format!("Create drawing failed: {e}")))),
        }
    }

    /// File tab › Import… (CAD files): the file through the Import dialog
    /// (Part Studio); a DXF / DWG makes a drawing.
    fn translate_file(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(TabContent::File(f)) = self.doc.read(cx).session.doc.tab(id).map(|t| t.content.clone()) else { return };
        let Some(bytes) = f.bytes() else { return };
        match f.kind() {
            noshape_model::file_tab::FileKind::Drawing2d => self.create_cad_drawing(id, window, cx),
            _ => {
                // The Import dialog reads files from disk: hand it a temporary copy.
                let dir = std::env::temp_dir().join(format!("noshape-translate-{}", id));
                let path = dir.join(&f.file_name);
                if std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &bytes)).is_err() {
                    self.doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::Error("Translate failed: cannot write a temporary file".into())));
                    return;
                }
                // Live doc-cad-file-import: the new tabs are added and the open tab stays.
                crate::import_dialog::open(self.doc.clone(), vec![path], Some(id), false, |_, _, _| {}, window, cx);
            }
        }
    }

    /// A CAD file tab › Export… (live document/94): the Export modal, then the file read and converted off the UI
    /// thread ("Exporting <file>…" at once, then where it went or why it failed).
    fn export_file(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(TabContent::File(f)) = self.doc.read(cx).session.doc.tab(id).map(|t| t.content.clone()) else { return };
        let doc = self.doc.clone();
        let base = noshape_ops::export::file_stem(&f.file_name);
        crate::part_studio::export::open_file_export_modal(base, std::rc::Rc::new(move |s, name, window, cx| {
            // (Several parts one file each: a zip; OBJ always, as live.)
            let (options, ext) = (s.options, s.file_ext(s.individual));
            let file = format!("{name}.{ext}");
            let units = doc.read(cx).session.doc.settings.units;
            let run = {
                let (doc, f, file) = (doc.clone(), f.clone(), file.clone());
                move |path: std::path::PathBuf, store: bool, cx: &mut App| {
                    doc.update(cx, |_, cx| cx.emit(DocEvent::Info(format!("Exporting {file}…"))));
                    let task = cx.background_spawn(async move { noshape_ops::export::cad_file_parts(&f, &units).and_then(|parts| noshape_ops::export::write_export(&s, &path, parts)) });
                    cx.spawn(async move |cx| {
                        let written = task.await;
                        cx.update(|cx| {
                            let msg = match written {
                                Ok(p) if store => {
                                    let name = crate::part_studio::export::store_written(&doc, id, &p, cx);
                                    DocEvent::Info(format!("Stored {name}"))
                                }
                                Ok(p) => DocEvent::Info(format!("Exported {}", p.display())),
                                Err(e) => DocEvent::Error(format!("Export failed: {e:#}")),
                            };
                            doc.update(cx, |_, cx| cx.emit(msg));
                        });
                    })
                    .detach();
                }
            };
            // Options › Store file in a new tab: no download.
            if options == 2 {
                let dir = std::env::temp_dir().join(format!("noshape-store-{}", std::process::id()));
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("Export failed: {e}"))));
                    return;
                }
                run(dir.join(&file), true, cx);
                return;
            }
            crate::files::request_save_path("Export", file, ext, window, cx, move |path, cx| run(path, options == 1, cx));
        }), window, cx);
    }

    /// Left rail › Document notes (`notes`) or Performance; one left panel
    /// at a time, a second click closes it.
    fn toggle_doc_panel(&mut self, notes: bool, cx: &mut Context<Self>) {
        let same = match &self.doc_panel {
            Some((DocPanel::Notes(_), _)) => notes,
            Some((DocPanel::Performance(_), _)) => !notes,
            Some((DocPanel::WhereUsed(_), _)) | None => false,
        };
        self.doc_panel = None;
        if !same {
            self.history = None;
            self.release_drafts = None;
            self.tab_manager = None;
            let close = |this: &mut Self, cx: &mut Context<Self>| {
                this.doc_panel = None;
                cx.notify();
            };
            self.doc_panel = Some(if notes {
                let doc = self.doc.clone();
                let panel = cx.new(|cx| crate::doc_panels::NotesPanel::new(doc, cx));
                let sub = cx.subscribe(&panel, move |this, _, _: &crate::doc_panels::ClosePanel, cx| close(this, cx));
                (DocPanel::Notes(panel), vec![sub])
            } else {
                let snap = self.perf_snapshot(cx);
                let panel = cx.new(|_| crate::doc_panels::PerformancePanel::new(snap));
                let subs = vec![
                    cx.subscribe(&panel, move |this, _, _: &crate::doc_panels::ClosePanel, cx| close(this, cx)),
                    cx.subscribe(&panel, |this, _, _: &crate::doc_panels::ShowRegenTimes, cx| {
                        if let Some(s) = this.active_studio() {
                            s.update(cx, |s, cx| s.show_regen_times(cx));
                        }
                    }),
                ];
                (DocPanel::Performance(panel), subs)
            });
        }
        cx.notify();
    }

    /// The rail's Where used (`doc_panels::WhereUsedPanel`): opens or closes it like the other document panels.
    fn toggle_where_used(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = matches!(self.doc_panel, Some((DocPanel::WhereUsed(_), _)));
        self.doc_panel = None;
        if !open {
            self.history = None;
            self.release_drafts = None;
            self.tab_manager = None;
            let (doc, tab) = (self.doc.clone(), self.active);
            let panel = cx.new(|_| crate::doc_panels::WhereUsedPanel::new(doc, tab));
            let subs = vec![
                cx.subscribe(&panel, |this, _, _: &crate::doc_panels::ClosePanel, cx| {
                    this.doc_panel = None;
                    cx.notify();
                }),
                cx.subscribe_in(&panel, window, |this, _, ev: &crate::doc_panels::OpenTab, window, cx| this.activate(ev.0, window, cx)),
            ];
            self.doc_panel = Some((DocPanel::WhereUsed(panel), subs));
        }
        cx.notify();
    }

    /// The Performance panel's numbers: the active Part Studio, the
    /// document's tabs by kind, the GPU.
    fn perf_snapshot(&self, cx: &App) -> crate::doc_panels::PerfSnapshot {
        let mut counts = [("Part Studios", 0usize), ("Other tabs", 0), ("Assemblies", 0), ("Drawings", 0)];
        for t in self.doc.read(cx).session.doc.ordered_tabs() {
            let i = match t.content {
                TabContent::PartStudio(_) => 0,
                TabContent::Assembly(_) => 2,
                TabContent::Drawing(_) => 3,
                _ => 1,
            };
            counts[i].1 += 1;
        }
        crate::doc_panels::PerfSnapshot {
            tab: self.active_studio().map(|s| s.read(cx).tab_perf(cx)),
            tabs: counts.map(|(k, n)| (k.to_string(), n)).to_vec(),
            adapter: self.gpu.adapter_name(),
        }
    }

    fn toggle_release_drafts(&mut self, cx: &mut Context<Self>) {
        if self.release_drafts.take().is_none() {
            let doc = self.doc.clone();
            self.history = None;
            self.doc_panel = None;
            self.release_drafts = Some(cx.new(|cx| crate::release_dialog::ReleaseDraftsPanel::new(doc, cx)));
        }
        cx.notify();
    }

    /// The Versions and history panel, opened if it isn't.
    pub fn show_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.history.is_none() {
            self.toggle_history(window, cx);
        }
    }

    fn toggle_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.release_drafts = None;
        self.doc_panel = None;
        if self.history.take().is_none() {
            let doc = self.doc.clone();
            self.history = Some(cx.new(|cx| HistoryPanel::new(doc, window, cx)));
        }
        cx.notify();
    }

    /// Onshape's bottom tab bar (§3.5): 30 px `#d7d7d7`; tab-list cell under
    /// the rail, a + cell, then tabs hanging from the top edge with rounded
    /// bottom corners; the active tab is white with a 2 px blue underline.
    /// The tab bar: the Tab manager cell (it ends the left rail) and the
    /// row with "+" and the tabs (under the content, right of any open left
    /// panel).
    fn render_tab_bar(&mut self, cx: &mut Context<Self>) -> (AnyElement, AnyElement) {
        let pal = p();
        let d = &self.doc.read(cx).session.doc;
        // Onshape: with a folder open, the bar shows only that folder's tabs.
        let open = self.open_folder.filter(|f| d.is_folder(*f));
        let items: Vec<(TabId, String, Option<&'static str>)> = d
            .children(open)
            .into_iter()
            .filter_map(|id| d.tab(id).map(|t| (id, t.name.clone(), Some(kind_of(&t.content)))).or_else(|| d.folder(id).map(|f| (id, f.name.clone(), None))))
            .collect();
        let crumbs: Vec<(TabId, String)> = open.map(|f| d.ancestors(f).into_iter().rev().filter_map(|a| d.folder(a).map(|x| (a, x.name.clone()))).collect()).unwrap_or_default();
        let open_name = open.and_then(|f| d.folder(f).map(|x| x.name.clone()));
        let view = cx.entity();
        let cell = |id: &'static str, cx: &App| {
            Button::new(id)
                .custom(gpui_kit::component::button::ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).hover(gpui_kit::rgb(0xbababa).into()).active(gpui_kit::rgb(0xbababa).into()).shadow(false))
                .h(px(crate::ui::TAB_BAR_H))
                .p_0()
                .rounded(px(0.))
        };
        // Onshape: the tab-search cell (continuing the left rail, `#fafafa`
        // with a 1 px right border) opens the Tab manager (Alt+T).
        // Open (live document/30): a blue icon and the rail's blue bar on the left.
        let tm_open = self.tab_manager.is_some();
        let tab_list = cell("tab-list", cx)
            .w(px(crate::ui::RAIL_W))
            .relative()
            .bg(pal.rail)
            .border_r_1()
            .border_color(pal.border)
            .when(tm_open, |c| c.child(div().absolute().left_0().top_0().bottom_0().w(px(4.)).bg(pal.accent)).child(crate::ui::glyph("tab-search", 20., pal.accent)))
            .when(!tm_open, |c| c.child(crate::assets::icon("tab-search", 20., cx)))
            .tooltip("Tab manager (Alt+T)")
            .map(|d| noshape_app_core::ui_script::mark(d, || "button:Tab manager".into()))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_tab_manager(window, cx)));
        // Onshape's "+" menu (live, document/01): a Bootstrap dropdown opening
        // upward from the cell. Cloud items (Applications, CAM / PCB /
        // Render Studio) are not part of noshape.
        // A protected workspace takes no new tabs: "+" greyed, inert (live wp-932).
        let protected = self.doc.read(cx).session.is_protected();
        let add = cell("add-tab", cx).w(px(34.)).child(crate::ui::glyph("plus", 20., if protected { gpui_kit::rgb(0xc0c0c0) } else { crate::theme::themed(0x555555, 0xcccccc) })).tooltip("Insert new tab").map(|d| noshape_app_core::ui_script::mark(d, || "button:Insert new tab".into())).on_click(move |_, window, cx| {
            if protected {
                return;
            }
            let (v1, v2, v3, v4, v5, v6, v7) = (view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone());
            let (v8, v9) = (view.clone(), view.clone());
            let copied = cx.try_global::<crate::doc_dialogs::CopiedTab>().is_some_and(|c| c.0.is_some());
            let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                use crate::os_menu::OsMenuItem as I;
                let m = m
                    .dropdown()
                    .item(I::new("Create Material Library").icon_name("material-library").on_click(move |_, w, cx| v9.update(cx, |t, cx| t.add_tab("materiallibrary", w, cx))))
                    .item(I::new("Create Feature Studio").icon_name("feature-studio").on_click(move |_, w, cx| v5.update(cx, |t, cx| t.add_tab("featurestudio", w, cx))))
                    .separator()
                    .item(I::new("Create Part Studio").icon_name("part-studio").on_click(move |_, w, cx| v1.update(cx, |t, cx| t.add_tab("partstudio", w, cx))))
                    .item(I::new("Create Assembly").icon_name("assembly").on_click(move |_, w, cx| v2.update(cx, |t, cx| t.add_tab("assembly", w, cx))))
                    .item(I::new("Create Variable Studio").icon_name("variable-studio").on_click(move |_, w, cx| v4.update(cx, |t, cx| t.add_tab("variables", w, cx))))
                    .item(I::new("Create Drawing…").icon_name("drawing").on_click(move |_, w, cx| v3.update(cx, |t, cx| t.open_create_drawing(crate::drawing_create::CreateRequest { external: None, source: None, name: None }, w, cx))))
                    .item(I::new("Create folder").icon_name("folder").on_click(move |_, w, cx| v8.update(cx, |t, cx| t.create_folder(w, cx))))
                    .item(I::new("Import…").icon_name("import-file").on_click(move |_, w, cx| v6.update(cx, |t, cx| t.import(w, cx))));
                // After a tab's Copy to clipboard: "Paste tab", last (live doc-tab-menu-copy-paste).
                if !copied {
                    return m;
                }
                m.separator().item(I::new("Paste tab").icon_name("paste").on_click(move |_, _, cx| v7.update(cx, |t, cx| t.paste_tab(cx))))
            });
            // Live 2414: Escape leaves it open (a click elsewhere closes it).
            crate::os_menu::mark_page_dropdown(&menu, cx);
            let h = window.viewport_size().height;
            crate::os_menu::open_anchored(point(px(crate::ui::RAIL_W), h - px(crate::ui::TAB_BAR_H)), Anchor::BottomLeft, menu, window, cx);
        });
        if let Some(ix) = items.iter().position(|(id, _, _)| self.active == Some(*id)).filter(|_| self.reveal_active_tab) {
            self.tab_scroll.scroll_to_item(ix);
            self.reveal_active_tab = false;
        }
        // Live (qa5 TABS1): while a tab is dragged it leaves the bar, the
        // others close up and a gap the tab's width shows where it will
        // drop (no line); the ghost moves along the bar only.
        if !cx.has_active_drag() {
            self.tab_gap = None;
        }
        let gap = self.tab_gap.as_ref().map(|g| (g.dragged, g.before));
        // The gap is the dragged tab's own width (live tabs size to their name).
        let gap_w = self.tab_gap.as_ref().map(|g| g.width).filter(|w| *w > px(0.)).unwrap_or(px(200.));
        let placeholder = || div().flex_none().w(gap_w).h(px(27.));
        let mut strip = h_flex()
            .id("tab-strip")
            .flex_1()
            .min_w_0()
            .h_full()
            .items_start()
            .pl(px(5.))
            .gap(px(2.))
            .overflow_x_scroll()
            .track_scroll(&self.tab_scroll)
            .on_drag_move::<TabDrag>(cx.listener(|this, e: &DragMoveEvent<TabDrag>, window, cx| {
                if let Some(g) = &mut this.tab_gap {
                    g.x = e.event.position.x;
                    g.scroll = autoscroll_speed(g.x, e.bounds.left(), e.bounds.right());
                    if g.scroll != 0. && g.autoscroll.is_none() {
                        g.autoscroll = Some(cx.spawn_in(window, async move |this, cx| {
                            loop {
                                cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
                                if !this.update(cx, |this, cx| this.autoscroll_tabs(cx)).unwrap_or(false) {
                                    break;
                                }
                                // The pointer, still, over the tabs scrolled under it: the gap goes where it now is
                                // (live2 tabBar.dropWhileAutoScrolled).
                                let _ = cx.update(|window, cx| {
                                    if cx.has_active_drag() {
                                        let event = MouseMoveEvent { position: window.mouse_position(), pressed_button: Some(MouseButton::Left), modifiers: window.modifiers() };
                                        window.dispatch_event(PlatformInput::MouseMove(event), cx);
                                    }
                                });
                            }
                        }));
                    }
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|this, _: &TabDrag, _, cx| this.drop_tab(cx)));
        // Live (behaviours "tab-bar folder breadcrumbs"): with a folder open,
        // chevron crumbs between "+" and the tabs (they don't scroll): 🏠 ›
        // (deeper than one level: …, its dropdown lists the folders between)
        // › the open folder, white (#333 in dark).
        let crumbs_el = open_name.map(|name| {
            let folder = open.unwrap_or(TabId(0));
            let grey = pal.tab_inactive;
            let mut row = h_flex().flex_none().h_full().items_start().child(
                breadcrumb("tab-home", grey, None, false)
                    .pl(px(8.))
                    .pr(px(5.))
                    .child(div().size(px(20.)).flex().items_center().justify_center().child(os::glyph(os::GLYPH_HOME, 18., pal.text_muted)))
                    .tooltip(crate::ui::tooltip("All tabs"))
                    .map(|d| noshape_app_core::ui_script::mark(d, || "button:All tabs".into()))
                    // Not a drop target: a tab dropped on it stays in its folder (live doc-folder-drag-to-all-tabs,
                    // behaviours "tab dropped on 🏠").
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.open_folder = None;
                        cx.notify();
                    })),
            );
            if !crumbs.is_empty() {
                let v = cx.entity();
                let bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
                let probe = bounds.clone();
                // Live: 87 wide, "…" centred left of the 12 px padding and 2 px
                // high; its dropdown opens upward from the crumb's left edge.
                let crumb = breadcrumb("tab-crumbs", grey, Some(grey), false).w(px(87.)).pr(px(12.)).pb(px(8.)).justify_center().map(|d| noshape_app_core::ui_script::mark(d, || "crumb:…".into()));
                row = row.child(crumb.child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full()).child("…").on_click(move |_, window, cx| {
                    let (crumbs, v) = (crumbs.clone(), v.clone());
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                        m = m.dropdown();
                        // Nearest parent first (live doc-nested-crumb-menu: Folder 2, Folder 1).
                        for (id, name) in crumbs.into_iter().rev() {
                            let v = v.clone();
                            m = m.item(crate::os_menu::OsMenuItem::new(name).user().icon(folder_glyph(20., p().text)).on_click(move |_, _, cx| {
                                v.update(cx, |t, cx| {
                                    t.open_folder = Some(id);
                                    cx.notify();
                                })
                            }));
                        }
                        m
                    });
                    let h = window.viewport_size().height;
                    crate::os_menu::open_anchored(point(bounds.get().left(), h - px(crate::ui::TAB_BAR_H)), Anchor::BottomLeft, menu, window, cx);
                }));
            }
            row.child(self.folder_crumb(folder, name, grey, cx)).into_any_element()
        });
        let order: Vec<TabId> = items.iter().map(|(id, ..)| *id).filter(|id| gap.is_none_or(|(d, _)| d != *id)).collect();
        for (id, name, kind) in items {
            if gap.is_some_and(|(d, _)| d == id) {
                continue;
            }
            if gap.is_some_and(|(_, b)| b == Some(Some(id))) {
                strip = strip.child(placeholder());
            }
            // The gap goes before this tab (pointer on its left half) or
            // after it.
            let next = order.iter().position(|t| *t == id).and_then(|i| order.get(i + 1)).copied();
            let gap_here = cx.listener(move |this, e: &DragMoveEvent<TabDrag>, _, cx| {
                if !e.bounds.contains(&e.event.position) {
                    return;
                }
                let Some(g) = &mut this.tab_gap else { return };
                // Over a folder the gap stays put: closing it would slide the folder out from under the pointer, and
                // the drop goes into the folder.
                if kind.is_none() {
                    return;
                }
                let before = if e.event.position.x < e.bounds.center().x { Some(Some(id)) } else { Some(next) };
                if g.before != before {
                    g.before = before;
                    cx.notify();
                }
            });
            let Some(kind) = kind else {
                strip = strip.child(self.folder_tab(id, name, cx).on_drag_move(gap_here));
                continue;
            };
            let active = self.active == Some(id);
            // Compare mode (live 952): a status dot after the name; drawings and
            // tabs only in the base are greyed and can't be opened.
            let compare = crate::compare_mode::tab_status(&self.doc, id, cx);
            let compare_off = compare.is_some_and(|s| s.disabled());
            let compare_dot = compare.map(|s| crate::compare_mode::tab_dot(SharedString::from(format!("tab-dot-{id}")), s, &crate::compare_mode::base_label(&self.doc, cx).unwrap_or_default()));
            // A drawing pinned to an older version of this document: live's
            // blue out-of-date icon before its own (drawing/297–300).
            let newer = kind == "drawing" && drawing_behind(&self.doc, id, cx);
            let view = cx.entity();
            let tab_name = name.clone();
            let bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
            let probe = bounds.clone();
            strip = strip.child(
                h_flex()
                    .id(SharedString::from(format!("tab-{id}")))
                    .relative()
                    .flex_none()
                    // Live (computed .os-tab-bar-tab: flex 1 0 auto, min-width
                    // 60 px, max-width 200 px, padding 3 10 0, margin-right
                    // 2 px): a tab is its icon and name, grown to share the
                    // bar's free space up to 200 px (two tabs in a new
                    // document: 200 px each); a full bar scrolls instead of
                    // shrinking them (ui-1025b-0: "Base" 74 px).
                    .flex_grow_1()
                    .flex_shrink_0()
                    .min_w(px(60.))
                    .max_w(px(200.))
                    .h(px(if active { 28. } else { 27. }))
                    .pl(px(10.))
                    .pr(px(10.))
                    .gap(px(3.))
                    .rounded_b(px(5.))
                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.12), offset: point(px(0.), px(1.)), blur_radius: px(2.), spread_radius: px(0.), inset: false }])
                    .text_size(px(14.))
                    // Live: inactive tabs pointer, the active tab default.
                    .when(!active, |d| d.cursor_pointer())
                    .bg(if active { pal.panel } else if compare_off { gpui_kit::rgb(0xe1e1e1) } else { pal.tab_inactive })
                    .text_color(if active { pal.text } else { pal.text_muted })
                    // Live tabbar: the blue out-of-date icon left of the tab's
                    // (a linked document's newer version, or a drawing pinned to an
                    // older version of this one).
                    .when(self.behind.contains(&id) || newer, |d| {
                        // A linked document's newer version: a click opens the Reference manager (Update to latest),
                        // as live's out-of-date icons do (a drawing's own is its Change to version…).
                        // (A drawing's does nothing on a click: its Sheets panel row opens its Reference manager, live 2303.)
                        let (doc, linked) = (self.doc.clone(), self.behind.contains(&id) && kind != "drawing");
                        d.child(
                            div()
                                .id(SharedString::from(format!("tab-behind-{id}")))
                                .flex_none()
                                .cursor_pointer()
                                .tooltip(crate::ui::tooltip("Newer versions of linked documents available"))
                                .map(|d| noshape_app_core::ui_script::mark(d, || "button:Newer versions of linked documents available".into()))
                                .when(linked, |d| {
                                    d.on_click(move |_, window, cx| {
                                        cx.stop_propagation();
                                        crate::doc_dialogs::reference_manager(doc.clone(), window, cx);
                                    })
                                })
                                .child(crate::assets::icon("inserted-version-update", 16., cx)),
                        )
                    })
                    .child(div().flex_none().when(compare_off, |d| d.opacity(0.5)).child(crate::assets::icon(tab_icon(kind), 20., cx)))
                    .child(match self.renaming_tab.as_ref().filter(|r| r.0 == id) {
                        // Tab › Rename / double-click: the name becomes a field.
                        Some((_, st, _)) => div().w(px(150.)).child(gpui_kit::component::input::Input::new(st).h(px(22.)).text_size(px(14.)).px(px(3.)).text_color(pal.text)).into_any_element(),
                        None => div().min_w_0().truncate().when(compare_off, |d| d.opacity(0.5)).child(crate::ui::user_text(name)).into_any_element(),
                    })
                    .children(compare_dot.map(|dot| div().flex_none().ml(px(3.)).when(compare_off, |d| d.opacity(0.55)).child(dot)))
                    .when(active, |d| d.child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(pal.accent)))
                    .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                    // The ui-parity driver's `{"doc_tab": name}`.
                    .map(|d| noshape_app_core::ui_script::mark(d, || format!("doc_tab:{}", tab_name.clone())))
                    // Live (part-studio/17): an inactive tab shows its element's
                    // picture in a card above it.
                    .when(!active && matches!(kind, "partstudio" | "assembly" | "drawing"), |d| {
                        let (thumbs, anchor) = (self.tab_thumbs.clone(), bounds.clone());
                        d.tooltip(move |_, cx| {
                            let (thumbs, anchor) = (thumbs.clone(), anchor.get());
                            cx.new(|cx| crate::tab_thumbs::TabCard::new(thumbs, id, anchor, cx)).into()
                        })
                    })
                    // Drag to reorder (the strip drops it in the gap); drop a
                    // tab on a folder to file it.
                    .drag_threshold(TAB_DRAG_PX)
                    .on_drag(TabDrag { id, name: tab_name.clone(), icon: Some(tab_icon(kind)), width: bounds.get().size.width }, start_tab_drag(cx.entity().downgrade()))
                    .on_drag_move(gap_here)
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        if compare_off {
                            return;
                        }
                        // Ctrl/Cmd+click or Shift+click: the tab in a new window, this one stays (live: a new browser
                        // tab / window; the tab menu's Open in new browser tab).
                        if e.modifiers().secondary() || e.modifiers().shift {
                            if let Err(e) = crate::files::open_tab_window(this.doc.clone(), id, this.gpu.clone(), cx) {
                                log::warn!("Could not open a window: {e}");
                            }
                            return;
                        }
                        // The first click activated the tab; a double-click does nothing more (live 2279 (b): no
                        // rename, Rename is in the tab's menu).
                        if e.click_count() < 2 {
                            this.activate(id, window, cx);
                        }
                    }))
                    .tab_context_menu(bounds, move |menu, _, cx| {
                        use crate::doc_dialogs as dd;
                        use crate::os_menu::OsMenuItem as I;
                        if kind.starts_with("file") {
                            return file_tab_menu(menu, &view, id, &tab_name, cx);
                        }
                        let (v1, v2, v3) = (view.clone(), view.clone(), view.clone());
                        let (v5, v6, v7, v8, v9, v10, v11) = (view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone());
                        // A protected workspace (live wp-933, ver-protected-tab-menu): Open in new browser tab,
                        // Properties…, Show code, Copy to clipboard, Export….
                        if view.read(cx).doc.read(cx).session.is_protected() {
                            let v13 = view.clone();
                            let menu = menu.item(I::new("Open in new browser tab").on_click(move |_, _, cx| {
                                let (doc, gpu) = (v13.read(cx).doc.clone(), v13.read(cx).gpu.clone());
                                if let Err(e) = crate::files::open_tab_window(doc, id, gpu, cx) {
                                    log::warn!("Could not open a window: {e}");
                                }
                            }));
                            let menu = menu.item(I::new("Properties…").on_click(move |_, w, cx| {
                                let doc = v6.read(cx).doc.clone();
                                let thumb = v6.read(cx).tab_thumbs.read(cx).picture(id).flatten();
                                dd::tab_properties(doc, id, thumb, w, cx)
                            }));
                            let menu = if kind == "partstudio" {
                                menu.item(I::new("Show code").on_click(move |_, w, cx| v7.update(cx, |t, cx| t.show_code(id, w, cx))))
                            } else {
                                menu
                            };
                            let menu = menu.item(I::new("Copy to clipboard").on_click(move |_, _, cx| {
                                let t = v8.read(cx).doc.read(cx).session.doc.tab(id).cloned();
                                cx.set_global(dd::CopiedTab(t));
                            }));
                            return if kind == "drawing" { menu.item(I::new("Export…").on_click(move |_, w, cx| v11.update(cx, |t, cx| t.open_export(id, w, cx)))) } else { menu };
                        }
                        // Onshape (live ui2-tabmenu-*): ⊗ Delete | Open in new browser
                        // tab, Rename, Properties…, Show code | Duplicate, Copy to
                        // clipboard | (active Drawing) Change to version… |
                        // (Assembly) Create Drawing of <tab>… | Select as document
                        // thumbnail | Move to document…, (Drawing) Export…, Release….
                        let v13 = view.clone();
                        let menu = menu
                            .item(I::new("Delete").icon_name("menu-delete").on_click(move |_, w, cx| v2.update(cx, |t, cx| t.delete_tab(id, w, cx))))
                            .separator()
                            .item(I::new("Open in new browser tab").on_click(move |_, _, cx| {
                                let (doc, gpu) = (v13.read(cx).doc.clone(), v13.read(cx).gpu.clone());
                                if let Err(e) = crate::files::open_tab_window(doc, id, gpu, cx) {
                                    log::warn!("Could not open a window: {e}");
                                }
                            }))
                            .item(I::new("Rename").on_click(move |_, w, cx| v5.update(cx, |t, cx| t.start_tab_rename(id, w, cx))))
                            .item(I::new("Properties…").on_click(move |_, w, cx| {
                                let doc = v6.read(cx).doc.clone();
                                let thumb = v6.read(cx).tab_thumbs.read(cx).picture(id).flatten();
                                dd::tab_properties(doc, id, thumb, w, cx)
                            }));
                        let menu = if kind == "partstudio" {
                            menu.item(I::new("Show code").on_click(move |_, w, cx| v7.update(cx, |t, cx| t.show_code(id, w, cx))))
                        } else {
                            menu
                        };
                        // In a folder: Move to parent folder (live STEP tab in CAD Imports).
                        let menu = if view.read(cx).doc.read(cx).session.doc.parent_of(id).is_some() {
                            let v14 = view.clone();
                            menu.item(I::new("Move to parent folder").on_click(move |_, _, cx| v14.update(cx, |t, cx| t.move_to_parent(id, cx))))
                        } else {
                            menu
                        };
                        // A Material Library has no Duplicate or Copy to clipboard; its Update… comes before Move to
                        // document… (live doc-material-library-tab-menu).
                        let menu = if kind == "materiallibrary" {
                            let d2 = view.read(cx).doc.clone();
                            menu.separator().item(I::new("Update…").on_click(move |_, w, cx| crate::material_library_view::update(d2.clone(), id, w, cx)))
                        } else {
                            menu.separator()
                                .item(I::new("Duplicate").on_click(move |_, _, cx| v1.update(cx, |t, cx| t.duplicate_tab(id, cx))))
                                .item(I::new("Copy to clipboard").on_click(move |_, _, cx| {
                                    let t = v8.read(cx).doc.read(cx).session.doc.tab(id).cloned();
                                    cx.set_global(dd::CopiedTab(t));
                                }))
                                .separator()
                        };
                        // The active Drawing: Change to version… (live tab menu, after
                        // Copy to clipboard; not on an inactive Drawing tab): point
                        // every view at a version of the document.
                        let menu = if kind == "drawing" && active {
                            let v12 = view.clone();
                            menu.item(I::new("Change to version…").icon_name("change-version").on_click(move |_, w, cx| v12.update(cx, |t, cx| t.change_drawing_version(id, w, cx)))).separator()
                        } else {
                            menu
                        };
                        // Live menus/rmb-tab-assembly: its own group after Copy to clipboard.
                        let menu = if kind == "assembly" {
                            let n = tab_name.clone();
                            menu.item(I::new(format!("Create Drawing of {tab_name}…")).icon_name("menu-create-drawing").on_click(move |_, w, cx| {
                                let source = Some(noshape_model::drawing::create::DrawingSource { tab: id, part: None, config: Default::default(), sketch: None, flat: false });
                                let req = crate::drawing_create::CreateRequest { external: None, source, name: Some(n.clone()) };
                                v3.update(cx, |t, cx| t.open_create_drawing(req, w, cx));
                            }))
                            .separator()
                        } else {
                            menu
                        };
                        // Onshape: only tabs with a picture can be the document thumbnail
                        // (the Feature Studio menu has no such item, menus/rmb-tab-feature-studio).
                        let menu = if matches!(kind, "partstudio" | "assembly" | "drawing") {
                            menu.item(I::new("Select as document thumbnail").on_click(move |_, _, cx| v9.update(cx, |t, cx| t.select_thumbnail(id, cx)))).separator()
                        } else {
                            menu
                        };
                        let menu = menu
                            .item(I::new("Move to document…").on_click(move |_, w, cx| {
                                let doc = v10.read(cx).doc.clone();
                                dd::move_to_document(doc, vec![id], w, cx)
                            }));
                        // Only a Drawing tab ends with Export… (live2 11-13 05:05: no longer on Part Studio or Assembly tabs).
                        let menu = if kind == "drawing" { menu.item(I::new("Export…").on_click(move |_, w, cx| v11.update(cx, |t, cx| t.open_export(id, w, cx)))) } else { menu };
                        // Release management (help image Release_highlighted2, live
                        // rmb-tab-assembly / -drawing: the last entry, after Export…):
                        // Release… for Assemblies, Drawings, Variable Studios and files
                        // (Part Studios release their parts, not the tab: live
                        // rmb-tab-partstudio-2 has none); Revision history once one
                        // has revisions.
                        let menu = if matches!(kind, "assembly" | "drawing" | "variables") || kind.starts_with("file") {
                            let (d1, d2) = (view.read(cx).doc.clone(), view.read(cx).doc.clone());
                            let pn = crate::release_dialog::tab_part_number(d1.read(cx), id);
                            let has_revisions = !pn.is_empty() && !crate::release_dialog::release_log(d1.read(cx)).revisions(&pn).is_empty();
                            let menu = menu.item(I::new("Release…").on_click(move |_, w, cx| {
                                let items = crate::release_dialog::tab_items(d1.read(cx), id);
                                crate::release_dialog::open_release_candidate(d1.clone(), items, w, cx)
                            }));
                            if has_revisions && matches!(kind, "assembly" | "drawing" | "variables") {
                                menu.item(I::new("Revision history").on_click(move |_, w, cx| crate::release_dialog::open_revision_history(d2.clone(), pn.clone(), w, cx)))
                            } else {
                                menu
                            }
                        } else {
                            menu
                        };
                        // Material Library tabs end with Export (a CSV download, no dialog: live doc-material-library-tab-menu).
                        if kind == "materiallibrary" {
                            let d1 = view.read(cx).doc.clone();
                            menu.item(I::new("Export").on_click(move |_, w, cx| crate::material_library_view::export(d1.clone(), id, w, cx)))
                        } else {
                            menu
                        }
                    }),
            );
        }
        if gap.is_some_and(|(_, b)| b == Some(None)) {
            strip = strip.child(placeholder());
        }
        // Live: a 100 px ghost (#eaeaea, shadow 0 5px 10px .2, the label
        // truncated) under the pointer, along the bar (it starts at x 0).
        let ghost = self.tab_gap.as_ref().map(|g| {
            h_flex()
                .absolute()
                .top_0()
                .left(g.x - g.grab.min(px(90.)))
                .w(px(100.))
                .h(px(27.))
                .px(px(10.))
                .gap(px(3.))
                .rounded_b(px(5.))
                .bg(gpui_kit::rgb(0xeaeaea))
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_size(px(14.))
                .text_color(pal.text)
                .child(div().flex_none().child(match g.icon {
                    Some(i) => crate::assets::icon(i, 20., cx),
                    None => os::glyph(os::GLYPH_FOLDER, 20., pal.text).into_any_element(),
                }))
                .child(div().min_w_0().truncate().child(crate::ui::user_text(g.name.clone())))
        });
        let arrows = self.render_tab_scroll_buttons(cx);
        // The strip's overflow is known only after layout, and the bar is a
        // cached slice: when it differs from what this render assumed (tabs
        // added, window resized, wheel-scrolled to an end), render again.
        let rendered = tab_scroll_state(&self.tab_scroll);
        let (handle, ws) = (self.tab_scroll.clone(), cx.entity().downgrade());
        let watch = canvas(|_, _, _| {}, move |_, _, _, cx| {
            if tab_scroll_state(&handle) != rendered {
                let ws = ws.clone();
                cx.defer(move |cx| {
                    let _ = ws.update(cx, |_, cx| cx.notify());
                });
            }
        })
        .absolute()
        .size(px(1.));
        // Full width: inside the cached slice nothing stretches the row, and
        // the strip must overflow (and scroll) within the window.
        (tab_list.into_any_element(), h_flex().relative().w_full().min_w_0().h(px(crate::ui::TAB_BAR_H)).flex_none().bg(pal.tab_bar).child(add).children(crumbs_el).child(strip).children(arrows).child(watch).children(ghost).into_any_element())
    }

    /// Live (behaviours "Document tab bar scroll buttons"): ‹ › at the right
    /// end of the bar, 24x30 each, only while the tabs overflow; the one at
    /// an end dimmed to 0.4. A press scrolls about a page, a right press to
    /// the first / last tab. (The wheel scrolls the strip already.)
    fn render_tab_scroll_buttons(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (overflow, at_first, at_last) = tab_scroll_state(&self.tab_scroll);
        if !overflow {
            return None;
        }
        let button = |left: bool, enabled: bool, cx: &mut Context<Self>| {
            div()
                .id(if left { "tab-scroll-left" } else { "tab-scroll-right" })
                .map(|d| noshape_app_core::ui_script::mark(d, move || if left { "button:Scroll left".into() } else { "button:Scroll right".into() }))
                .w(px(24.))
                .h(px(crate::ui::TAB_BAR_H))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .when(!enabled, |d| d.opacity(0.4))
                .child(crate::ui::glyph(if left { "ui-chevron-left" } else { "ui-chevron-right" }, 22., gpui_kit::rgb(0x555555)))
                .tip(if left { "Scroll left (RMB: to first)" } else { "Scroll right (RMB: to last)" }, TipSide::Above)
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| this.scroll_tabs(left, false, cx)))
                .on_mouse_down(MouseButton::Right, cx.listener(move |this, _: &MouseDownEvent, _, cx| this.scroll_tabs(left, true, cx)))
        };
        Some(h_flex().flex_none().child(button(true, !at_first, cx)).child(button(false, !at_last, cx)).into_any_element())
    }

    /// A frame of a tab drag's scroll at an end of the strip; false once the drag is over.
    fn autoscroll_tabs(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(g) = &mut self.tab_gap else { return false };
        let now = std::time::Instant::now();
        let secs = g.scrolled_at.replace(now).map_or(1. / 60., |t| now.duration_since(t).as_secs_f32().min(0.1));
        if g.scroll == 0. {
            g.scrolled_at = None;
        } else {
            let (mut off, max) = (self.tab_scroll.offset(), self.tab_scroll.max_offset().x);
            let x = autoscroll_offset(off.x, max, g.scroll, secs);
            if x != off.x {
                off.x = x;
                self.tab_scroll.set_offset(off);
                cx.notify();
            }
        }
        true
    }

    /// Scrolls the tab strip a page (about the visible width less one tab),
    /// or to its first / last tab.
    fn scroll_tabs(&mut self, left: bool, to_end: bool, cx: &mut Context<Self>) {
        let (mut off, max) = (self.tab_scroll.offset(), self.tab_scroll.max_offset().x);
        let page = (self.tab_scroll.bounds().size.width - px(201.)).max(px(201.));
        off.x = match (left, to_end) {
            (true, true) => px(0.),
            (false, true) => -max,
            (true, false) => (off.x + page).min(px(0.)),
            (false, false) => (off.x - page).max(-max),
        };
        self.tab_scroll.set_offset(off);
        cx.notify();
    }

    /// Onshape's left document-panel rail (§3.3): 40 px `#fafafa`.
    fn render_rail(&mut self, tab_list: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let history_open = self.history.is_some();
        let notes_open = matches!(self.doc_panel, Some((DocPanel::Notes(_), _)));
        let perf_open = matches!(self.doc_panel, Some((DocPanel::Performance(_), _)));
        let where_open = matches!(self.doc_panel, Some((DocPanel::WhereUsed(_), _)));
        let paused = self.active_part_studio().is_some_and(|v| v.read(cx).regen_paused());
        // At a version there is no Create version… (live psui/q2485: 6 icons become 5).
        let at_version = self.doc.read(cx).session.viewing_version().is_some();
        let perf_error = crate::doc_panels::PerfSnapshot { adapter: self.gpu.adapter_name(), ..Default::default() }.software_renderer();
        // Live (part-studio/181): the icons' ink starts at x 10, y 49 (20 px
        // icons): 2 px under the rail's top border, and centred in the 40 px
        // as if the 1 px right border weren't there (each button pads 1 px at
        // the left; its active bar stays at the edge).
        let buttons = v_flex()
            .w(px(crate::ui::RAIL_W))
            .flex_1()
            .min_h_0()
            .pt(px(2.))
            .gap(px(4.))
            .bg(pal.rail)
            .border_r_1()
            .border_t_1()
            .border_color(pal.border)
            .child(
                crate::ui::rail_button("rail-history", "history", history_open, true, cx).pl(px(1.))
                    .tip("Versions and history", TipSide::Right)
                    .map(|d| noshape_app_core::ui_script::mark(d, || "button:Versions and history".into()))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_history(window, cx))),
            )
            // Live 10-27: paused regeneration drops Create version… (and
            // Comments).
            .when(!paused && !at_version, |d| d.child(
                crate::ui::rail_button("rail-version", "version", false, true, cx).pl(px(1.))
                    .tip("Create version…", TipSide::Right)
                    .map(|d| noshape_app_core::ui_script::mark(d, || "button:Create version…".into()))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| crate::history_panel::open_create_version(this.doc.clone(), window, cx))),
            ))
            // Live order: Versions and history, Create version…, (Comments),
            // Document notes, (Where used), Performance, (AI Advisor).
            .child(
                crate::ui::rail_button("rail-notes", "doc-notes", notes_open, true, cx).pl(px(1.))
                    .tip("Document notes", TipSide::Right)
                    .map(|d| noshape_app_core::ui_script::mark(d, || "button:Document notes".into()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_doc_panel(true, cx))),
            )
            .child(
                crate::ui::rail_button("rail-where-used", "where-used", where_open, true, cx).pl(px(1.))
                    .tip("Where used", TipSide::Right)
                    .map(|d| noshape_app_core::ui_script::mark(d, || "button:Where used".into()))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_where_used(window, cx))),
            )
            .child(
                crate::ui::rail_button("rail-performance", "performance", perf_open, true, cx).pl(px(1.))
                    // A software renderer is a performance error (live: a red badge).
                    .when(perf_error, |d| d.child(div().absolute().left(px(22.)).top(px(16.)).size(px(11.)).rounded_full().bg(gpui_kit::rgb_static(0xffffff)).child(crate::assets::icon("error", 11., cx))))
                    .tip(if perf_error { "Performance errors detected" } else { "Performance" }, TipSide::Right)
                    .map(|d| noshape_app_core::ui_script::mark(d, || "button:Performance".into()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_doc_panel(false, cx))),
            )
            // Release management (help releasedraftspanel.png).
            .child(
                crate::ui::rail_button("rail-release-drafts", "release-drafts", self.release_drafts.is_some(), true, cx).pl(px(1.))
                    .tip("Release drafts", TipSide::Right)
                    .map(|d| noshape_app_core::ui_script::mark(d, || "button:Release drafts".into()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_release_drafts(cx))),
            );
        // The Tab manager cell ends the rail, beside the tab bar.
        v_flex().w(px(crate::ui::RAIL_W)).h_full().flex_none().child(buttons).child(tab_list).into_any_element()
    }
}

/// The left rail's document panels.
enum DocPanel {
    Notes(Entity<crate::doc_panels::NotesPanel>),
    Performance(Entity<crate::doc_panels::PerformancePanel>),
    WhereUsed(Entity<crate::doc_panels::WhereUsedPanel>),
}

/// A file tab's right-click menu (live menus/rmb-tab-file-*, behaviours
/// "File tabs" and "Imported DWG/DXF drawings"): Delete | Rename,
/// Properties… | (CAD) Import…, (DXF / DWG) Create Drawing of <file>…,
/// Update… | (images) Select as document thumbnail | Move to document… |
/// (CAD) Export… | Release… | Download (and Open in new browser tab, as every tab). No
/// Translate….
fn file_tab_menu(menu: crate::os_menu::OsMenu, view: &Entity<Workspace>, id: TabId, tab_name: &str, cx: &App) -> crate::os_menu::OsMenu {
    use crate::doc_dialogs as dd;
    use crate::os_menu::OsMenuItem as I;
    use noshape_model::file_tab::FileKind;
    let doc = view.read(cx).doc.clone();
    let file_kind = match doc.read(cx).session.doc.tab(id).map(|t| &t.content) {
        Some(TabContent::File(f)) => Some(f.kind()),
        _ => None,
    };
    let (v1, v2, v3, v4, v5) = (view.clone(), view.clone(), view.clone(), view.clone(), view.clone());
    let (d1, d2, d3, d4) = (doc.clone(), doc.clone(), doc.clone(), doc.clone());
    let (d0, gpu) = (doc.clone(), view.read(cx).gpu.clone());
    let mut menu = menu
        .item(I::new("Delete").icon_name("menu-delete").on_click(move |_, w, cx| v1.update(cx, |t, cx| t.delete_tab(id, w, cx))))
        .separator()
        .item(I::new("Open in new browser tab").on_click(move |_, _, cx| {
            if let Err(e) = crate::files::open_tab_window(d0.clone(), id, gpu.clone(), cx) {
                log::warn!("Could not open a window: {e}");
            }
        }))
        .item(I::new("Rename").on_click(move |_, w, cx| v2.update(cx, |t, cx| t.start_tab_rename(id, w, cx))))
        .item(I::new("Properties…").on_click(move |_, w, cx| dd::tab_properties(d1.clone(), id, None, w, cx)));
    if doc.read(cx).session.doc.parent_of(id).is_some() {
        let v6 = view.clone();
        menu = menu.item(I::new("Move to parent folder").on_click(move |_, _, cx| v6.update(cx, |t, cx| t.move_to_parent(id, cx))));
    }
    menu = menu.separator();
    match file_kind {
        Some(FileKind::Cad) => menu = menu.item(I::new("Import…").on_click(move |_, w, cx| v3.update(cx, |t, cx| t.translate_file(id, w, cx)))),
        Some(FileKind::Drawing2d) => menu = menu.item(I::new(format!("Create Drawing of {tab_name}…")).icon_name("menu-create-drawing").on_click(move |_, w, cx| v3.update(cx, |t, cx| t.create_cad_drawing(id, w, cx)))),
        _ => {}
    }
    menu = menu.item(I::new("Update…").on_click(move |_, w, cx| crate::file_tab_view::update(&d2, id, w, cx))).separator();
    if file_kind == Some(FileKind::Image) {
        menu = menu.item(I::new("Select as document thumbnail").on_click(move |_, _, cx| v4.update(cx, |t, cx| t.select_thumbnail(id, cx)))).separator();
    }
    menu = menu.item(I::new("Move to document…").on_click(move |_, w, cx| dd::move_to_document(d3.clone(), vec![id], w, cx))).separator();
    // A CAD file converts to another format (live document/94).
    if file_kind == Some(FileKind::Cad) {
        let v7 = view.clone();
        menu = menu.item(I::new("Export…").on_click(move |_, w, cx| v7.update(cx, |t, cx| t.export_file(id, w, cx)))).separator();
    }
    menu.item(I::new("Release…").on_click(move |_, w, cx| {
            let items = crate::release_dialog::tab_items(d4.read(cx), id);
            crate::release_dialog::open_release_candidate(d4.clone(), items, w, cx)
        }))
        .separator()
        .item(I::new("Download").on_click(move |_, w, cx| {
            let doc = v5.read(cx).doc.clone();
            crate::file_tab_view::download(&doc, id, w, cx)
        }))
}

/// The switcher after one more Ctrl+Space: opened on the session's tabs
/// (the active one, then the previously used, most recent first) with the
/// previous one selected, else moved one on, wrapping to the first.
fn switcher_next(open: Option<(Vec<TabId>, usize)>, active: Option<TabId>, recent: &[TabId], tabs: &[TabId]) -> Option<(Vec<TabId>, usize)> {
    match open {
        Some((list, i)) => {
            let n = list.len();
            Some((list, (i + 1) % n))
        }
        None => {
            let list: Vec<TabId> = active.into_iter().chain(recent.iter().copied()).filter(|t| tabs.contains(t)).collect();
            (!list.is_empty()).then(|| (list.clone(), 1.min(list.len() - 1)))
        }
    }
}

/// A drawing whose views are pinned to a version of this document older
/// than its latest one.
fn drawing_behind(doc: &Entity<crate::doc_model::DocModel>, tab: TabId, cx: &App) -> bool {
    let d = doc.read(cx);
    let Some(TabContent::Drawing(dr)) = d.session.doc.tab(tab).map(|t| &t.content) else { return false };
    let pins: Vec<&str> = dr.sheets.iter().flat_map(|s| &s.views).filter_map(|v| v.pinned.as_ref()).map(|p| p.id.as_str()).filter(|id| !id.starts_with("ext:")).collect();
    if pins.is_empty() {
        return false;
    }
    let latest = d.session.store.list_versions().ok().and_then(|v| v.last().map(|v| v.id.to_string()));
    latest.is_some_and(|l| pins.iter().any(|p| *p != l))
}

fn tab_icon(kind: &'static str) -> &'static str {
    match kind {
        // Live (tabbar captures): the tab icons are white inside.
        "partstudio" => "tab-part-studio",
        "assembly" => "tab-assembly",
        "drawing" => "tab-drawing",
        "featurestudio" => "feature-studio",
        "materiallibrary" => "material-library",
        // File tabs: the kind is the icon ("file-cad", "file-pdf"...).
        k if k.starts_with("file") => k,
        _ => "variable-studio",
    }
}

/// Navbar buttons: transparent, `#d7d7d7` hover over the full bar height.
fn navbar_variant(cx: &App) -> gpui_kit::component::button::ButtonCustomVariant {
    let pal = p();
    gpui_kit::component::button::ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.border.into()).active(pal.border.into()).shadow(false)
}

fn kind_of(c: &TabContent) -> &'static str {
    match c {
        TabContent::PartStudio(_) => "partstudio",
        TabContent::Assembly(_) => "assembly",
        TabContent::Drawing(_) => "drawing",
        TabContent::VariableStudio(_) => "variables",
        TabContent::FeatureStudio(_) => "featurestudio",
        TabContent::MaterialLibrary(_) => "materiallibrary",
        TabContent::File(f) => crate::file_tab_view::file_icon(f),
    }
}

/// A tab's kind as the ui-parity state names it (live's element types).
fn ui_tab_kind(c: &TabContent) -> &'static str {
    match c {
        TabContent::PartStudio(_) => "partstudio",
        TabContent::Assembly(_) => "assembly",
        TabContent::Drawing(_) => "drawing",
        TabContent::VariableStudio(_) => "variablestudio",
        TabContent::FeatureStudio(_) => "featurestudio",
        TabContent::MaterialLibrary(_) => "materiallibrary",
        // Live's element types for the file kinds its recordings show; "file" for the others.
        TabContent::File(f) => match f.file_name.rsplit('.').next().map(str::to_lowercase).as_deref() {
            Some("png" | "jpg" | "jpeg" | "gif" | "bmp") => "image-element",
            Some("pdf") => "pdf-element",
            Some("step" | "stp") => "step-element",
            Some("txt") => "txt-element",
            _ => "file",
        },
    }
}

/// Whether the tab strip overflows, and is scrolled to its first / last tab.
fn tab_scroll_state(h: &ScrollHandle) -> (bool, bool, bool) {
    let (off, max) = (h.offset().x, h.max_offset().x);
    (max > px(0.5), off > px(-0.5), off < -max + px(0.5))
}

/// The workspace's cached chrome (see `crate::cached_slice`).
#[derive(Clone)]
struct WsSlices {
    title: Entity<crate::cached_slice::Slice<Workspace>>,
    rail: Entity<crate::cached_slice::Slice<Workspace>>,
    tabs: Entity<crate::cached_slice::Slice<Workspace>>,
}

impl Workspace {
    fn ws_slices(&mut self, cx: &mut Context<Self>) -> WsSlices {
        if let Some(s) = &self.slices {
            return s.clone();
        }
        use crate::cached_slice::Slice;
        let me = cx.entity();
        let title = Slice::new(&me, |this, _, cx| this.render_title_bar(cx), cx);
        let rail = Slice::new(&me, |this, _, cx| {
            let (tab_list, _) = this.render_tab_bar(cx);
            this.render_rail(tab_list, cx)
        }, cx);
        let tabs = Slice::new(&me, |this, _, cx| this.render_tab_bar(cx).1, cx);
        // They show the document (name, tabs, versions).
        for s in [&title, &rail, &tabs] {
            Slice::observing(s, &self.doc, cx);
        }
        let s = WsSlices { title, rail, tabs };
        self.slices = Some(s.clone());
        s
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The window has drawn: the viewport's shaders compile now.
        static FIRST: std::sync::Once = std::sync::Once::new();
        FIRST.call_once(|| {
            noshape_render::worker::release_prewarm();
            noshape_render::perf::milestone("first workspace render");
        });
        let pal = p();
        crate::assets::set_scale_factor(_window.scale_factor());
        let count = self.selection_cursor_count(cx);
        let want = (self.navbar_hovered && count > 0 && _window.hovered_cursor_style().is_none()).then(|| crate::cursor::AppCursor::count(count));
        crate::cursor::CursorSync::sync(self, want, _window, cx, |w| &mut w.cursor_sync);
        // The navbar, rail and tab bar are cached: a keystroke or a caret blink
        // in a tab re-renders the workspace but reuses them.
        let ws = self.ws_slices(cx);
        let title = ws.title.cached(StyleRefinement::default().w_full().h(px(crate::ui::NAVBAR_H)).flex_none());
        let rail = ws.rail.cached(StyleRefinement::default().w(px(crate::ui::RAIL_W)).h_full().flex_none());
        let tabs = ws.tabs.cached(StyleRefinement::default().w_full().h(px(crate::ui::TAB_BAR_H)).flex_none());
        if let Some((DocPanel::Performance(panel), _)) = &self.doc_panel {
            let snap = self.perf_snapshot(cx);
            panel.update(cx, |p, cx| p.set_snapshot(snap, cx));
        }
        if let Some((panel, _)) = &self.fs_notices {
            let results: HashMap<TabId, String> = self.views.iter().filter_map(|(t, v)| match v {
                TabView::PartStudio(v) => Some((*t, v.read(cx).regen_result())),
                _ => None,
            }).collect();
            panel.update(cx, |p, cx| p.set_results(results, cx));
        }
        // Compare mode (compare_mode.rs, live ui2/952, cp-923): "NOT
        // COMPARABLE" for tabs compare doesn't cover, the code compare for a
        // Feature Studio.
        crate::compare_mode::follow(&self.doc, cx);
        let compare_view = self.active.and_then(|a| crate::compare_mode::content_override(&self.doc, a, cx));
        let content: AnyElement = match self.active.and_then(|a| self.views.get(&a)) {
            _ if compare_view.is_some() => compare_view.unwrap_or_else(|| div().into_any_element()),
            Some(TabView::PartStudio(v)) => v.clone().into_any_element(),
            Some(TabView::Drawing(v)) => v.clone().into_any_element(),
            Some(TabView::Variables(v)) => v.clone().into_any_element(),
            // The assembly's left dialogs (Insert, Mate, …) and what floats
            // over them are drawn here, in front of its view rather than in
            // it, and the view is a cached view (see `crate::cached_slice`),
            // so their fields' keystrokes and caret blinks don't render the
            // assembly again (`AssemblyView::front_host`).
            Some(TabView::Assembly(v)) => {
                let (front, top) = v.update(cx, |v, cx| v.front_host(cx));
                div()
                    .size_full()
                    .relative()
                    .child(v.clone().cached(StyleRefinement::default().size_full()))
                    .child(div().absolute().top(px(top)).left_0().right_0().bottom_0().child(front))
                    .into_any_element()
            }
            Some(TabView::FeatureStudio(v)) => v.clone().into_any_element(),
            Some(TabView::MaterialLibrary(v)) => v.clone().into_any_element(),
            Some(TabView::File(v)) => v.clone().into_any_element(),
            None => div().into_any_element(),
        };
        v_flex()
            .size_full()
            .bg(pal.app_bg)
            .text_color(pal.text)
            .key_context("Workspace")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Undo, _, cx| this.doc.update(cx, |d, cx| d.undo(cx))))
            .on_action(cx.listener(|this, _: &Redo, _, cx| this.doc.update(cx, |d, cx| d.redo(cx))))
            .on_action(cx.listener(|this, _: &ToggleVersionGraph, window, cx| this.toggle_history(window, cx)))
            .on_action(cx.listener(|this, _: &SearchFeatureStudios, window, cx| this.open_fs_search(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleReleaseDrafts, _, cx| this.toggle_release_drafts(cx)))
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .on_action(cx.listener(|this, _: &crate::actions::FocusNavbar, window, cx| window.focus(&this.notices_focus, cx)))
            // Tool search from anywhere in the window (e.g. after clicking the tab bar).
            .on_action(cx.listener(|this, _: &SearchTools, window, cx| {
                if let Some(s) = this.active_studio() {
                    s.update(cx, |s, cx| s.open_palette(window, cx));
                }
            }))
            // Ctrl+Space: the tab switcher (live spec "Ctrl+Space quick tab
            // switcher"): the first Space selects the previous tab, each
            // further one the next (wrapping); releasing Ctrl switches.
            .on_action(cx.listener(|this, _: &SwitchRecentTab, _, cx| this.switcher_step(cx)))
            .on_modifiers_changed(cx.listener(|this, ev: &ModifiersChangedEvent, window, cx| {
                if !ev.modifiers.control {
                    this.switcher_commit(window, cx);
                }
            }))
            // Alt+T: Tab manager (search and open tabs).
            .on_action(cx.listener(|this, _: &TabManager, window, cx| this.toggle_tab_manager(window, cx)))
            // A customized key of an action without its own GPUI action.
            .on_action(cx.listener(|this, a: &crate::shortcuts::RunShortcut, window, cx| this.run_shortcut(a.id, window, cx)))
            // Create version (no default key in live, live2 2405; a customized one runs it): the Create version dialog.
            .on_action(cx.listener(|this, _: &CreateVersion, window, cx| crate::history_panel::open_create_version(this.doc.clone(), window, cx)))
            .on_action(cx.listener(|this, _: &ExportRequestedItem, window, cx| {
                if let Some((t, parts)) = REQUESTED_EXPORT.lock().take() {
                    this.export_item(t, parts, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ActivateRequestedTab, window, cx| {
                if let Some(t) = crate::actions::take_requested_tab() {
                    this.activate(t, window, cx);
                    if let Some((line, column)) = crate::actions::take_requested_caret()
                        && let Some(TabView::FeatureStudio(v)) = this.views.get(&t)
                    {
                        v.update(cx, |v, cx| v.goto_line(line, column, window, cx));
                    }
                    if let Some(selection) = crate::actions::take_requested_selection()
                        && let Some(TabView::PartStudio(v)) = this.views.get(&t)
                    {
                        v.update(cx, |v, cx| {
                            v.selection = selection;
                            cx.notify();
                        });
                    }
                }
            }))
            .on_action(cx.listener(|this, _: &CreateDrawing, window, cx| {
                if let Some(req) = crate::drawing_create::take_request() {
                    this.open_create_drawing(req, window, cx);
                }
            }))
            .on_key_down(cx.listener(Self::forward_chrome_key))
            .child(div().w_full().flex_none().track_focus(&self.chrome_focus[0]).child(title))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(rail)
                    // One left panel at a time: the Tab manager or Versions and history.
                    .children(match &self.tab_manager {
                        Some((tm, _)) => Some(tm.clone().into_any_element()),
                        None => self.history.clone().map(|h| h.into_any_element()).or_else(|| self.release_drafts.clone().map(|d| d.into_any_element())).or_else(|| {
                            self.doc_panel.as_ref().map(|(p, _)| match p {
                                DocPanel::Notes(n) => n.clone().into_any_element(),
                                DocPanel::Performance(v) => v.clone().into_any_element(),
                                DocPanel::WhereUsed(w) => w.clone().into_any_element(),
                            })
                        }),
                    }.map(|panel| div().w(px(crate::history_panel::PANEL_W)).h_full().flex_none().border_r_1().border_color(pal.border).child(panel)))
                    // Onshape: the left panels run the window's full height;
                    // the tab bar sits under the content only.
                    .child(
                        v_flex().flex_1().h_full().min_w_0().child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .relative()
                                .child(content)
                                // Compare mode: the toolbar row becomes white with the
                                // centred "Comparing Main and V3" bar (live 953).
                                .children(crate::compare_mode::render_bar(&self.doc, cx).map(|bar| {
                                    div().absolute().top_0().left_0().right_0().h(px(crate::ui::TOOLBAR_H)).bg(crate::theme::p().panel).occlude().flex().justify_center().child(bar)
                                }))
                                // A protected workspace (workspace_protection.rs): its banner
                                // at the top centre (not over the compare bar).
                                .children(crate::workspace_protection::render_banner(&self.doc, matches!(self.active.and_then(|a| self.views.get(&a)), Some(TabView::PartStudio(_))), cx).filter(|_| crate::compare_mode::rows(&self.doc, cx).is_none()).map(|banner| {
                                    div().absolute().top_0().left_0().right_0().h(px(crate::ui::TOOLBAR_H)).bg(crate::theme::p().panel).occlude().flex().justify_center().child(banner)
                                }))
                                .children(self.render_toast(cx))
                                .children(self.render_switcher(cx))
                                // Onshape: at the feature dialog's place (below the toolbar, right of the list).
                                .children(self.units_panel.as_ref().map(|(p, _)| div().absolute().top(px(39.)).left(px(crate::ui::left_panel_w())).child(p.clone()))),
                        )
                        .child(div().w_full().flex_none().track_focus(&self.chrome_focus[1]).child(tabs))
                        // Live document/75: the notices panel sits under the tab bar,
                        // right of the left rail (the rail runs the full height).
                        .children(self.fs_notices.as_ref().map(|(p, _)| p.clone())),
                    ),
            )
            .children(self.fs_search.as_ref().map(|(p, _)| p.clone()))
            .children(self.create_drawing.as_ref().map(|(d, _)| d.clone()))
            // The bell's panel under the navbar's right end.
            .children(self.notifications_open.then(|| {
                let ws = cx.entity();
                div().absolute().top(px(crate::ui::NAVBAR_H)).right(px(40.)).child(crate::notifications::panel(
                    move |_, cx| {
                        ws.update(cx, |w, cx| {
                            w.notifications_open = false;
                            cx.notify();
                        })
                    },
                    cx,
                ))
            }))
            // Onshape right-click menus opened with `os_context_menu`.
    }
}

impl Workspace {
    /// Ctrl+Space: opens the switcher on the previously used tab, or moves it
    /// one tab on (back to the first after the last).
    fn switcher_step(&mut self, cx: &mut Context<Self>) {
        let opening = self.switcher.is_none();
        let tabs: Vec<TabId> = self.doc.read(cx).session.doc.ordered_tabs().iter().map(|t| t.id).collect();
        self.switcher = switcher_next(self.switcher.take(), self.active, &self.recent, &tabs);
        if let Some((list, _)) = self.switcher.as_ref().filter(|_| opening) {
            let list = list.clone();
            self.tab_thumbs.update(cx, |th, cx| list.iter().for_each(|t| th.request(*t, cx)));
        }
        cx.notify();
    }

    /// Ctrl released (or a thumbnail clicked): the selected tab becomes active.
    fn switcher_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((list, i)) = self.switcher.take() else { return };
        if let Some(t) = list.get(i).copied().filter(|t| Some(*t) != self.active) {
            self.activate(t, window, cx);
        }
        cx.notify();
    }

    /// The Ctrl+Space card (live part-studio/ks-switch-1, -2,
    /// ks-ctrlspace-held): white, 1 px `#ccc`, rounded, centred 10 px above
    /// the tab bar; 180 x 121 thumbnails 20 px apart, the selected one on
    /// `#cedbe5` with its tab's icon and name under it; ‹ › at the ends, a
    /// bar where there is nothing more that way. Hovering a thumbnail selects
    /// it, a click on it switches.
    fn render_switcher(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (list, sel) = self.switcher.as_ref()?;
        let sel = *sel;
        let pal = p();
        let doc = self.doc.read(cx).session.doc.clone();
        let thumbs = self.tab_thumbs.clone();
        let end = |more: bool, left: bool, cx: &mut Context<Self>| -> AnyElement {
            if more {
                div().w(px(30.)).flex().justify_center().opacity(0.45).child(crate::assets::icon(if left { "ui-chevron-left" } else { "ui-chevron-right" }, 22., cx)).into_any_element()
            } else {
                div().w(px(30.)).flex().justify_center().child(div().w(px(3.)).h(px(16.)).bg(gpui_kit::rgb(0x777777))).into_any_element()
            }
        };
        let items: Vec<AnyElement> = list
            .iter()
            .enumerate()
            .filter_map(|(n, t)| {
                let tab = doc.tab(*t)?;
                let on = n == sel;
                let picture: AnyElement = match thumbs.read(cx).picture(*t) {
                    Some(Some(image)) => img(ImageSource::Render(image)).w(px(178.)).h(px(104.)).into_any_element(),
                    Some(None) if thumbs.read(cx).is_assembly(*t, cx) => crate::assets::icon("tab-card-empty-assembly", 80., cx).into_any_element(),
                    _ => div().into_any_element(),
                };
                let label = on.then(|| h_flex().h(px(29.)).gap(px(3.)).items_center().text_size(px(11.)).text_color(pal.text_muted).child(crate::assets::icon(tab_icon(kind_of(&tab.content)), 16., cx)).child(crate::ui::user_text(tab.name.clone())));
                Some(
                    v_flex()
                        .w(px(180.))
                        .flex_none()
                        .items_center()
                        .child(
                            div()
                                .id(("tab-switch", n))
                                .w(px(180.))
                                .h(px(121.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .border_1()
                                .border_color(if on { gpui_kit::rgb(0xaaaaaa) } else { gpui_kit::transparent_white().into() })
                                .when(on, |d| d.bg(crate::theme::themed(0xcedbe5, 0x4b5761)))
                                .child(picture)
                                .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                                    if let Some((_, i)) = this.switcher.as_mut().filter(|_| *h) {
                                        *i = n;
                                        cx.notify();
                                    }
                                }))
                                .on_mouse_up(MouseButton::Left, cx.listener(move |this, _: &MouseUpEvent, window, cx| {
                                    if let Some((_, i)) = &mut this.switcher {
                                        *i = n;
                                    }
                                    this.switcher_commit(window, cx);
                                })),
                        )
                        .child(div().h(px(29.)).children(label))
                        .into_any_element(),
                )
            })
            .collect();
        let card = h_flex()
            .occlude()
            .pt(px(11.))
            .items_start()
            .bg(pal.panel)
            .border_1()
            .border_color(crate::theme::themed(0xcccccc, 0x666666))
            .rounded(px(4.))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.15), offset: point(px(0.), px(1.)), blur_radius: px(4.), spread_radius: px(0.), inset: false }])
            .child(div().h(px(121.)).flex().items_center().child(end(sel > 0, true, cx)))
            .child(h_flex().gap(px(20.)).items_start().children(items))
            .child(div().h(px(121.)).flex().items_center().child(end(sel + 1 < list.len(), false, cx)));
        Some(div().absolute().left_0().right_0().bottom(px(10.)).flex().justify_center().child(card).into_any_element())
    }

    /// Runs a customized shortcut (Preferences › Keyboard shortcuts) in the
    /// active tab.
    fn run_shortcut(&mut self, id: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if id == "general:Create version" {
            crate::history_panel::open_create_version(self.doc.clone(), window, cx);
            return;
        }
        match self.active.and_then(|a| self.views.get(&a)) {
            Some(TabView::PartStudio(v)) => v.update(cx, |v, cx| v.run_shortcut(id, window, cx)),
            Some(TabView::Drawing(v)) => {
                // Typing in a drawing dialog's text box: the key is text, not a
                // shortcut (asb).
                let f = v.read(cx).focus.clone();
                if f.contains_focused(window, cx) && !f.is_focused(window) {
                    cx.propagate();
                    return;
                }
                crate::shortcuts::run_in_drawing(&v.clone(), id, window, cx)
            }
            _ => {}
        }
    }

    /// Onshape's message bubble (top center) for tabs without their own.
    fn render_toast(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (kind, msg) = self.toast.clone()?;
        if matches!(self.active.and_then(|a| self.views.get(&a)), Some(TabView::PartStudio(_) | TabView::Assembly(_) | TabView::Drawing(_)) | None) {
            return None;
        }
        let ws = cx.entity();
        let said = crate::doc_model::notice_text(&self.doc, msg.clone(), cx);
        Some(
            div()
                .absolute()
                .top(px(8.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .map(|d| noshape_app_core::ui_script::mark_message(d, || said))
                .child(crate::doc_model::notice_bubble("ws-toast", kind, msg, &self.doc, move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    ws.update(cx, |this, cx| {
                        this.toast = None;
                        cx.notify();
                    })
                }, cx))
                .into_any_element(),
        )
    }

    /// The navbar `{ }` icon (Onshape: "FeatureScript notices"; red `{!}`
    /// while a Feature Studio's committed source has errors).
    fn render_notices_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        // Live (part-studio/00): shown in every document, `{✓}` when clean.
        // Red for a Feature Studio's compile errors and for errors a custom
        // feature raised while regenerating.
        let errors = crate::fs_notices::document_notices(&self.doc, cx).iter().any(|(_, _, n)| n.iter().any(|n| n.error))
            || noshape_model::custom::run::has_errors(&self.doc.read(cx).session.doc);
        Some(
            h_flex()
                .id("fs-notices-btn")
                .track_focus(&self.notices_focus)
                .focus(|s| s.shadow(vec![BoxShadow { color: gpui_kit::rgb(0x1651b0).into(), offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(1.5), inset: false }]))
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                    if matches!(ev.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        this.toggle_fs_notices(window, cx);
                    }
                }))
                .h(px(32.))
                .px(px(6.))
                .rounded(px(3.))
                .cursor_pointer()
                .hover(|d| d.bg(p().border))
                .when(self.fs_notices.is_some(), |d| d.bg(p().border))
                .tooltip(crate::ui::tooltip("FeatureScript notices"))
                .map(|d| noshape_app_core::ui_script::mark(d, || "button:FeatureScript notices".into()))
                .items_center()
                .child(crate::assets::icon(if errors { "fs-notices-error" } else { "fs-notices" }, 20., cx))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_fs_notices(window, cx)))
                .into_any_element(),
        )
    }

    /// The open view of assembly tab `tab`, if it has one.
    pub fn assembly_view(&self, tab: TabId) -> Option<Entity<crate::assembly_view::AssemblyView>> {
        match self.views.get(&tab) {
            Some(TabView::Assembly(v)) => Some(v.clone()),
            _ => None,
        }
    }

    /// The in-context banner's Create version and update in assembly (live 2464): the next version of this document
    /// ("V<n>"), then the other document's assembly opened on its instance, moved to that version.
    fn version_and_update(&mut self, other: noshape_app_core::assembly_context::OtherAssembly, assembly: TabId, cx: &mut Context<Self>) {
        let n = self.doc.read(cx).session.store.graph().ok().map_or(0, |g| g.versions.len()) + 1;
        let name = format!("V{n}");
        let r = self.doc.update(cx, |d, _| {
            // (Live 2475: its description.)
            let v = d.session.create_version(&name, noshape_ops::in_context::VERSION_FOR_EDIT_IN_CONTEXT)?;
            anyhow::Ok(d.session.store.version(v)?.microversion)
        });
        let mv = match r {
            Ok(mv) => mv,
            Err(e) => {
                self.doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::Error(format!("{e:#}"))));
                return;
            }
        };
        self.doc.update(cx, |d, cx| d.reloaded(cx));
        let here = self.doc.read(cx).session.path.clone();
        if let Err(e) = crate::files::update_linked_instance(other, assembly, here, mv, &name, self.gpu.clone(), cx) {
            log::warn!("Create version and update in assembly: {e}");
        }
    }

    /// Edit in context from another document's assembly (`files::open_in_context`): the context snapshot stored in
    /// Part Studio `studio` (not undoable, as for this document's assemblies), the studio shown with it.
    pub fn enter_context_from_other(&mut self, studio: TabId, ctx: noshape_app_core::assembly_context::AssemblyContext, install: (u32, Option<noshape_model::in_context::AssemblyContext>), window: &mut Window, cx: &mut Context<Self>) {
        let (id, new) = install;
        self.doc.update(cx, |d, cx| {
            d.edit_document_not_undoable("Edit in context", cx, |doc| {
                if let Some(noshape_model::TabContent::PartStudio(ps)) = doc.tab_mut(studio).map(|t| &mut t.content) {
                    noshape_ops::in_context::install(ps, id, new);
                }
            });
            cx.emit(crate::doc_model::DocEvent::TabChanged(studio));
        });
        self.activate(studio, window, cx);
        if let Some(TabView::PartStudio(v)) = self.views.get(&studio) {
            v.update(cx, |v, cx| v.set_assembly_context(Some(ctx), cx));
        }
    }

    fn toggle_fs_notices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fs_notices.take().is_some() {
            cx.notify();
            return;
        }
        let doc = self.doc.clone();
        let panel = cx.new(|cx| crate::fs_notices::FsNoticesPanel::new(doc, window, cx));
        let sub = cx.subscribe_in(&panel, window, |this: &mut Self, _, ev: &crate::fs_notices::FsNoticesEvent, window, cx| match ev {
            crate::fs_notices::FsNoticesEvent::Close => {
                this.fs_notices = None;
                cx.notify();
            }
            crate::fs_notices::FsNoticesEvent::Goto(tab, line, column) => {
                let (tab, line, column) = (*tab, *line, *column);
                this.activate(tab, window, cx);
                if let Some(TabView::FeatureStudio(v)) = this.views.get(&tab) {
                    v.update(cx, |v, cx| v.goto_line(line, column, window, cx));
                }
            }
            // The Part Studio a notice's feature ran in (live 2467: the panel stays open).
            crate::fs_notices::FsNoticesEvent::OpenTab(tab) => this.activate(*tab, window, cx),
            crate::fs_notices::FsNoticesEvent::OpenStd(module, line, column) => {
                if let Err(e) = crate::std_source::open(module, *line, *column, this.gpu.clone(), cx) {
                    log::warn!("Could not open the standard library: {e}");
                }
            }
        });
        self.fs_notices = Some((panel, sub));
        cx.notify();
    }

    /// "+" › Create folder: a new folder tab, its name field active.
    fn create_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (after, open) = (self.active, self.open_folder);
        let mut new_id = None;
        self.doc.update(cx, |doc, cx| {
            doc.edit_document("Create folder", cx, |d| {
                let parent = open.or_else(|| after.and_then(|a| d.parent_of(a)));
                let name = d.next_folder_name(parent);
                let id = d.add_folder(&name, None);
                if let Some(a) = after.filter(|a| d.parent_of(*a) == parent) {
                    place_after(d, id, Some(a));
                } else {
                    d.set_parent(id, parent);
                }
                new_id = Some(id);
            });
        });
        if let Some(id) = new_id {
            self.start_tab_rename(id, window, cx);
        }
    }

    /// A tab dropped on a folder (or on "All tabs": the root): the bar under it takes the same drop, whose gap must
    /// not move the tab back out.
    fn drop_into(&mut self, id: TabId, folder: Option<TabId>, cx: &mut Context<Self>) {
        self.tab_gap = None;
        cx.stop_propagation();
        self.move_into(id, folder, cx);
    }

    /// `id` into `folder` (None: the root).
    fn move_into(&mut self, id: TabId, folder: Option<TabId>, cx: &mut Context<Self>) {
        if Some(id) == folder {
            return;
        }
        self.doc.update(cx, |doc, cx| {
            if doc.session.doc.parent_of(id) != folder {
                doc.edit_document("Move to folder", cx, |d| {
                    d.set_parent(id, folder);
                });
            }
        });
        cx.notify();
    }

    /// Drop on a tab: in front of it, in its folder.
    /// Put tab `id` before `before` (None: the end of the open folder's
    /// tabs). Live: a reorder isn't undoable.
    fn move_tab(&mut self, id: TabId, before: Option<TabId>, cx: &mut Context<Self>) {
        if Some(id) == before {
            return;
        }
        let parent = before.map_or(self.open_folder, |b| self.doc.read(cx).session.doc.parent_of(b));
        self.doc.update(cx, |doc, cx| {
            doc.edit_document_not_undoable("Reorder tabs", cx, |d| {
                if d.set_parent(id, parent) {
                    d.move_in_order(id, before);
                }
            });
        });
        cx.notify();
    }

    /// A tab dropped on the bar: into the gap, if one is open (live:
    /// dropping past the last tab does nothing).
    fn drop_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(TabGap { dragged, before: Some(before), .. }) = self.tab_gap.take() {
            self.move_tab(dragged, before, cx);
        }
        cx.notify();
    }

    /// A folder on the tab bar: a click opens it (a double-click too: live doc-folder-dblclick-rename, no rename);
    /// right-click: Rename, Unpack, Move to document… (live qa5 TABS3).
    fn folder_tab(&self, id: TabId, name: String, cx: &mut Context<Self>) -> Stateful<Div> {
        let pal = p();
        tab_chip(SharedString::from(format!("folder-{id}")), false)
            .map(|d| noshape_app_core::ui_script::mark(d, || format!("doc_tab:{name}")))
            .min_w(px(60.))
            .max_w(px(200.))
            .px(px(10.))
            .child(folder_glyph(20., pal.text_muted))
            .child(self.folder_label(id, &name))
            .drag_threshold(TAB_DRAG_PX)
            .on_drag(TabDrag { id, name: name.clone(), icon: None, width: px(0.) }, start_tab_drag(cx.entity().downgrade()))
            .drag_over::<TabDrag>(|st, _, _, _| st.bg(gpui_kit::rgb(0xdef1fb)))
            .on_drop(cx.listener(move |this, drag: &TabDrag, _, cx| this.drop_into(drag.id, Some(id), cx)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.open_folder = Some(id);
                cx.notify();
            }))
            .os_context_menu(self.folder_menu(id, cx))
    }

    /// The open folder's crumb (live: padding 2 12 4 0, the folder icon and
    /// the name in at most 100 px, white / `#333` in dark; `prev`: the crumb
    /// before it, whose point it draws over its own notch).
    fn folder_crumb(&self, id: TabId, name: String, prev: Rgba, cx: &mut Context<Self>) -> Stateful<Div> {
        let pal = p();
        let mark = format!("crumb:{name}");
        breadcrumb(SharedString::from(format!("crumb-{id}")), pal.panel, Some(prev), true)
            .map(|d| noshape_app_core::ui_script::mark(d, || mark))
            .pr(px(12.))
            .child(h_flex().max_w(px(100.)).overflow_hidden().gap(px(5.)).child(div().flex_none().child(folder_glyph(20., pal.text_muted))).child(self.folder_label(id, &name)))
            .drag_over::<TabDrag>(|st, _, _, _| st.bg(gpui_kit::rgb(0xdef1fb)))
            .on_drop(cx.listener(move |this, drag: &TabDrag, _, cx| this.drop_into(drag.id, Some(id), cx)))
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if e.click_count() >= 2 {
                    this.start_tab_rename(id, window, cx);
                }
            }))
            .os_context_menu(self.folder_menu(id, cx))
    }

    /// A folder's name, or its rename field.
    fn folder_label(&self, id: TabId, name: &str) -> AnyElement {
        let pal = p();
        match self.renaming_tab.as_ref().filter(|r| r.0 == id) {
            Some((_, st, _)) => div().w(px(150.)).child(gpui_kit::component::input::Input::new(st).h(px(22.)).text_size(px(14.)).px(px(3.)).text_color(pal.text)).into_any_element(),
            None => div().min_w_0().truncate().child(crate::ui::user_text(name.to_string())).into_any_element(),
        }
    }

    /// A folder's right-click menu: Rename, (in a folder) Move to parent
    /// folder, Unpack | Move to document… (live TABS3 and behaviours
    /// "tab-bar folder breadcrumbs").
    fn folder_menu(&self, id: TabId, cx: &mut Context<Self>) -> impl Fn(crate::os_menu::OsMenu, &mut Window, &mut Context<crate::os_menu::OsMenu>) -> crate::os_menu::OsMenu + 'static {
        let view = cx.entity();
        let nested = self.doc.read(cx).session.doc.parent_of(id).is_some();
        move |menu, _, _| {
            use crate::os_menu::OsMenuItem as I;
            let (v1, v2, v3, v4) = (view.clone(), view.clone(), view.clone(), view.clone());
            let menu = menu.item(I::new("Rename").on_click(move |_, w, cx| v1.update(cx, |t, cx| t.start_tab_rename(id, w, cx))));
            let menu = if nested { menu.item(I::new("Move to parent folder").on_click(move |_, _, cx| v2.update(cx, |t, cx| t.move_to_parent(id, cx)))) } else { menu };
            menu.item(I::new("Unpack").on_click(move |_, _, cx| {
                v3.update(cx, |t, cx| {
                    if t.open_folder == Some(id) {
                        t.open_folder = t.doc.read(cx).session.doc.parent_of(id);
                    }
                    t.doc.update(cx, |doc, cx| doc.edit_document("Unpack folder", cx, |d| d.unpack_folder(id)));
                    cx.notify();
                })
            }))
            .separator()
            .item(I::new("Move to document…").on_click(move |_, w, cx| {
                let doc = v4.read(cx).doc.clone();
                crate::doc_dialogs::move_to_document(doc, vec![id], w, cx)
            }))
        }
    }

    /// Tab › Move to parent folder: out of its folder into the one around it.
    fn move_to_parent(&mut self, id: TabId, cx: &mut Context<Self>) {
        let d = &self.doc.read(cx).session.doc;
        let Some(parent) = d.parent_of(id) else { return };
        let to = d.parent_of(parent);
        if self.open_folder == Some(id) {
            self.open_folder = to;
        }
        self.move_into(id, to, cx);
    }

    /// Open (or focus) the Search Feature Studios pane.
    fn open_fs_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.fs_search.as_ref().map(|(p, _)| p.clone()) {
            let find = pane.read(cx).find_field();
            find.update(cx, |s, cx| s.focus(window, cx));
            return;
        }
        let doc = self.doc.clone();
        let pane = cx.new(|cx| crate::fs_search::FsSearchPane::new(doc, window, cx));
        let sub = cx.subscribe_in(&pane, window, |this: &mut Self, _, ev: &crate::fs_search::FsSearchEvent, window, cx| match ev {
            crate::fs_search::FsSearchEvent::Close => {
                this.fs_search = None;
                cx.notify();
            }
            crate::fs_search::FsSearchEvent::Goto(tab, line, column) => {
                let (tab, line, column) = (*tab, *line, *column);
                this.activate(tab, window, cx);
                if let Some(TabView::FeatureStudio(v)) = this.views.get(&tab) {
                    v.update(cx, |v, cx| v.goto_line(line, column, window, cx));
                }
            }
        });
        self.fs_search = Some((pane, sub));
        cx.notify();
    }
}

static REQUESTED_EXPORT: parking_lot::Mutex<Option<(TabId, Vec<noshape_model::BodyId>)>> = parking_lot::Mutex::new(None);

/// Open the Export modal of a tab's item (parts of a Part Studio, or the
/// tab), e.g. from a release candidate row.
pub fn request_export(tab: TabId, parts: Vec<noshape_model::BodyId>, window: &mut Window, cx: &mut App) {
    // From a modal (release dialog, revision history) an action wouldn't
    // reach the workspace: call the window's workspace directly.
    let ws = crate::os_menu::content_view(window, cx).and_then(|v| v.downcast::<Workspace>().ok());
    match ws {
        Some(ws) => ws.update(cx, |w, cx| w.export_item(tab, parts, window, cx)),
        None => {
            *REQUESTED_EXPORT.lock() = Some((tab, parts));
            window.dispatch_action(Box::new(ExportRequestedItem), cx);
        }
    }
}

/// The workspace's + › Import… (e.g. the drawing Insert view picker's
/// "Import..." row).
pub fn request_import(window: &mut Window, cx: &mut App) {
    if let Some(ws) = crate::os_menu::content_view(window, cx).and_then(|v| v.downcast::<Workspace>().ok()) {
        ws.update(cx, |w, cx| w.import(window, cx));
    }
}

/// Ask the workspace to switch to `tab` (app-core, so the studio crates can ask too).
pub use crate::actions::request_activate_tab;

impl Workspace {
    /// Export one tab's item: its parts (Part Studio) or the tab.
    pub(crate) fn export_item(&mut self, t: TabId, parts: Vec<noshape_model::BodyId>, window: &mut Window, cx: &mut Context<Self>) {
        self.activate(t, window, cx);
        let studio = match self.views.get(&t) {
            Some(TabView::PartStudio(v)) if !parts.is_empty() => Some(v.clone()),
            _ => None,
        };
        match studio {
            Some(v) => v.update(cx, |v, cx| v.open_part_export(parts, window, cx)),
            None => self.open_export(t, window, cx),
        }
    }

    /// Show the Create Drawing dialog (from "+" › Create Drawing or a
    /// "Create drawing of …" menu entry).
    fn open_create_drawing(&mut self, req: crate::drawing_create::CreateRequest, window: &mut Window, cx: &mut Context<Self>) {
        use crate::drawing_create::{CreateDrawingDialog, CreateDrawingEvent};
        let doc = &self.doc.read(cx).session.doc;
        // Onshape: "<part> Drawing 1" for a part or assembly, else "Drawing 1".
        let default_name = doc.next_tab_name(&req.name.as_ref().map(|n| format!("{n} Drawing")).unwrap_or_else(|| "Drawing".into()));
        let metric = !matches!(doc.settings.units.length, noshape_units::LengthUnit::Inch | noshape_units::LengthUnit::Foot | noshape_units::LengthUnit::Yard);
        // The document's DWT tabs: Create drawing › Created by me (live request 1627).
        let document = self.doc.read(cx).name();
        let dwts = doc
            .tabs
            .iter()
            .filter_map(|t| match &t.content {
                noshape_model::document::TabContent::File(f) if f.kind() == noshape_model::file_tab::FileKind::Template => Some(crate::drawing_create::DwtTab { name: t.name.clone(), document: document.clone(), owner: String::new(), data_base64: f.data_base64.clone() }),
                _ => None,
            })
            .collect();
        let external = req.external.clone();
        let dialog = cx.new(|cx| CreateDrawingDialog::new(req, default_name, metric, dwts, window, cx));
        // The other documents' DWTs (live 1627: "Created by me" lists the
        // DWTs exported from every document), read in the background.
        let here = self.doc.read(cx).session.path.clone();
        let dlg = dialog.downgrade();
        cx.spawn(async move |_, cx| {
            let (more, own) = cx
                .background_spawn(async move {
                    let docs = crate::start_library::Library::default().scan(&crate::start_library::ScanInput::current()).docs;
                    let own = docs.iter().find(|d| d.path == here).map(|d| d.owner.clone());
                    let mut more = Vec::new();
                    for d in docs.iter().filter(|d| !d.broken && !d.trashed && d.path != here) {
                        let Some(snap) = (|| {
                            let store = noshape_store::Document::open_read_only(&d.path).ok()?;
                            let head = store.head(store.default_branch().ok()?).ok()?;
                            crate::derived::external_snapshot(&d.path, head).ok()
                        })() else {
                            continue;
                        };
                        for t in snap.ordered_tabs() {
                            if let noshape_model::document::TabContent::File(f) = &t.content
                                && f.kind() == noshape_model::file_tab::FileKind::Template
                            {
                                more.push(crate::drawing_create::DwtTab { name: t.name.clone(), document: d.name.clone(), owner: d.owner.clone(), data_base64: f.data_base64.clone() });
                            }
                        }
                    }
                    (more, own)
                })
                .await;
            let _ = dlg.update(cx, |d, cx| d.add_dwts(more, own, cx));
        })
        .detach();
        let sub = cx.subscribe_in(&dialog, window, move |this: &mut Self, _, ev: &CreateDrawingEvent, window, cx| {
            if let CreateDrawingEvent::Create { name, template, source, suggested, dwt } = ev {
                this.create_drawing_tab(name.clone(), (*template, dwt.as_deref().cloned()), (source.clone(), external.clone()), *suggested, window, cx);
            }
            this.create_drawing = None;
            cx.notify();
        });
        let fh = dialog.read(cx).focus.clone();
        window.focus(&fh, cx);
        self.create_drawing = Some((dialog, sub));
        cx.notify();
    }

    /// Create the Drawing tab: suggested views are laid out from the source's
    /// bounds (computed in the background); with No views the new drawing
    /// starts placing a view of the source at the cursor.
    /// `template`: a built-in template, and the document's DWT tab chosen
    /// under Created by me (its formats and properties go over it). `sources`: this document's, or another document's
    /// version (Insert view on it).
    fn create_drawing_tab(&mut self, name: String, template: (noshape_model::drawing::templates::DrawingTemplate, Option<crate::drawing_create::DwtTab>), sources: (Option<noshape_model::drawing::create::DrawingSource>, Option<crate::drawing_create::ExternalSource>), suggested: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (template, dwt) = template;
        let (source, external) = sources;
        let doc = self.doc.read(cx).session.doc.clone();
        // A Part Studio source its tab shows regenerated: laid out (and drawn) from that.
        let shown = source.as_ref().filter(|_| suggested).and_then(|s| Some((noshape_model::drawing::views::SourceKey { config: s.config.clone(), ..noshape_model::drawing::views::SourceKey::studio(s.tab) }, crate::part_studio::committed_context(&self.doc, s.tab, &s.config, cx)?)));
        cx.spawn_in(window, async move |this, cx| {
            let src = source.clone();
            let drawing = cx
                .background_spawn(async move {
                    if let Some((key, context)) = shown {
                        noshape_model::drawing::views::studio_source(&doc, &key, context);
                    }
                    let bounds = if suggested { src.as_ref().and_then(|s| noshape_model::drawing::create::source_bounds(&doc, s)) } else { None };
                    // Sketch sources face the sketch plane.
                    let frame = src.as_ref().and_then(|s| noshape_model::drawing::create::source_frame(&doc, s));
                    let mut d = noshape_model::drawing::create::create_drawing(&template, src.as_ref(), bounds, suggested, frame);
                    noshape_model::drawing::create::assembly_drawing_defaults(&doc, &mut d);
                    // A DWT template: its sheet formats and drawing properties
                    // (live request 1627), the views as laid out.
                    if let Some(t) = &dwt
                        && let Some(bytes) = noshape_model::sketch_feature::base64_decode(&t.data_base64)
                        && let Err(e) = noshape_model::drawing::dwt::apply_dwt_template(&mut d, &t.name, &bytes)
                    {
                        log::warn!("DWT template {}: {e}", t.name);
                    }
                    // DRAWN: who created it, saved with the drawing (qa5 D13).
                    d.drawn_by = Some(noshape_store::local_user_name()).filter(|n| !n.is_empty());
                    // Suggested views show the Part Studio's MBD datums (live behaviours "MBD datums in drawings").
                    if suggested {
                        noshape_model::drawing::mbd::trim_suggested_views(&mut d, &doc);
                        noshape_model::drawing::mbd::place_model_datums(&mut d, &doc);
                    }
                    d
                })
                .await;
            let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                let mut new_id = None;
                this.doc.update(cx, |doc, cx| {
                    // The views are made from the document as it is now: a
                    // later change to a source puts the drawing out of date,
                    // edited or not.
                    let mut drawing = drawing;
                    drawing.sources_at = doc.session.head().ok().map(|m| m.to_string());
                    doc.edit_document(&format!("Insert tab : {name}"), cx, |d| {
                        new_id = Some(docedit::insert_drawing_tab(d, &name, drawing, source.as_ref().map(|s| s.tab)));
                    });
                });
                let Some(id) = new_id else { return };
                this.activate(id, window, cx);
                // Another document's version: Insert view on it.
                if let (Some(e), Some(TabView::Drawing(v))) = (&external, this.views.get(&id)) {
                    v.update(cx, |v, cx| v.choose_external(e.key.clone(), e.tab, e.part.clone(), e.label.clone(), cx));
                    return;
                }
                if let (false, Some(TabView::Drawing(v))) = (suggested, this.views.get(&id)) {
                    match &source {
                        Some(s) => {
                            let tool = crate::drawing_view::Tool::InsertView { source: s.tab, orientation: noshape_model::drawing::ViewOrientation::Front, part: s.part.clone() };
                            let sketch = s.sketch;
                            // An assembly's last applied named position and display state (live drawing/1106).
                            let prefill = match this.views.get(&s.tab) {
                                Some(TabView::Assembly(a)) => Some(a.read(cx).drawing_prefill(cx)),
                                _ => None,
                            };
                            v.update(cx, |v, cx| {
                                if let Some((pos, ds)) = prefill {
                                    v.insert.asm_position = pos;
                                    v.insert.display_state = ds;
                                }
                                v.set_tool(tool, cx);
                                v.insert_sketch(sketch, cx);
                            });
                        }
                        // From "+" › Create Drawing…: choose what to insert first.
                        None => v.update(cx, |v, cx| v.open_insert_chooser(cx)),
                    }
                }
            });
        })
        .detach();
    }
}

impl Workspace {
    fn toggle_tab_manager(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::tab_manager::{TabManager as Panel, TabManagerEvent};
        self.reveal_active_tab = true;
        if self.tab_manager.take().is_some() {
            if let Some(a) = self.active {
                self.activate(a, window, cx);
            }
        } else {
            let (doc, active) = (self.doc.clone(), self.active);
            let me = cx.entity().downgrade();
            let tm = cx.new(|cx| Panel::new(doc, active, me, window, cx));
            let sub = cx.subscribe_in(&tm, window, |this: &mut Self, _, ev: &TabManagerEvent, window, cx| match ev {
                TabManagerEvent::Open(t) => this.activate(*t, window, cx),
                TabManagerEvent::Close => this.toggle_tab_manager(window, cx),
                TabManagerEvent::Delete(tabs) => {
                    for t in tabs.clone() {
                        this.delete_tab(t, window, cx);
                    }
                }
            });
            self.tab_manager = Some((tm, sub));
        }
        cx.notify();
    }
}


/// Onshape inserts a new tab right after the active one (behaviours.md, tab bar).
pub(crate) use noshape_ops::docedit::place_after;

/// A tab or folder dragged on the tab bar (also its drag preview).
#[derive(Clone)]
struct TabDrag {
    id: TabId,
    name: String,
    /// Tab icon; None: a folder.
    icon: Option<&'static str>,
    /// The tab's width when the drag started (its gap in the bar).
    width: Pixels,
}

impl Render for TabDrag {
    /// Nothing under the pointer: the bar draws the ghost, along the bar
    /// only (`Workspace::render_tab_bar`).
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// A tab drag along the bar: the dragged tab, where its gap is (None: no
/// gap, Some(None): at the end), the pointer x and where the tab was
/// grabbed, and the ghost's icon and label.
struct TabGap {
    dragged: TabId,
    before: Option<Option<TabId>>,
    x: Pixels,
    grab: Pixels,
    icon: Option<&'static str>,
    name: String,
    /// The dragged tab's width: the gap it leaves.
    width: Pixels,
    /// The strip scrolling under a drag held near one of its ends (px a second, + to the right), and the timer doing it.
    scroll: f32,
    autoscroll: Option<Task<()>>,
    /// When the strip last scrolled under the drag.
    scrolled_at: Option<std::time::Instant>,
}

/// How far a tab moves before it drags (live2 feel/documents.json tabBar.dragStartPx: 5 px is a click, 6 a drag).
const TAB_DRAG_PX: f32 = 5.;

/// The strip's scroll speed (px a second) by the dragging pointer's distance from its end (live2 tabBar.autoScroll,
/// both ends' mean: 18 / 55 / 70 / 107 / 114 px/s at 45 / 30 / 20 / 10 / 0 px; none at 60).
const TAB_AUTOSCROLL: [(f32, f32); 6] = [(52., 0.), (45., 18.), (30., 55.), (20., 70.), (10., 107.), (0., 114.)];

/// The scroll speed of a tab drag at `x` over the strip spanning `left`..`right`: negative near its left end.
fn autoscroll_speed(x: Pixels, left: Pixels, right: Pixels) -> f32 {
    let (to_left, to_right) = (f32::from(x - left), f32::from(right - x));
    let speed = |d: f32| {
        let d = d.max(0.);
        TAB_AUTOSCROLL.windows(2).find(|w| d <= w[0].0 && d >= w[1].0).map_or(0., |w| w[1].1 + (w[0].1 - w[1].1) * (d - w[1].0) / (w[0].0 - w[1].0))
    };
    if to_left < to_right { -speed(to_left) } else { speed(to_right) }
}

/// The strip's scroll offset (0 to -`max`) `secs` of scrolling at `speed` later.
fn autoscroll_offset(x: Pixels, max: Pixels, speed: f32, secs: f32) -> Pixels {
    (x - px(speed * secs)).clamp(-max, px(0.))
}

/// The drag start of a tab or folder chip: records the drag on the
/// workspace (the gap starts where the tab was).
fn start_tab_drag(ws: WeakEntity<Workspace>) -> impl Fn(&TabDrag, Point<Pixels>, &mut Window, &mut App) -> Entity<TabDrag> {
    move |d, offset, window, cx| {
        let x = window.mouse_position().x;
        let _ = ws.update(cx, |w, cx| {
            let order = w.doc.read(cx).session.doc.children(w.open_folder);
            let next = order.iter().position(|t| *t == d.id).and_then(|i| order.get(i + 1)).copied();
            w.tab_gap = Some(TabGap { dragged: d.id, before: Some(next), x, grab: offset.x, icon: d.icon, name: d.name.clone(), width: d.width, scroll: 0., autoscroll: None, scrolled_at: None });
            cx.notify();
        });
        cx.new(|_| d.clone())
    }
}

/// A tab-bar cell hanging from the bar's top edge (Onshape §3.5).
fn tab_chip(id: impl Into<ElementId>, active: bool) -> Stateful<Div> {
    let pal = p();
    h_flex()
        .id(id)
        .relative()
        .flex_none()
        .h(px(if active { 28. } else { 27. }))
        .gap(px(3.))
        .rounded_b(px(5.))
        .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.12), offset: point(px(0.), px(1.)), blur_radius: px(2.), spread_radius: px(0.), inset: false }])
        .text_size(px(14.))
        .cursor_pointer()
        .bg(if active { pal.panel } else { pal.tab_inactive })
        .text_color(if active { pal.text } else { pal.text_muted })
}

/// A tab-bar breadcrumb (live `.os-tab-bar-breadcrumb`): 27 high in
/// `bg`, a 2 px shadow down-left, an 8 px point on the right (CSS border
/// triangle). After the first (`prev`: the crumb before it) its 17 px flag
/// reaches 16 px back with the bar's colour cut in as a notch, and the
/// previous crumb's point is drawn over it 2 px left of the notch. Crumbs
/// are 18 px apart, the `last` none.
fn breadcrumb(id: impl Into<ElementId>, bg: Rgba, prev: Option<Rgba>, last: bool) -> Stateful<Div> {
    let pal = p();
    let shadow = || vec![BoxShadow { color: hsla(0., 0., 0., 0.247), offset: point(px(-1.), px(1.)), blur_radius: px(2.), spread_radius: px(-1.), inset: false }];
    let point_at = |color: Rgba| {
        canvas(|_, _, _| {}, move |b, _, window, _| {
            let (l, t) = (b.left(), b.top());
            let mut pb = PathBuilder::fill();
            pb.move_to(point(l, t));
            pb.line_to(point(l + px(8.), t + px(13.)));
            pb.line_to(point(l + px(8.), t + px(14.)));
            pb.line_to(point(l, t + px(27.)));
            pb.close();
            if let Ok(path) = pb.build() {
                window.paint_path(path, color);
            }
        })
        .absolute()
        .top_0()
        .w(px(8.))
        .h(px(27.))
    };
    h_flex()
        .id(id)
        .relative()
        .flex_none()
        .h(px(27.))
        .pt(px(2.))
        .pb(px(4.))
        .when(!last, |d| d.mr(px(18.)))
        .bg(bg)
        .shadow(shadow())
        .text_size(px(14.))
        .text_color(pal.text_muted)
        .cursor_pointer()
        .when_some(prev, |d, prev| {
            d.child(div().absolute().top_0().left(px(-16.)).w(px(17.)).h(px(27.)).bg(bg).shadow(shadow()).child(point_at(pal.tab_bar).left_0()))
                .child(point_at(prev).left(px(-18.)))
        })
        .child(point_at(bg).right(px(-8.)))
}

/// The folder icon: an outline, filled in dark (live tab bar).
fn folder_glyph(size: f32, color: impl Into<Hsla>) -> gpui_kit::component::Icon {
    os::glyph(if noshape_scene::palette::is_dark() { os::GLYPH_FOLDER_FILLED } else { os::GLYPH_FOLDER }, size, color)
}

// Importing is shared with the agent API (crates/ops/src/import.rs).
pub(crate) use noshape_ops::import::{IMPORT_EXTS, import_names};

/// Tab right-click menus (live menus/rmb-tab-*): they open upward, the
/// bottom on the tab bar's top edge and the left edge at the tab's.
trait TabContextMenuExt: InteractiveElement + Sized {
    /// `tab`: the tab's bounds from its last paint.
    fn tab_context_menu(self, tab: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>, f: impl Fn(crate::os_menu::OsMenu, &mut Window, &mut Context<crate::os_menu::OsMenu>) -> crate::os_menu::OsMenu + 'static) -> Self {
        let f = std::rc::Rc::new(f);
        self.on_mouse_down(MouseButton::Right, move |e, window, cx| {
            cx.stop_propagation();
            let f = f.clone();
            let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| f(m, window, cx));
            if !menu.read(cx).is_empty() {
                let h = window.viewport_size().height;
                let b = tab.get();
                let x = if b.size.width > px(0.) { b.left() } else { e.position.x };
                crate::os_menu::open_anchored(point(x, h - px(crate::ui::TAB_BAR_H)), Anchor::BottomLeft, menu, window, cx);
            }
        })
    }
}

impl<E: InteractiveElement + Sized> TabContextMenuExt for E {}

/// The Tabs panel's Create… submenu (live 2331): Folder, Import…, Part Studio, Assembly, Variable Studio, Drawing…,
/// Feature Studio, Material Library, as the + menu makes them.
pub(crate) fn panel_create_menu(view: Entity<Workspace>, m: crate::os_menu::OsMenu) -> crate::os_menu::OsMenu {
    use crate::os_menu::OsMenuItem as I;
    let tab = |m: crate::os_menu::OsMenu, label: &'static str, icon: &'static str, kind: &'static str| {
        let v = view.clone();
        m.item(I::new(label).icon_name(icon).on_click(move |_, w, cx| v.update(cx, |t, cx| t.add_tab(kind, w, cx))))
    };
    let (v1, v2, v3) = (view.clone(), view.clone(), view.clone());
    let m = m
        .item(I::new("Folder").icon_name("folder").on_click(move |_, w, cx| v1.update(cx, |t, cx| t.create_folder(w, cx))))
        .item(I::new("Import…").icon_name("import-file").on_click(move |_, w, cx| v2.update(cx, |t, cx| t.import(w, cx))));
    let m = tab(m, "Part Studio", "part-studio", "partstudio");
    let m = tab(m, "Assembly", "assembly", "assembly");
    let m = tab(m, "Variable Studio", "variable-studio", "variables");
    let m = m.item(I::new("Drawing…").icon_name("drawing").on_click(move |_, w, cx| v3.update(cx, |t, cx| t.open_create_drawing(crate::drawing_create::CreateRequest { external: None, source: None, name: None }, w, cx))));
    let m = tab(m, "Feature Studio", "feature-studio", "featurestudio");
    tab(m, "Material Library", "material-library", "materiallibrary")
}

/// A tab's rows in the Tabs panel's menu (live doc-tab-manager-row-menu, 2331): the tab bar's menu without
/// Properties…, Export…, Release… and the drawing / assembly extras, the panel's Add selection to folder before
/// Duplicate. `delete`, `add_to_folder` and `move_to` are the panel's (they act on its selection).
pub(crate) fn panel_tab_menu(view: Entity<Workspace>, id: TabId, m: crate::os_menu::OsMenu, delete: crate::os_menu::OsMenuItem, add_to_folder: crate::os_menu::OsMenuItem, move_to: crate::os_menu::OsMenuItem, cx: &App) -> crate::os_menu::OsMenu {
    use crate::os_menu::OsMenuItem as I;
    let kind = view.read(cx).doc.read(cx).session.doc.tab(id).map(|t| kind_of(&t.content)).unwrap_or("");
    let (v1, v2, v3, v4, v5, v6, v7) = (view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view);
    let m = m
        .item(delete)
        .item(I::new("Open in new browser tab").on_click(move |_, _, cx| {
            let (doc, gpu) = (v1.read(cx).doc.clone(), v1.read(cx).gpu.clone());
            if let Err(e) = crate::files::open_tab_window(doc, id, gpu, cx) {
                log::warn!("Could not open a window: {e}");
            }
        }))
        .item(I::new("Rename").on_click(move |_, w, cx| v2.update(cx, |t, cx| t.start_tab_rename(id, w, cx))));
    let m = if kind == "partstudio" {
        m.item(I::new("Show code").on_click(move |_, w, cx| v3.update(cx, |t, cx| t.show_code(id, w, cx))))
    } else {
        m
    };
    let m = m
        .item(add_to_folder)
        .item(I::new("Duplicate").on_click(move |_, _, cx| v4.update(cx, |t, cx| t.duplicate_tab(id, cx))))
        .item(I::new("Copy to clipboard").on_click(move |_, _, cx| {
            let t = v5.read(cx).doc.read(cx).session.doc.tab(id).cloned();
            cx.set_global(crate::doc_dialogs::CopiedTab(t));
        }));
    let m = if matches!(kind, "partstudio" | "assembly" | "drawing") {
        m.item(I::new("Select as document thumbnail").on_click(move |_, _, cx| v6.update(cx, |t, cx| t.select_thumbnail(id, cx))))
    } else {
        m
    };
    let m = m.item(move_to);
    // Export… last on a Drawing's row, as on the tab bar (live2 11-13 05:05: not on Part Studios or Assemblies).
    if kind == "drawing" {
        return m.item(I::new("Export…").on_click(move |_, w, cx| v7.update(cx, |t, cx| t.open_export(id, w, cx))));
    }
    m
}


/// ☰ › Close document: the window closes and the documents page shows (live: the tab goes to the documents page).
fn close_document(_: &ClickEvent, window: &mut Window, cx: &mut App) {
    window.remove_window();
    crate::files::show_start_window(cx);
}

#[cfg(test)]
mod tab_autoscroll_tests {
    use super::{autoscroll_offset, autoscroll_speed, px};

    /// Lead 11-13 (interaction feel): a tab dragged within 30 px of an end of the strip scrolls it that way, 360 px a
    /// second whatever the frame rate, and stops at the strip's ends.
    #[test]
    fn a_tab_dragged_to_an_end_of_the_strip_scrolls_it() {
        // Live2 (feel/documents.json tabBar.autoScroll): none at 60 px from an end, then faster toward it.
        let (l, r) = (px(100.), px(900.));
        let at = |x: f32| autoscroll_speed(px(x), l, r);
        assert_eq!([at(160.), at(500.), at(840.)], [0., 0., 0.], "60 px from an end and further: still");
        assert_eq!([at(855.), at(870.), at(880.), at(890.), at(900.)], [18., 55., 70., 107., 114.], "toward the right end");
        assert_eq!(at(130.), -55., "toward the left end, leftwards");
        let max = px(400.);
        // Toward the right end: the offset goes negative with the time held, down to -max.
        assert_eq!(autoscroll_offset(px(0.), max, 100., 0.5), px(-50.), "half a second at 100 px/s");
        assert_eq!(autoscroll_offset(px(-50.), max, 100., 0.1), px(-60.), "at a slow frame rate the same speed");
        assert_eq!(autoscroll_offset(px(-390.), max, 100., 1.0), -max, "held at the end");
        assert_eq!(autoscroll_offset(px(0.), max, -100., 0.5), px(0.), "held at the start");
        assert_eq!(autoscroll_offset(px(-100.), max, 0., 0.5), px(-100.));
    }

}

#[cfg(test)]
mod switcher_tests {
    use super::{TabId, switcher_next};

    /// Live (spec "Ctrl+Space quick tab switcher"): the first Space selects the previous tab, each further one the
    /// next, wrapping to the current tab; closed tabs drop out; one tab selects itself.
    #[test]
    fn ctrl_space_walks_the_recent_tabs() {
        let (a, b, c, gone) = (TabId(1), TabId(2), TabId(3), TabId(9));
        let tabs = [a, b, c];
        let s = switcher_next(None, Some(c), &[gone, a, b], &tabs);
        assert_eq!(s, Some((vec![c, a, b], 1)));
        let s = switcher_next(s, Some(c), &[], &tabs);
        assert_eq!(s.as_ref().map(|s| s.1), Some(2));
        assert_eq!(switcher_next(s, Some(c), &[], &tabs).map(|s| s.1), Some(0));
        assert_eq!(switcher_next(None, Some(a), &[], &tabs), Some((vec![a], 0)));
        assert_eq!(switcher_next(None, None, &[], &tabs), None);
    }
}
