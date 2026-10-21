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
    history: Option<Entity<HistoryPanel>>,
    /// The Release drafts panel (left rail).
    release_drafts: Option<Entity<crate::release_dialog::ReleaseDraftsPanel>>,
    /// Previously active tabs, most recent first (Ctrl+Space).
    recent: Vec<TabId>,
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
    /// ☰ › Workspace units… panel.
    units_panel: Option<(Entity<crate::doc_dialogs::WorkspaceUnits>, Subscription)>,
    /// The tab-bar folder being shown (None: all tabs at the root).
    open_folder: Option<TabId>,
    /// Onshape's "FeatureScript notices" panel, under the tab bar.
    fs_notices: Option<(Entity<crate::fs_notices::FsNoticesPanel>, Subscription)>,
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
            DocEvent::TabsChanged | DocEvent::Reloaded => {
                // Register custom feature schemas of the (new) Feature Studios.
                let _ = this.doc.read(cx).custom_features();
                this.sync_tabs(window, cx);
                cx.notify();
            }
            // Tabs without their own message display show Onshape's bubble here.
            DocEvent::Error(msg) | DocEvent::Info(msg) => {
                let kind = if matches!(ev, DocEvent::Error(_)) { crate::ui::AlertKind::Error } else { crate::ui::AlertKind::Info };
                this.toast = Some((kind, msg.clone()));
                cx.notify();
            }
            _ => cx.notify(),
        })];
        // When the focused element goes away (a menu, a prompt, a closed
        // panel), shortcuts keep working: focus returns to the active tab.
        let focus_lost = cx.on_focus_lost(window, |this: &mut Self, window, cx| this.focus_active(window, cx));
        let active = doc.read(cx).session.doc.ordered_tabs().first().map(|t| t.id);
        let _ = doc.read(cx).custom_features();
        let thumbnailer = cx.new(|cx| crate::start_thumbnail::Thumbnailer::new(doc.clone(), gpu.clone(), cx));
        let tab_thumbs = cx.new(|_| crate::tab_thumbs::TabThumbs::new(doc.clone()));
        let mut this = Self { tab_thumbs, tab_scroll: ScrollHandle::new(), reveal_active_tab: true, navbar_hovered: false, cursor_sync: Default::default(), doc, gpu, active, views: HashMap::new(), focus: cx.focus_handle(), slices: None, logo_focus: cx.focus_handle().tab_stop(true), history: None, release_drafts: None, recent: Vec::new(), tab_manager: None, doc_panel: None, renaming_tab: None, units_panel: None, open_folder: None, fs_notices: None, fs_search: None, create_drawing: None, _thumbnailer: thumbnailer, toast: None, _subs: subs };
        this._subs.push(focus_lost);
        this.sync_tabs(window, cx);
        // Focus the active tab view so its shortcuts work immediately.
        if let Some(a) = this.active {
            cx.defer_in(window, move |this: &mut Self, window, cx| this.activate(a, window, cx));
        }
        if crate::smoke::enabled() {
            crate::smoke::start(window, cx);
        }
        this
    }

    fn sync_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tabs: Vec<(TabId, &'static str)> = self.doc.read(cx).session.doc.ordered_tabs().iter().map(|t| (t.id, kind_of(&t.content))).collect();
        self.views.retain(|id, _| tabs.iter().any(|(t, _)| t == id));
        if self.active.is_none_or(|a| !tabs.iter().any(|(t, _)| *t == a)) {
            self.active = tabs.first().map(|t| t.0);
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
                // In-context banner › Go to assembly.
                self._subs.push(cx.subscribe_in(&v, window, |this: &mut Self, _, ev: &crate::part_studio::PartStudioEvent, window, cx| match ev {
                    crate::part_studio::PartStudioEvent::ActivateTab(t) => this.activate(*t, window, cx),
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
                    crate::assembly_view::AssemblyViewEvent::EditInContext(t, ctx) => {
                        this.activate(*t, window, cx);
                        if let Some(TabView::PartStudio(v)) = this.views.get(t) {
                            let ctx = (**ctx).clone();
                            v.update(cx, |v, cx| v.set_assembly_context(Some(ctx), cx));
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

    pub(crate) fn activate(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        self.reveal_active_tab = true;
        if let Some(prev) = self.active.filter(|p| *p != tab) {
            self.recent.retain(|t| *t != prev && *t != tab);
            self.recent.insert(0, prev);
            self.recent.truncate(16);
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

    /// Give keyboard focus to the active tab's editor.
    fn focus_active(&self, window: &mut Window, cx: &mut Context<Self>) {
        let fh = match self.active.and_then(|t| self.views.get(&t)) {
            Some(TabView::PartStudio(v)) => v.read(cx).focus_handle(cx),
            Some(TabView::Drawing(v)) => v.read(cx).focus.clone(),
            Some(TabView::Assembly(v)) => v.read(cx).focus_handle(cx),
            Some(TabView::FeatureStudio(v)) => v.read(cx).editor_focus(cx),
            _ => return,
        };
        window.focus(&fh, cx);
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
        let users: Vec<String> = doc.session.doc.tab_users(tab).into_iter().filter_map(|u| doc.session.doc.tab(u).map(|t| t.name.clone())).collect();
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
        let tab_name = self.doc.read(cx).session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
        let delete = std::rc::Rc::new(delete);
        window.open_dialog(cx, move |dialog, _, cx| {
            let delete = delete.clone();
            // Onshape modal: title bar, message, blue Delete then gray Cancel.
            crate::os_ui::modal(dialog, "Delete tabs", 460.)
                .child(crate::os_ui::modal_body().child(format!("Deleting \"{tab_name}\" may cause errors in {} that use it: {}.", if users.len() == 1 { "the tab" } else { "the tabs" }, users.join(", "))))
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
                    this.doc.update(cx, |d, cx| {
                        d.edit_document(&format!("Rename tab : {name}"), cx, |doc| {
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

    /// Tab › Select as document thumbnail.
    fn select_thumbnail(&mut self, tab: TabId, cx: &mut Context<Self>) {
        self.doc.update(cx, |d, cx| {
            let _ = d.session.store.set_meta("thumbnail_tab", Some(tab.0.to_string().as_bytes()));
            cx.emit(DocEvent::HistoryChanged);
        });
    }

    /// "+" › Paste <element>: the element copied with Copy to clipboard.
    fn paste_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = cx.try_global::<crate::doc_dialogs::CopiedTab>().and_then(|c| c.0.clone()) else { return };
        let mut new_id = None;
        let after = self.active;
        self.doc.update(cx, |doc, cx| {
            doc.edit_document(&format!("Paste tab : {}", t.name), cx, |d| {
                let name = if d.tabs.iter().any(|x| x.name == t.name) { d.next_tab_name(&t.name) } else { t.name.clone() };
                let id = d.add_tab(&name, t.content.clone());
                place_after(d, id, after);
                new_id = Some(id);
            });
        });
        if let Some(id) = new_id {
            self.activate(id, window, cx);
        }
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
        let branch_name = doc.session.store.branch(doc.session.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into());
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
                            .w(px(123.))
                            .gap(px(10.))
                            .cursor_pointer()
                            // Onshape: the logo goes to the documents page.
                            .on_click(|_, _, cx| crate::files::show_start_window(cx))
                            .child(crate::assets::icon("noshape-logo", 24., cx))
                            .child(div().text_size(px(21.)).font_weight(FontWeight::NORMAL).text_color(gpui_kit::rgb(0x4a4a4a)).child("noshape")),
                    )
                    .child(self.render_doc_menu(cx))
                    .child(div().flex_none().pr(px(4.)).py(px(2.)).text_size(px(22.)).font_weight(FontWeight::BOLD).text_color(pal.text).truncate().cursor_text().child(name))
                    // Live: on the name's baseline, 11 px after it. The name and this
                    // label show the I-beam (dom/cursors-controls.json).
                    // Live (navigation/301): a version shows its name ("V1", #999) in the
                    // branch's place and no banner (its view-only bar is in the toolbar row).
                    .child(div().flex_none().mt(px(6.)).text_size(px(16.)).text_color(pal.text_faint).cursor_text().child(match (&viewing_version, read_only) {
                        (Some(v), _) => v.clone(),
                        (None, true) => "History".to_string(),
                        (None, false) => branch_name,
                    }))
                    .when(read_only && viewing_version.is_none(), |d| d.child(div().ml_2().px_2().rounded(px(2.)).bg(pal.warning).text_xs().text_color(pal.text).child("VIEWING HISTORY")))
                    .child(div().flex_1())
                    .children(self.render_notices_button(cx))
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

    /// Help ▾ at the right of the navbar (Onshape's `?▾`).
    fn render_help_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        Button::new("help-menu")
            .custom(navbar_variant(cx))
            // Live (part-studio/00): a filled dark "?" disc and a solid caret.
            .child(h_flex().gap(px(6.)).child(crate::assets::icon("help", 20., cx)).child(crate::ui::glyph("ui-caret-down", 9., pal.text)))
            .h(px(crate::ui::NAVBAR_H))
            .px(px(6.))
            .rounded(px(0.))
            .os_dropdown_menu("help-menu-dd", false, |menu, _, _| {
                menu.item(crate::os_menu::OsMenuItem::new("Keyboard shortcuts").on_click(|_, window, cx| window.dispatch_action(Box::new(ToggleShortcutHelp), cx)))
                    .separator()
                    .item(crate::os_menu::OsMenuItem::new("About noshape").disabled(true))
            })
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
        Button::new("doc-menu")
            .custom(navbar_variant(cx))
            .child(crate::assets::icon("menu", 20., cx))
            .tooltip("Document menu")
            .w(px(32.))
            .h(px(crate::ui::NAVBAR_H))
            .p_0()
            .rounded(px(0.))
            .on_click(move |e, window, cx| {
                use crate::doc_dialogs as dd;
                use crate::os_menu::OsMenuItem as I;
                let doc = ws.read(cx).doc.clone();
                let x = e.position().x;
                let (d1, d2, d3, d4, d5, d6, d7, d8) = (doc.clone(), doc.clone(), doc.clone(), doc.clone(), doc.clone(), doc.clone(), doc.clone(), doc.clone());
                let (w, w2) = (ws.clone(), ws.clone());
                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                    m.dropdown()
                        .item(I::new("Rename document").on_click(move |_, wi, cx| dd::rename_document(d1.clone(), wi, cx)))
                        .item(I::new("Move to…").on_click(move |_, wi, cx| dd::move_to(d2.clone(), wi, cx)))
                        .item(I::new("Document details…").on_click(move |_, wi, cx| dd::document_details(d3.clone(), wi, cx)))
                        .item(I::new("Restore deleted workspaces…").on_click(move |_, wi, cx| dd::restore_deleted_workspaces(d4.clone(), wi, cx)))
                        .separator()
                        .item(I::new("Copy workspace…").on_click(move |_, wi, cx| dd::copy_workspace(d5.clone(), wi, cx)))
                        .item(I::new("Update workspace…").on_click(move |_, wi, cx| dd::update_workspace(d6.clone(), wi, cx)))
                        .item(I::new("Workspace units…").on_click(move |_, _, cx| w.update(cx, |t, cx| t.open_units_panel(cx))))
                        .item(I::new("Workspace properties…").on_click(move |_, wi, cx| dd::workspace_properties(d7.clone(), wi, cx)))
                        .item(I::new(if view_only { "Switch to edit mode" } else { "Switch to view only mode" }).on_click(move |_, _, cx| {
                            d8.update(cx, |d, cx| {
                                d.session.view_only = !d.session.view_only;
                                cx.notify();
                            })
                        }))
                        .separator()
                        .item(I::new("Print…").on_click(move |_, _, cx| w2.update(cx, |t, cx| t.print_active(cx))))
                        .separator()
                        .item(I::new("Close document").on_click(|_, wi, _| wi.remove_window()))
                });
                crate::os_menu::open_anchored(point(x - px(16.), px(crate::ui::NAVBAR_H)), Anchor::TopLeft, menu, window, cx);
            })
            .into_any_element()
    }

    /// ☰ › Workspace units…: Onshape's panel at the feature dialog's place.
    fn open_units_panel(&mut self, cx: &mut Context<Self>) {
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
    fn print_active(&mut self, cx: &mut Context<Self>) {
        if let Some(TabView::Drawing(v)) = self.active.and_then(|a| self.views.get(&a)) {
            v.update(cx, |v, cx| v.print(cx));
        }
    }

    /// Drawing tab › Change to version…: the versions at the pointer.
    fn change_drawing_version(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        self.activate(tab, window, cx);
        let Some(TabView::Drawing(v)) = self.views.get(&tab) else { return };
        let v = v.clone();
        let entries = v.read(cx).version_menu(cx);
        let at = window.mouse_position();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| crate::drawing_view::fill_os_menu(m, &entries, &v, window, cx));
        crate::os_menu::open(at, menu, window, cx);
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
                        last = Some(f);
                    })
                });
            }
            let ws2 = ws.clone();
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    if let Some(id) = last {
                        ws2.update(cx, |this, cx| this.activate(id, window, cx));
                    }
                    if !cad.is_empty() {
                        crate::import_dialog::open(doc, cad, after, true, move |id, window, cx| ws.update(cx, |this, cx| this.activate(id, window, cx)), window, cx)
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
                let ws = cx.entity();
                crate::import_dialog::open(self.doc.clone(), vec![path], Some(id), false, move |t, window, cx| ws.update(cx, |this, cx| this.activate(t, window, cx)), window, cx);
            }
        }
    }

    /// Left rail › Document notes (`notes`) or Performance; one left panel
    /// at a time, a second click closes it.
    fn toggle_doc_panel(&mut self, notes: bool, cx: &mut Context<Self>) {
        let same = match &self.doc_panel {
            Some((DocPanel::Notes(_), _)) => notes,
            Some((DocPanel::Performance(_), _)) => !notes,
            None => false,
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
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_tab_manager(window, cx)));
        // Onshape's "+" menu (live, document/01): a Bootstrap dropdown opening
        // upward from the cell. Cloud items (Applications, CAM / PCB /
        // Render Studio) are not part of noshape.
        let add = cell("add-tab", cx).w(px(34.)).child(crate::ui::glyph("plus", 20., gpui_kit::rgb(0x555555))).tooltip("Insert new tab").on_click(move |_, window, cx| {
            let (v1, v2, v3, v4, v5, v6, v7) = (view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone());
            let (v8, v9) = (view.clone(), view.clone());
            let copied = cx.try_global::<crate::doc_dialogs::CopiedTab>().and_then(|c| c.0.as_ref().map(|t| t.name.clone()));
            let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                use crate::os_menu::OsMenuItem as I;
                let m = m.dropdown();
                let m = match &copied {
                    Some(n) => m.item(I::new(format!("Paste {n}")).icon_name("paste").on_click(move |_, w, cx| v7.update(cx, |t, cx| t.paste_tab(w, cx)))).separator(),
                    None => m,
                };
                m
                    .item(I::new("Create Material Library").icon_name("material-library").on_click(move |_, w, cx| v9.update(cx, |t, cx| t.add_tab("materiallibrary", w, cx))))
                    .item(I::new("Create Feature Studio").icon_name("feature-studio").on_click(move |_, w, cx| v5.update(cx, |t, cx| t.add_tab("featurestudio", w, cx))))
                    .separator()
                    .item(I::new("Create Part Studio").icon_name("part-studio").on_click(move |_, w, cx| v1.update(cx, |t, cx| t.add_tab("partstudio", w, cx))))
                    .item(I::new("Create Assembly").icon_name("assembly").on_click(move |_, w, cx| v2.update(cx, |t, cx| t.add_tab("assembly", w, cx))))
                    .item(I::new("Create Variable Studio").icon_name("variable-studio").on_click(move |_, w, cx| v4.update(cx, |t, cx| t.add_tab("variables", w, cx))))
                    .item(I::new("Create Drawing…").icon_name("drawing").on_click(move |_, w, cx| v3.update(cx, |t, cx| t.open_create_drawing(crate::drawing_create::CreateRequest { source: None, name: None }, w, cx))))
                    .item(I::new("Create folder").icon_name("folder").on_click(move |_, w, cx| v8.update(cx, |t, cx| t.create_folder(w, cx))))
                    .item(I::new("Import…").icon_name("import-file").on_click(move |_, w, cx| v6.update(cx, |t, cx| t.import(w, cx))))
            });
            let h = window.viewport_size().height;
            crate::os_menu::open_anchored(point(px(crate::ui::RAIL_W), h - px(crate::ui::TAB_BAR_H)), Anchor::BottomLeft, menu, window, cx);
        });
        // The active tab's place among the strip's children (home, the
        // breadcrumb tab and the open folder come first inside a folder).
        let lead = if open_name.is_some() { 2 + usize::from(!crumbs.is_empty()) } else { 0 };
        if let Some(ix) = items.iter().position(|(id, _, _)| self.active == Some(*id)).filter(|_| self.reveal_active_tab) {
            self.tab_scroll.scroll_to_item(lead + ix);
            self.reveal_active_tab = false;
        }
        let mut strip = h_flex().id("tab-strip").flex_1().min_w_0().h_full().items_start().pl(px(5.)).gap(px(3.)).overflow_x_scroll().track_scroll(&self.tab_scroll);
        if let Some(name) = open_name {
            let folder = open.unwrap_or(TabId(0));
            // "All tabs" (home): back to every tab; dropping here moves to the root.
            strip = strip.child(
                tab_chip("tab-home", false)
                    .px(px(8.))
                    .child(os::glyph(os::GLYPH_HOME, 18., pal.text_muted))
                    .tooltip(crate::ui::tooltip("All tabs"))
                    .drag_over::<TabDrag>(|st, _, _, _| st.bg(gpui_kit::rgb(0xdef1fb)))
                    .on_drop(cx.listener(|this, drag: &TabDrag, _, cx| this.move_into(drag.id, None, cx)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.open_folder = None;
                        cx.notify();
                    })),
            );
            // More than one level deep: an overflow tab lists the parent folders.
            if !crumbs.is_empty() {
                let v = cx.entity();
                strip = strip.child(tab_chip("tab-crumbs", false).px(px(10.)).child("…").on_click(move |e, window, cx| {
                    let (crumbs, v) = (crumbs.clone(), v.clone());
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                        m = m.dropdown();
                        for (id, name) in crumbs {
                            let v = v.clone();
                            m = m.item(crate::os_menu::OsMenuItem::new(name).icon(os::glyph(os::GLYPH_FOLDER, 20., p().text)).on_click(move |_, _, cx| {
                                v.update(cx, |t, cx| {
                                    t.open_folder = Some(id);
                                    cx.notify();
                                })
                            }));
                        }
                        m
                    });
                    let h = window.viewport_size().height;
                    crate::os_menu::open_anchored(point(e.position().x, h - px(crate::ui::TAB_BAR_H)), Anchor::BottomLeft, menu, window, cx);
                }));
            }
            strip = strip.child(self.folder_tab(folder, name, true, cx));
        }
        for (id, name, kind) in items {
            let Some(kind) = kind else {
                strip = strip.child(self.folder_tab(id, name, false, cx));
                continue;
            };
            let active = self.active == Some(id);
            // Compare mode (live 952): a status dot after the name; drawings and
            // tabs only in the base are greyed and can't be opened.
            let compare = crate::compare_mode::tab_status(&self.doc, id, cx);
            let compare_off = compare.is_some_and(|s| s.disabled());
            let compare_dot = compare.and_then(|s| crate::compare_mode::tab_dot(SharedString::from(format!("tab-dot-{id}")), s, &crate::compare_mode::base_label(&self.doc, cx).unwrap_or_default()));
            let view = cx.entity();
            let tab_name = name.clone();
            let bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
            let probe = bounds.clone();
            strip = strip.child(
                h_flex()
                    .id(SharedString::from(format!("tab-{id}")))
                    .relative()
                    .flex_none()
                    // Live (part-studio/00, behaviours "Tab bar"): fixed-width
                    // tabs, 198 px, 3 px apart; long names truncate.
                    .w(px(198.))
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
                    .child(div().flex_none().when(compare_off, |d| d.opacity(0.5)).child(crate::assets::icon(tab_icon(kind), 20., cx)))
                    .child(match self.renaming_tab.as_ref().filter(|r| r.0 == id) {
                        // Tab › Rename / double-click: the name becomes a field.
                        Some((_, st, _)) => div().w(px(150.)).child(gpui_kit::component::input::Input::new(st).h(px(22.)).text_size(px(14.)).px(px(3.)).text_color(pal.text)).into_any_element(),
                        None => div().truncate().when(compare_off, |d| d.opacity(0.5)).child(name).into_any_element(),
                    })
                    .children(compare_dot.map(|dot| div().flex_none().ml(px(3.)).when(compare_off, |d| d.opacity(0.55)).child(dot)))
                    .when(active, |d| d.child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(pal.accent)))
                    .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                    // Live (part-studio/17): an inactive tab shows its element's
                    // picture in a card above it.
                    .when(!active && matches!(kind, "partstudio" | "assembly" | "drawing"), |d| {
                        let (thumbs, anchor) = (self.tab_thumbs.clone(), bounds.clone());
                        d.tooltip(move |_, cx| {
                            let (thumbs, anchor) = (thumbs.clone(), anchor.get());
                            cx.new(|cx| crate::tab_thumbs::TabCard::new(thumbs, id, anchor, cx)).into()
                        })
                    })
                    // Drag to reorder; drop a tab on a folder to file it.
                    .on_drag(TabDrag { id, name: tab_name.clone(), icon: Some(tab_icon(kind)) }, |d, _, _, cx| cx.new(|_| d.clone()))
                    .drag_over::<TabDrag>(|st, _, _, _| st.border_l_2().border_color(gpui_kit::rgb(0x1651b0)))
                    .on_drop(cx.listener(move |this, drag: &TabDrag, _, cx| this.move_before(drag.id, id, cx)))
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        if compare_off {
                            return;
                        }
                        // The first click activated the tab; a double-click renames it
                        // (activating again would take the field's focus).
                        if e.click_count() >= 2 {
                            this.start_tab_rename(id, window, cx);
                        } else {
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
                        // Onshape (live menus/rmb-tab-*): ⊗ Delete | Rename,
                        // Properties…, Show code | Duplicate, Copy to clipboard |
                        // (Assembly) Create Drawing of <tab>… | Select as document
                        // thumbnail | Move to document…, Export…, Release….
                        let menu = menu
                            .item(I::new("Delete").icon_name("menu-delete").on_click(move |_, w, cx| v2.update(cx, |t, cx| t.delete_tab(id, w, cx))))
                            .separator()
                            .item(I::new("Rename").on_click(move |_, w, cx| v5.update(cx, |t, cx| t.start_tab_rename(id, w, cx))))
                            .item(I::new("Properties…").on_click(move |_, w, cx| {
                                let doc = v6.read(cx).doc.clone();
                                dd::tab_properties(doc, id, w, cx)
                            }));
                        let menu = if kind == "partstudio" {
                            menu.item(I::new("Show code").on_click(move |_, w, cx| {
                                let doc = v7.read(cx).doc.clone();
                                dd::show_code(doc, id, w, cx)
                            }))
                        } else {
                            menu
                        };
                        let menu = menu
                            .separator()
                            .item(I::new("Duplicate").on_click(move |_, _, cx| v1.update(cx, |t, cx| t.duplicate_tab(id, cx))))
                            .item(I::new("Copy to clipboard").on_click(move |_, _, cx| {
                                let t = v8.read(cx).doc.read(cx).session.doc.tab(id).cloned();
                                cx.set_global(dd::CopiedTab(t));
                            }))
                            .separator();
                        // Drawings: Change to version… (live tab menu, after Copy to
                        // clipboard): point every view at a version of the document.
                        let menu = if kind == "drawing" {
                            let v12 = view.clone();
                            menu.item(I::new("Change to version…").icon_name("version").on_click(move |_, w, cx| v12.update(cx, |t, cx| t.change_drawing_version(id, w, cx)))).separator()
                        } else {
                            menu
                        };
                        // Live menus/rmb-tab-assembly: its own group after Copy to clipboard.
                        let menu = if kind == "assembly" {
                            let n = tab_name.clone();
                            menu.item(I::new(format!("Create Drawing of {tab_name}…")).icon_name("menu-create-drawing").on_click(move |_, w, cx| {
                                let source = Some(noshape_model::drawing::create::DrawingSource { tab: id, part: None, config: Default::default(), sketch: None });
                                let req = crate::drawing_create::CreateRequest { source, name: Some(n.clone()) };
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
                        let menu = if matches!(kind, "partstudio" | "assembly" | "drawing") { menu.item(I::new("Export…").on_click(move |_, w, cx| v11.update(cx, |t, cx| t.open_export(id, w, cx)))) } else { menu };
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
                        // Material Library tabs: Export… (CSV) and Update… (help: customizing part materials).
                        if kind == "materiallibrary" {
                            let (d1, d2) = (view.read(cx).doc.clone(), view.read(cx).doc.clone());
                            menu.item(I::new("Export…").on_click(move |_, w, cx| crate::material_library_view::export(d1.clone(), id, w, cx)))
                                .item(I::new("Update…").on_click(move |_, w, cx| crate::material_library_view::update(d2.clone(), id, w, cx)))
                        } else {
                            menu
                        }
                    }),
            );
        }
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
        (tab_list.into_any_element(), h_flex().w_full().min_w_0().h(px(crate::ui::TAB_BAR_H)).flex_none().bg(pal.tab_bar).child(add).child(strip).children(arrows).child(watch).into_any_element())
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
        let perf_error = crate::doc_panels::PerfSnapshot { adapter: self.gpu.adapter_name(), ..Default::default() }.software_renderer();
        let buttons = v_flex()
            .w(px(crate::ui::RAIL_W))
            .flex_1()
            .min_h_0()
            .pt(px(1.))
            .gap(px(4.))
            .bg(pal.rail)
            .border_r_1()
            .border_t_1()
            .border_color(pal.border)
            .child(
                crate::ui::rail_button("rail-history", "history", history_open, true, cx)
                    .tip("Versions and history", TipSide::Right)
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_history(window, cx))),
            )
            .child(
                crate::ui::rail_button("rail-version", "version", false, true, cx)
                    .tip("Create version…", TipSide::Right)
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| crate::history_panel::open_create_version(this.doc.clone(), window, cx))),
            )
            // Live order: Versions and history, Create version…, (Comments),
            // Document notes, (Where used), Performance, (AI Advisor).
            .child(
                crate::ui::rail_button("rail-notes", "doc-notes", notes_open, true, cx)
                    .tip("Document notes", TipSide::Right)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_doc_panel(true, cx))),
            )
            .child(
                crate::ui::rail_button("rail-performance", "performance", perf_open, true, cx)
                    // A software renderer is a performance error (live: a red badge).
                    .when(perf_error, |d| d.child(div().absolute().left(px(22.)).top(px(16.)).size(px(11.)).rounded_full().bg(gpui_kit::white()).child(crate::assets::icon("error", 11., cx))))
                    .tip(if perf_error { "Performance errors detected" } else { "Performance" }, TipSide::Right)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_doc_panel(false, cx))),
            )
            // Release management (help releasedraftspanel.png).
            .child(
                crate::ui::rail_button("rail-release-drafts", "release-drafts", self.release_drafts.is_some(), true, cx)
                    .tip("Release drafts", TipSide::Right)
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
}

/// A file tab's right-click menu (live menus/rmb-tab-file-*, behaviours
/// "File tabs" and "Imported DWG/DXF drawings"): Delete | Rename,
/// Properties… | (CAD) Import…, (DXF / DWG) Create Drawing of <file>…,
/// Update… | (images) Select as document thumbnail | Move to document… |
/// Release… | Download. No Translate…; "Open in new browser tab" is the
/// browser's.
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
    let mut menu = menu
        .item(I::new("Delete").icon_name("menu-delete").on_click(move |_, w, cx| v1.update(cx, |t, cx| t.delete_tab(id, w, cx))))
        .separator()
        .item(I::new("Rename").on_click(move |_, w, cx| v2.update(cx, |t, cx| t.start_tab_rename(id, w, cx))))
        .item(I::new("Properties…").on_click(move |_, w, cx| dd::tab_properties(d1.clone(), id, w, cx)))
        .separator();
    match file_kind {
        Some(FileKind::Cad) => menu = menu.item(I::new("Import…").on_click(move |_, w, cx| v3.update(cx, |t, cx| t.translate_file(id, w, cx)))),
        Some(FileKind::Drawing2d) => menu = menu.item(I::new(format!("Create Drawing of {tab_name}…")).icon_name("menu-create-drawing").on_click(move |_, w, cx| v3.update(cx, |t, cx| t.create_cad_drawing(id, w, cx)))),
        _ => {}
    }
    menu = menu.item(I::new("Update…").on_click(move |_, w, cx| crate::file_tab_view::update(&d2, id, w, cx))).separator();
    if file_kind == Some(FileKind::Image) {
        menu = menu.item(I::new("Select as document thumbnail").on_click(move |_, _, cx| v4.update(cx, |t, cx| t.select_thumbnail(id, cx)))).separator();
    }
    menu.item(I::new("Move to document…").on_click(move |_, w, cx| dd::move_to_document(d3.clone(), vec![id], w, cx)))
        .separator()
        .item(I::new("Release…").on_click(move |_, w, cx| {
            let items = crate::release_dialog::tab_items(d4.read(cx), id);
            crate::release_dialog::open_release_candidate(d4.clone(), items, w, cx)
        }))
        .separator()
        .item(I::new("Download").on_click(move |_, w, cx| {
            let doc = v5.read(cx).doc.clone();
            crate::file_tab_view::download(&doc, id, w, cx)
        }))
}

fn tab_icon(kind: &'static str) -> &'static str {
    match kind {
        "partstudio" => "part-studio",
        "assembly" => "assembly",
        "drawing" => "drawing",
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
        // Compare mode (compare_mode.rs, live ui2/952): a tab that exists only
        // in the base shows "NOT COMPARABLE".
        let only_in_base = self.active.and_then(|a| crate::compare_mode::tab_status(&self.doc, a, cx)) == Some(crate::compare_mode::TabStatus::OnlyInBase);
        let content: AnyElement = match self.active.and_then(|a| self.views.get(&a)) {
            _ if only_in_base => crate::compare_mode::render_not_comparable(&crate::compare_mode::base_label(&self.doc, cx).unwrap_or_default()),
            Some(TabView::PartStudio(v)) => v.clone().into_any_element(),
            Some(TabView::Drawing(v)) => v.clone().into_any_element(),
            Some(TabView::Variables(v)) => v.clone().into_any_element(),
            Some(TabView::Assembly(v)) => v.clone().into_any_element(),
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
            .on_action(cx.listener(|this, _: &ToggleReleaseDrafts, _, cx| this.toggle_release_drafts(cx)))
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .on_action(cx.listener(|this, _: &crate::actions::FocusNavbar, window, cx| window.focus(&this.logo_focus, cx)))
            // Tool search from anywhere in the window (e.g. after clicking the tab bar).
            .on_action(cx.listener(|this, _: &SearchTools, window, cx| {
                if let Some(s) = this.active_studio() {
                    s.update(cx, |s, cx| s.open_palette(window, cx));
                }
            }))
            // Ctrl+Space: back to the previously active tab (Onshape).
            .on_action(cx.listener(|this, _: &SwitchRecentTab, window, cx| {
                let tabs: Vec<TabId> = this.doc.read(cx).session.doc.ordered_tabs().iter().map(|t| t.id).collect();
                if let Some(t) = this.recent.iter().copied().find(|t| tabs.contains(t)) {
                    this.activate(t, window, cx);
                    cx.notify();
                }
            }))
            // Alt+T: Tab manager (search and open tabs).
            .on_action(cx.listener(|this, _: &TabManager, window, cx| this.toggle_tab_manager(window, cx)))
            // A customized key of an action without its own GPUI action.
            .on_action(cx.listener(|this, a: &crate::shortcuts::RunShortcut, window, cx| this.run_shortcut(a.id, window, cx)))
            // Ctrl+Alt+V (Onshape "Create version"): the Create version dialog.
            .on_action(cx.listener(|this, _: &CreateVersion, window, cx| crate::history_panel::open_create_version(this.doc.clone(), window, cx)))
            .on_action(cx.listener(|this, _: &ExportRequestedItem, window, cx| {
                if let Some((t, parts)) = REQUESTED_EXPORT.lock().take() {
                    this.export_item(t, parts, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ActivateRequestedTab, window, cx| {
                if let Some(t) = REQUESTED_TAB.lock().take() {
                    this.activate(t, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &CreateDrawing, window, cx| {
                if let Some(req) = crate::drawing_create::take_request() {
                    this.open_create_drawing(req, window, cx);
                }
            }))
            .child(title)
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
                                    div().absolute().top_0().left_0().right_0().h(px(crate::ui::TOOLBAR_H)).bg(gpui_kit::white()).occlude().flex().justify_center().child(bar)
                                }))
                                // A protected workspace (workspace_protection.rs): its banner
                                // at the top centre (not over the compare bar).
                                .children(crate::workspace_protection::render_banner(&self.doc, cx).filter(|_| crate::compare_mode::rows(&self.doc, cx).is_none()).map(|banner| div().absolute().top_0().left_0().right_0().flex().justify_center().child(banner)))
                                .children(self.render_toast(cx))
                                // Onshape: at the feature dialog's place (below the toolbar, right of the list).
                                .children(self.units_panel.as_ref().map(|(p, _)| div().absolute().top(px(39.)).left(px(crate::ui::left_panel_w())).child(p.clone()))),
                        )
                        .child(tabs)
                        // Live document/75: the notices panel sits under the tab bar,
                        // right of the left rail (the rail runs the full height).
                        .children(self.fs_notices.as_ref().map(|(p, _)| p.clone())),
                    ),
            )
            .children(self.fs_search.as_ref().map(|(p, _)| p.clone()))
            .children(self.create_drawing.as_ref().map(|(d, _)| d.clone()))
            // Onshape right-click menus opened with `os_context_menu`.
    }
}

impl Workspace {
    /// Runs a customized shortcut (Preferences › Keyboard shortcuts) in the
    /// active tab.
    fn run_shortcut(&mut self, id: &'static str, window: &mut Window, cx: &mut Context<Self>) {
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
        Some(
            div()
                .absolute()
                .top(px(8.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(crate::os_ui::message_bubble("ws-toast", kind, msg, move |_, _, cx| {
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
        let errors = crate::fs_notices::document_notices(&self.doc, cx).iter().any(|(_, _, n)| n.iter().any(|n| n.error));
        Some(
            h_flex()
                .id("fs-notices-btn")
                .h(px(32.))
                .px(px(6.))
                .rounded(px(3.))
                .cursor_pointer()
                .hover(|d| d.bg(p().border))
                .when(self.fs_notices.is_some(), |d| d.bg(p().border))
                .tooltip(crate::ui::tooltip("FeatureScript notices"))
                .items_center()
                .child(crate::assets::icon(if errors { "fs-notices-error" } else { "fs-notices" }, 20., cx))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_fs_notices(window, cx)))
                .into_any_element(),
        )
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
                let name = d.next_folder_name();
                let id = d.add_folder(&name, None);
                let parent = open.or_else(|| after.and_then(|a| d.parent_of(a)));
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

    /// Drop on a folder (or on "All tabs": the root).
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
    fn move_before(&mut self, id: TabId, before: TabId, cx: &mut Context<Self>) {
        if id == before {
            return;
        }
        self.doc.update(cx, |doc, cx| {
            doc.edit_document("Reorder tabs", cx, |d| {
                let parent = d.parent_of(before);
                if d.set_parent(id, parent) {
                    d.move_in_order(id, Some(before));
                }
            });
        });
        cx.notify();
    }

    /// A folder on the tab bar: click opens it, double-click renames;
    /// right-click: Rename, Move to parent folder, Unpack folder, Move to
    /// document (help: Document tabs › Organizing tabs with folders).
    fn folder_tab(&self, id: TabId, name: String, current: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let has_parent = self.doc.read(cx).session.doc.parent_of(id).is_some();
        let view = cx.entity();
        tab_chip(SharedString::from(format!("folder-{id}")), current)
            .min_w(px(60.))
            .max_w(px(200.))
            .px(px(10.))
            .child(os::glyph(os::GLYPH_FOLDER, 20., pal.text))
            .child(match self.renaming_tab.as_ref().filter(|r| r.0 == id) {
                Some((_, st, _)) => div().w(px(150.)).child(gpui_kit::component::input::Input::new(st).h(px(22.)).text_size(px(14.)).px(px(3.)).text_color(pal.text)).into_any_element(),
                None => div().truncate().when(current, |d| d.font_weight(FontWeight::SEMIBOLD)).child(name.clone()).into_any_element(),
            })
            .when(!current, |d| d.on_drag(TabDrag { id, name: name.clone(), icon: None }, |d, _, _, cx| cx.new(|_| d.clone())))
            .drag_over::<TabDrag>(|st, _, _, _| st.bg(gpui_kit::rgb(0xdef1fb)))
            .on_drop(cx.listener(move |this, drag: &TabDrag, _, cx| this.move_into(drag.id, Some(id), cx)))
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if e.click_count() >= 2 {
                    this.start_tab_rename(id, window, cx);
                } else {
                    this.open_folder = Some(id);
                    cx.notify();
                }
            }))
            .os_context_menu(move |menu, _, _| {
                use crate::os_menu::OsMenuItem as I;
                let (v1, v2, v3, v4) = (view.clone(), view.clone(), view.clone(), view.clone());
                menu.item(I::new("Rename").on_click(move |_, w, cx| v1.update(cx, |t, cx| t.start_tab_rename(id, w, cx))))
                    .item(I::new("Move to parent folder").disabled(!has_parent).on_click(move |_, _, cx| {
                        v2.update(cx, |t, cx| {
                            let up = t.doc.read(cx).session.doc.ancestors(id).get(1).copied();
                            t.move_into(id, up, cx)
                        })
                    }))
                    .item(I::new("Unpack folder").on_click(move |_, _, cx| {
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
            })
            .into_any_element()
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

/// Tab to activate, set by views that are not subscribed to by the
/// workspace (e.g. a drawing view's "Switch to <Part Studio>").
static REQUESTED_TAB: parking_lot::Mutex<Option<TabId>> = parking_lot::Mutex::new(None);
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

/// Ask the workspace to switch to `tab`.
pub fn request_activate_tab(tab: TabId, window: &mut Window, cx: &mut App) {
    *REQUESTED_TAB.lock() = Some(tab);
    window.dispatch_action(Box::new(ActivateRequestedTab), cx);
}

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
        let dialog = cx.new(|cx| CreateDrawingDialog::new(req, default_name, metric, window, cx));
        let sub = cx.subscribe_in(&dialog, window, |this: &mut Self, _, ev: &CreateDrawingEvent, window, cx| {
            if let CreateDrawingEvent::Create { name, template, source, suggested } = ev {
                this.create_drawing_tab(name.clone(), *template, source.clone(), *suggested, window, cx);
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
    fn create_drawing_tab(&mut self, name: String, template: noshape_model::drawing::templates::DrawingTemplate, source: Option<noshape_model::drawing::create::DrawingSource>, suggested: bool, window: &mut Window, cx: &mut Context<Self>) {
        let doc = self.doc.read(cx).session.doc.clone();
        cx.spawn_in(window, async move |this, cx| {
            let src = source.clone();
            let drawing = cx
                .background_spawn(async move {
                    let bounds = if suggested { src.as_ref().and_then(|s| noshape_model::drawing::create::source_bounds(&doc, s)) } else { None };
                    // Sketch sources face the sketch plane.
                    let frame = src.as_ref().and_then(|s| noshape_model::drawing::create::source_frame(&doc, s));
                    let mut d = noshape_model::drawing::create::create_drawing(&template, src.as_ref(), bounds, suggested, frame);
                    // Suggested views show the Part Studio's MBD datums (live behaviours "MBD datums in drawings").
                    if suggested {
                        noshape_model::drawing::mbd::place_model_datums(&mut d, &doc);
                    }
                    d
                })
                .await;
            let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                let mut new_id = None;
                this.doc.update(cx, |doc, cx| {
                    doc.edit_document(&format!("Insert tab : {name}"), cx, |d| {
                        new_id = Some(docedit::insert_drawing_tab(d, &name, drawing, source.as_ref().map(|s| s.tab)));
                    });
                });
                let Some(id) = new_id else { return };
                this.activate(id, window, cx);
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
            let tm = cx.new(|cx| Panel::new(doc, active, window, cx));
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
}

impl Render for TabDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        h_flex()
            .h(px(27.))
            .px(px(10.))
            .gap(px(3.))
            .rounded_b(px(5.))
            .bg(pal.panel)
            .opacity(0.9)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(2.)), blur_radius: px(4.), spread_radius: px(0.), inset: false }])
            .text_size(px(14.))
            .text_color(pal.text)
            .child(match self.icon {
                Some(i) => crate::assets::icon(i, 20., cx),
                None => os::glyph(os::GLYPH_FOLDER, 20., pal.text).into_any_element(),
            })
            .child(self.name.clone())
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

/// File types "Import files…" translates.
pub(crate) const IMPORT_EXTS: &[&str] =
    &["step", "stp", "iges", "igs", "brep", "stl", "obj", "pvz", "x_t", "x_b", "xmt_txt", "xmt_bin", "3dm", "sldprt", "par", "psm", "prt", "ipt", "sat", "sab", "3mf", "gltf", "glb", "jt", "zip"];

/// Tab (and document) name of an imported file: the file name without its
/// extension; and the full file name.
pub(crate) fn import_names(path: &std::path::Path) -> (String, String) {
    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let stem = path.file_stem().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| file.clone());
    (stem, file)
}

/// Read a CAD or mesh file as BREP text. Mesh files carry no unit: they are
/// read in the document's length unit.
/// A translated CAD file: its shapes as BREP text (a compound, one child per
/// part when `parts` is set) and what the file says about each part (JSON
/// list of `ImportPartInfo`, empty when the format names none).
#[derive(Clone)]
pub(crate) struct Foreign {
    pub brep: String,
    pub parts: String,
}

pub(crate) fn read_foreign(path: &std::path::Path, units: &noshape_units::UnitPrefs) -> anyhow::Result<Foreign> {
    use noshape_kernel as k;
    let mesh_unit = {
        use noshape_units::LengthUnit as U;
        match units.length {
            U::Centimeter => k::LengthUnit::Centimeter,
            U::Meter => k::LengthUnit::Meter,
            U::Inch => k::LengthUnit::Inch,
            U::Foot => k::LengthUnit::Foot,
            U::Yard => k::LengthUnit::Yard,
            _ => k::LengthUnit::Millimeter,
        }
    };
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    // Parts with names and colours: one compound child each, described in `info`.
    let mut info: Vec<noshape_model::features::importing::ImportPartInfo> = Vec::new();
    let mut compound = |parts: Vec<k::ImportedShape>| {
        info = parts.iter().map(|p| noshape_model::features::importing::ImportPartInfo { name: p.name.clone(), color: p.color, opacity: p.opacity, faces: p.face_colors.clone() }).collect();
        k::Shape::compound(&parts.into_iter().map(|p| p.shape).collect::<Vec<_>>())
    };
    let shape = match ext.as_str() {
        "step" | "stp" => compound(k::read_step(path, k::LengthUnit::Meter)?),
        "iges" | "igs" => k::read_iges(path, k::LengthUnit::Meter)?,
        "brep" => k::read_brep(path)?,
        // Onshape imports STL, OBJ and PVZ as mesh parts (PVZ is in metres).
        "stl" | "obj" => k::read_mesh_bodies(path, mesh_unit)?,
        // 3MF and glTF carry their units (read in millimetres).
        "3mf" | "gltf" | "glb" => k::read_mesh_bodies(path, k::LengthUnit::Millimeter)?,
        "pvz" => k::read_pvz(path, k::LengthUnit::Meter)?,
        "x_t" | "x_b" | "xmt_txt" | "xmt_bin" => compound(k::read_parasolid_parts(path, k::LengthUnit::Meter)?),
        "3dm" => compound(k::read_rhino_parts(path, k::LengthUnit::Meter)?),
        // Native CAD parts: the geometry they embed (Parasolid: SolidWorks, Solid Edge, NX; ACIS: Inventor).
        "sat" | "sab" => compound(k::read_acis_parts(path, k::LengthUnit::Meter)?),
        "jt" => compound(k::read_jt_parts(path, k::LengthUnit::Meter)?),
        "sldprt" | "par" | "psm" | "prt" | "ipt" => compound(k::read_native_parts(path, k::LengthUnit::Meter)?),
        other => anyhow::bail!("Unsupported file type .{other}"),
    };
    let parts = if info.is_empty() { String::new() } else { serde_json::to_string(&info)? };
    Ok(Foreign { brep: String::from_utf8_lossy(&shape.to_brep_bytes()?).to_string(), parts })
}

/// Append an Import feature ("Import N") holding `brep` to `ps`.
pub(crate) fn add_import_feature(ps: &mut noshape_model::PartStudio, file: &str, foreign: Foreign, units: &noshape_units::UnitPrefs) {
    if let Some(mut f) = ps.new_feature("importForeign", units) {
        // Live (documents/64-import-feature-dialog.png): the feature is
        // numbered like any other ("Import 1") and allows faulty parts.
        f.params.set("allowFaultyParts", noshape_model::ParamValue::Bool(true));
        f.params.set("blobData", noshape_model::ParamValue::Text(foreign.brep));
        f.params.set("partInfo", noshape_model::ParamValue::Text(foreign.parts));
        f.params.set("foreignId", noshape_model::ParamValue::Text(file.to_string()));
        ps.insert_at_rollback(f);
    }
}

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
