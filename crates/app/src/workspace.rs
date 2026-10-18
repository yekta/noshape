//! Document window: title bar, active tab content and the bottom tab bar.

use crate::actions::*;
use crate::assembly_view::AssemblyView;
use crate::doc_model::{DocEvent, DocModel};
use crate::gpu::Gpu;
use crate::history_panel::HistoryPanel;
use crate::part_studio::PartStudioView;
use crate::variable_studio::VariableStudioView;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, IconName, Sizable as _, TitleBar, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use noshape_model::assembly::Assembly;
use noshape_model::document::{TabContent, TabId};
use noshape_model::drawing::Drawing;
use noshape_model::partstudio::PartStudio;
use noshape_model::variables::VariableStudio;
use std::collections::HashMap;

enum TabView {
    Variables(Entity<VariableStudioView>),
    PartStudio(Entity<PartStudioView>),
    Drawing(Entity<crate::drawing_view::DrawingEditor>),
    Assembly(Entity<AssemblyView>),
    FeatureStudio(Entity<crate::feature_studio::FeatureStudioView>),
}

pub struct Workspace {
    pub doc: Entity<DocModel>,
    gpu: Gpu,
    active: Option<TabId>,
    views: HashMap<TabId, TabView>,
    focus: FocusHandle,
    history: Option<Entity<HistoryPanel>>,
    /// Previously active tabs, most recent first (Ctrl+Space).
    recent: Vec<TabId>,
    /// Tab manager (Alt+T), docked left like Onshape's.
    tab_manager: Option<(Entity<crate::tab_manager::TabManager>, Subscription)>,
    /// Tab being renamed (tab › Rename, double-click).
    renaming_tab: Option<(TabId, Entity<gpui_kit::component::input::InputState>, Subscription)>,
    /// ☰ › Workspace units… panel.
    units_panel: Option<(Entity<crate::doc_dialogs::WorkspaceUnits>, Subscription)>,
    /// Create Drawing dialog (see `drawing_create`).
    create_drawing: Option<(Entity<crate::drawing_create::CreateDrawingDialog>, Subscription)>,
    _thumbnailer: Entity<crate::start_thumbnail::Thumbnailer>,
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
            _ => cx.notify(),
        })];
        let active = doc.read(cx).session.doc.ordered_tabs().first().map(|t| t.id);
        let _ = doc.read(cx).custom_features();
        let thumbnailer = cx.new(|cx| crate::start_thumbnail::Thumbnailer::new(doc.clone(), gpu.clone(), cx));
        let mut this = Self { doc, gpu, active, views: HashMap::new(), focus: cx.focus_handle(), history: None, recent: Vec::new(), tab_manager: None, renaming_tab: None, units_panel: None, create_drawing: None, _thumbnailer: thumbnailer, _subs: subs };
        this.sync_tabs(window, cx);
        // Focus the active tab view so its shortcuts work immediately.
        if let Some(a) = this.active {
            cx.defer_in(window, move |this: &mut Self, window, cx| this.activate(a, window, cx));
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
                TabView::PartStudio(cx.new(|cx| PartStudioView::new(doc, tab, gpu, window, cx)))
            }
            Some("assembly") => {
                let doc = self.doc.clone();
                let gpu = self.gpu.clone();
                let v = cx.new(|cx| AssemblyView::new(doc, tab, gpu, window, cx));
                // "Switch to Part Studio / Assembly" from the instance context menu.
                self._subs.push(cx.subscribe_in(&v, window, |this: &mut Self, _, ev: &crate::assembly_view::AssemblyViewEvent, window, cx| match ev {
                    crate::assembly_view::AssemblyViewEvent::ActivateTab(t) => this.activate(*t, window, cx),
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
                TabView::FeatureStudio(cx.new(|cx| crate::feature_studio::FeatureStudioView::new(doc, tab, window, cx)))
            }
            _ => return,
        };
        self.views.insert(tab, view);
    }

    fn activate(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(prev) = self.active.filter(|p| *p != tab) {
            self.recent.retain(|t| *t != prev && *t != tab);
            self.recent.insert(0, prev);
            self.recent.truncate(16);
        }
        self.active = Some(tab);
        // Ctrl+Z / Ctrl+Y act on this tab's edits (Onshape).
        self.doc.update(cx, |d, _| d.session.active_tab = Some(tab));
        if let Some((tm, _)) = &self.tab_manager {
            tm.update(cx, |t, cx| t.set_active(Some(tab), cx));
        }
        self.ensure_view(tab, window, cx);
        if let Some(TabView::PartStudio(v)) = self.views.get(&tab) {
            let fh = v.read(cx).focus_handle(cx);
            window.focus(&fh, cx);
        }
        if let Some(TabView::Drawing(v)) = self.views.get(&tab) {
            let fh = v.read(cx).focus.clone();
            window.focus(&fh, cx);
        }
        if let Some(TabView::Assembly(v)) = self.views.get(&tab) {
            let fh = v.read(cx).focus_handle(cx);
            window.focus(&fh, cx);
        }
        if let Some(TabView::FeatureStudio(v)) = self.views.get(&tab) {
            let fh = v.read(cx).editor_focus(cx);
            window.focus(&fh, cx);
        }
        cx.notify();
    }

    fn add_tab(&mut self, kind: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let mut new_id = None;
        self.doc.update(cx, |doc, cx| {
            let (base, content) = match kind {
                "partstudio" => ("Part Studio", TabContent::PartStudio(PartStudio::new())),
                "assembly" => ("Assembly", TabContent::Assembly(Assembly::default())),
                "drawing" => ("Drawing", TabContent::Drawing(Drawing::default())),
                "featurestudio" => ("Feature Studio", TabContent::FeatureStudio(noshape_model::custom::FeatureStudio::template())),
                _ => ("Variable Studio", TabContent::VariableStudio(VariableStudio::default())),
            };
            doc.edit_document(&format!("Insert tab : {base}"), cx, |d| {
                let name = d.next_tab_name(base);
                new_id = Some(d.add_tab(&name, content));
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
        let name = self.doc.read(cx).session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
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
        self.doc.update(cx, |doc, cx| {
            doc.edit_document(&format!("Paste tab : {}", t.name), cx, |d| {
                let name = if d.tabs.iter().any(|x| x.name == t.name) { d.next_tab_name(&t.name) } else { t.name.clone() };
                new_id = Some(d.add_tab(&name, t.content.clone()));
            });
        });
        if let Some(id) = new_id {
            self.activate(id, window, cx);
        }
    }

    fn duplicate_tab(&mut self, tab: TabId, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            let Some(t) = doc.session.doc.tab(tab).cloned() else { return };
            doc.edit_document(&format!("Duplicate tab : {}", t.name), cx, |d| {
                let name = format!("{} (copy)", t.name);
                d.add_tab(&name, t.content);
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
        // The current branch (Onshape shows the workspace name, e.g. "Main").
        let branch_name = doc.session.store.branch(doc.session.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into());
        TitleBar::new()
            .h(px(crate::ui::NAVBAR_H))
            .bg(pal.panel_inactive)
            .border_b_0()
            .child(
                h_flex()
                    .w_full()
                    .h_full()
                    .pr_2()
                    .gap(px(4.))
                    .child(
                        // Onshape: the logo block is 139 px wide with 14 px left padding.
                        h_flex()
                            .id("navbar-logo")
                            .flex_none()
                            .w(px(139.))
                            .pl(px(14.))
                            .gap(px(6.))
                            .cursor_pointer()
                            // Onshape: the logo goes to the documents page.
                            .on_click(|_, _, cx| crate::files::show_start_window(cx))
                            .child(crate::assets::icon("noshape-logo", 24., cx))
                            .child(div().text_size(px(21.)).font_weight(FontWeight::NORMAL).text_color(gpui_kit::rgb(0x4a4a4a)).child("noshape")),
                    )
                    .child(self.render_doc_menu(cx))
                    .child(div().flex_none().px(px(4.)).py(px(2.)).text_size(px(22.)).font_weight(FontWeight::BOLD).text_color(pal.text).truncate().child(name))
                    .child(div().flex_none().ml(px(4.)).text_size(px(16.)).text_color(pal.text_faint).child(if read_only { "History".to_string() } else { branch_name }))
                    .when(read_only, |d| d.child(div().ml_2().px_2().rounded(px(2.)).bg(pal.warning).text_xs().text_color(pal.text).child("VIEWING HISTORY")))
                    .child(div().flex_1())
                    .child(self.render_help_menu(cx)),
            )
            .into_any_element()
    }

    /// Help ▾ at the right of the navbar (Onshape's `?▾`).
    fn render_help_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        Button::new("help-menu")
            .custom(navbar_variant(cx))
            .child(h_flex().gap(px(3.)).child(Icon::new(IconName::Info).with_size(px(18.)).text_color(pal.text)).child(crate::ui::glyph("ui-chevron-down", 9., pal.text)))
            .h(px(crate::ui::NAVBAR_H))
            .px(px(6.))
            .rounded(px(0.))
            .dropdown_menu(|menu, _, _| menu.menu("Keyboard shortcuts", Box::new(ToggleShortcutHelp)).separator().item(PopupMenuItem::new("About noshape").disabled(true)))
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
                let w = ws.clone();
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

    fn report(&self, msg: String, cx: &mut Context<Self>) {
        self.doc.update(cx, |_, cx| cx.emit(DocEvent::Error(msg)));
    }

    /// Tab › Export…: Onshape's Export modal (live, drawing/40 layout): File
    /// name, Format select; Export saves the file.
    fn open_export(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        use crate::part_studio::ExportFormat;
        self.activate(tab, window, cx);
        match self.views.get(&tab) {
            Some(TabView::Assembly(v)) => return v.update(cx, |v, cx| v.open_export_all(window, cx)),
            // Onshape's "Export Drawing" modal (drawing_export).
            Some(TabView::Drawing(v)) => return v.update(cx, |v, cx| v.open_export_dialog(window, cx)),
            _ => {}
        }
        let ws = cx.entity();
        let name = self.doc.read(cx).session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
        let file = cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).default_value(name));
        let chosen = std::rc::Rc::new(std::cell::Cell::new(0usize));
        window.open_dialog(cx, move |dialog, _, cx| {
            let (c, ws, file2) = (chosen.clone(), ws.clone(), file.clone());
            let formats: Vec<String> = ExportFormat::ALL.iter().map(|f| f.label().to_string()).collect();
            crate::os_ui::export_modal(dialog, "Export", &file, formats, chosen.clone(), cx)
                .on_ok(move |_, window, cx| {
                    let (ws, f) = (ws.clone(), ExportFormat::ALL[c.get()]);
                    let name = file2.read(cx).value().trim().to_string();
                    window.defer(cx, move |window, cx| ws.update(cx, |t, cx| t.export(f, Some(name), window, cx)));
                    true
                })
        });
    }

    fn export(&mut self, format: crate::part_studio::ExportFormat, file_name: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.active_studio() else {
            self.report("Open a Part Studio tab to export its parts".into(), cx);
            return;
        };
        let name = file_name.filter(|n| !n.is_empty()).unwrap_or_else(|| view.read(cx).studio_name(cx));
        let ws = cx.entity();
        crate::files::request_save_path("Export", format!("{name}.{}", format.ext()), format.ext(), window, cx, move |path, cx| {
            let r = view.read(cx).export(format, &path);
            let msg = match r {
                Ok(()) => format!("Exported {}", path.display()),
                Err(e) => format!("Export failed: {e:#}"),
            };
            ws.update(cx, |this, cx| this.report(msg, cx));
        });
    }

    /// "+" › Import…: Onshape translates the file into a new Part Studio tab
    /// named after the file and opens it.
    fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ws = cx.entity();
        let handle = window.window_handle();
        let units = self.doc.read(cx).session.doc.settings.units;
        crate::files::request_open_path("Import files", "CAD and mesh files", IMPORT_EXTS, window, cx, move |path, cx| {
            let brep = match read_foreign(&path, &units) {
                Ok(b) => b,
                Err(e) => return ws.update(cx, |this, cx| this.report(format!("Import failed: {e:#}"), cx)),
            };
            let (name, file) = import_names(&path);
            let mut new_id = None;
            ws.read(cx).doc.clone().update(cx, |doc, cx| {
                doc.edit_document(&format!("Import {file}"), cx, |d| {
                    let mut ps = PartStudio::new();
                    add_import_feature(&mut ps, &file, brep, &units);
                    let name = if d.tabs.iter().any(|t| t.name == name) { d.next_tab_name(&name) } else { name };
                    new_id = Some(d.add_tab(&name, TabContent::PartStudio(ps)));
                });
            });
            if let Some(id) = new_id {
                let _ = handle.update(cx, |_, window, cx| ws.update(cx, |this, cx| this.activate(id, window, cx)));
            }
        });
    }

    fn toggle_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.history.take().is_none() {
            let doc = self.doc.clone();
            self.history = Some(cx.new(|cx| HistoryPanel::new(doc, window, cx)));
        }
        cx.notify();
    }

    /// Onshape's bottom tab bar (§3.5): 30 px `#d7d7d7`; tab-list cell under
    /// the rail, a + cell, then tabs hanging from the top edge with rounded
    /// bottom corners; the active tab is white with a 2 px blue underline.
    fn render_tab_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let tabs: Vec<(TabId, String, &'static str)> = self.doc.read(cx).session.doc.ordered_tabs().iter().map(|t| (t.id, t.name.clone(), kind_of(&t.content))).collect();
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
        let tab_list = cell("tab-list", cx)
            .w(px(crate::ui::RAIL_W))
            .bg(pal.rail)
            .border_r_1()
            .border_color(pal.border)
            .child(crate::assets::icon("tab-search", 20., cx))
            .tooltip("Tab manager (Alt+T)")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_tab_manager(window, cx)));
        // Onshape's "+" menu (live, document/01): a Bootstrap dropdown opening
        // upward from the cell. Cloud items (Applications, CAM / PCB /
        // Render Studio) are not part of noshape.
        let add = cell("add-tab", cx).w(px(34.)).child(crate::ui::glyph("plus", 20., gpui_kit::rgb(0x555555))).tooltip("Insert new tab").on_click(move |_, window, cx| {
            let (v1, v2, v3, v4, v5, v6, v7) = (view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone());
            let copied = cx.try_global::<crate::doc_dialogs::CopiedTab>().and_then(|c| c.0.as_ref().map(|t| t.name.clone()));
            let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                use crate::os_menu::OsMenuItem as I;
                let m = m.dropdown();
                let m = match &copied {
                    Some(n) => m.item(I::new(format!("Paste {n}")).icon_name("paste").on_click(move |_, w, cx| v7.update(cx, |t, cx| t.paste_tab(w, cx)))).separator(),
                    None => m,
                };
                m
                    .item(I::new("Create Feature Studio").icon_name("feature-studio").on_click(move |_, w, cx| v5.update(cx, |t, cx| t.add_tab("featurestudio", w, cx))))
                    .separator()
                    .item(I::new("Create Part Studio").icon_name("part-studio").on_click(move |_, w, cx| v1.update(cx, |t, cx| t.add_tab("partstudio", w, cx))))
                    .item(I::new("Create Assembly").icon_name("assembly").on_click(move |_, w, cx| v2.update(cx, |t, cx| t.add_tab("assembly", w, cx))))
                    .item(I::new("Create Variable Studio").icon_name("variable-studio").on_click(move |_, w, cx| v4.update(cx, |t, cx| t.add_tab("variables", w, cx))))
                    .item(I::new("Create Drawing…").icon_name("drawing").on_click(move |_, w, cx| v3.update(cx, |t, cx| t.open_create_drawing(crate::drawing_create::CreateRequest { source: None, name: None }, w, cx))))
                    .item(I::new("Import…").icon_name("import").on_click(move |_, w, cx| v6.update(cx, |t, cx| t.import(w, cx))))
            });
            let h = window.viewport_size().height;
            crate::os_menu::open_anchored(point(px(crate::ui::RAIL_W), h - px(crate::ui::TAB_BAR_H)), Anchor::BottomLeft, menu, window, cx);
        });
        let mut strip = h_flex().id("tab-strip").flex_1().min_w_0().h_full().items_start().pl(px(5.)).gap(px(2.)).overflow_x_scroll();
        for (id, name, kind) in tabs {
            let active = self.active == Some(id);
            let view = cx.entity();
            let tab_name = name.clone();
            strip = strip.child(
                h_flex()
                    .id(SharedString::from(format!("tab-{id}")))
                    .relative()
                    .flex_none()
                    .min_w(px(60.))
                    .max_w(px(200.))
                    .h(px(if active { 28. } else { 27. }))
                    .pl(px(10.))
                    .pr(px(10.))
                    .gap(px(3.))
                    .rounded_b(px(5.))
                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.12), offset: point(px(0.), px(1.)), blur_radius: px(2.), spread_radius: px(0.), inset: false }])
                    .text_size(px(14.))
                    .cursor_pointer()
                    .bg(if active { pal.panel } else { pal.tab_inactive })
                    .text_color(if active { pal.text } else { pal.text_muted })
                    .child(crate::assets::icon(tab_icon(kind), 20., cx))
                    .child(match self.renaming_tab.as_ref().filter(|r| r.0 == id) {
                        // Tab › Rename / double-click: the name becomes a field.
                        Some((_, st, _)) => div().w(px(150.)).child(gpui_kit::component::input::Input::new(st).h(px(22.)).text_size(px(14.))).into_any_element(),
                        None => div().truncate().child(name).into_any_element(),
                    })
                    .when(active, |d| d.child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(pal.accent)))
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        // The first click activated the tab; a double-click renames it
                        // (activating again would take the field's focus).
                        if e.click_count() >= 2 {
                            this.start_tab_rename(id, window, cx);
                        } else {
                            this.activate(id, window, cx);
                        }
                    }))
                    .os_context_menu(move |menu, _, _| {
                        use crate::doc_dialogs as dd;
                        use crate::os_menu::OsMenuItem as I;
                        let (v1, v2, v3) = (view.clone(), view.clone(), view.clone());
                        let (v5, v6, v7, v8, v9, v10, v11) = (view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone());
                        // Onshape (live): Delete | Rename, Properties…, Show code |
                        // Duplicate, Copy to clipboard | Select as document
                        // thumbnail | Move to document…, Export….
                        let menu = menu
                            .item(I::new("Delete").on_click(move |_, w, cx| v2.update(cx, |t, cx| t.delete_tab(id, w, cx))))
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
                            .separator()
                            .item(I::new("Select as document thumbnail").on_click(move |_, _, cx| v9.update(cx, |t, cx| t.select_thumbnail(id, cx))))
                            .separator()
                            .item(I::new("Move to document…").on_click(move |_, w, cx| {
                                let doc = v10.read(cx).doc.clone();
                                dd::move_to_document(doc, id, w, cx)
                            }));
                        let menu = if matches!(kind, "partstudio" | "assembly" | "drawing") { menu.item(I::new("Export…").on_click(move |_, w, cx| v11.update(cx, |t, cx| t.open_export(id, w, cx)))) } else { menu };
                        // Onshape: Part Studio and Assembly tabs offer "Create Drawing of <tab>…".
                        let menu = if matches!(kind, "partstudio" | "assembly") {
                            let n = tab_name.clone();
                            menu.item(crate::os_menu::OsMenuItem::new(format!("Create Drawing of {tab_name}…")).on_click(move |_, w, cx| {
                                let source = Some(noshape_model::drawing::create::DrawingSource { tab: id, part: None, config: Default::default() });
                                let req = crate::drawing_create::CreateRequest { source, name: Some(n.clone()) };
                                v3.update(cx, |t, cx| t.open_create_drawing(req, w, cx));
                            }))
                        } else {
                            menu
                        };
                        menu
                    }),
            );
        }
        h_flex().h(px(crate::ui::TAB_BAR_H)).flex_none().bg(pal.tab_bar).child(tab_list).child(add).child(strip).into_any_element()
    }

    /// Onshape's left document-panel rail (§3.3): 40 px `#fafafa`.
    fn render_rail(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let history_open = self.history.is_some();
        v_flex()
            .w(px(crate::ui::RAIL_W))
            .h_full()
            .flex_none()
            .pt(px(1.))
            .gap(px(4.))
            .bg(pal.rail)
            .border_r_1()
            .border_t_1()
            .border_color(pal.border)
            .child(
                crate::ui::rail_button("rail-history", "history", history_open, true, cx)
                    .tooltip(crate::ui::tooltip("Versions and history (Alt+H)"))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_history(window, cx))),
            )
            .child(
                crate::ui::rail_button("rail-version", "version", false, true, cx)
                    .tooltip(crate::ui::tooltip("Create version…"))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| crate::history_panel::open_create_version(this.doc.clone(), window, cx))),
            )
            .into_any_element()
    }
}

fn tab_icon(kind: &str) -> &'static str {
    match kind {
        "partstudio" => "part-studio",
        "assembly" => "assembly",
        "drawing" => "drawing",
        "featurestudio" => "feature-studio",
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
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        crate::assets::set_scale_factor(_window.scale_factor());
        let title = self.render_title_bar(cx);
        let rail = self.render_rail(cx);
        let tabs = self.render_tab_bar(cx);
        let content: AnyElement = match self.active.and_then(|a| self.views.get(&a)) {
            Some(TabView::PartStudio(v)) => v.clone().into_any_element(),
            Some(TabView::Drawing(v)) => v.clone().into_any_element(),
            Some(TabView::Variables(v)) => v.clone().into_any_element(),
            Some(TabView::Assembly(v)) => v.clone().into_any_element(),
            Some(TabView::FeatureStudio(v)) => v.clone().into_any_element(),
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
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
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
                        None => self.history.clone().map(|h| h.into_any_element()),
                    }.map(|panel| div().w(px(crate::history_panel::PANEL_W)).h_full().flex_none().border_r_1().border_color(pal.border).child(panel)))
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .relative()
                            .child(content)
                            // Onshape: at the feature dialog's place (below the toolbar, right of the list).
                            .children(self.units_panel.as_ref().map(|(p, _)| div().absolute().top(px(39.)).left(px(crate::ui::LEFT_PANEL_W)).child(p.clone()))),
                    ),
            )
            .child(tabs)
            .children(self.create_drawing.as_ref().map(|(d, _)| d.clone()))
            // Onshape right-click menus opened with `os_context_menu`.
            .children(crate::os_menu::render_host(cx))
    }
}

/// Tab to activate, set by views that are not subscribed to by the
/// workspace (e.g. a drawing view's "Switch to <Part Studio>").
static REQUESTED_TAB: parking_lot::Mutex<Option<TabId>> = parking_lot::Mutex::new(None);

/// Ask the workspace to switch to `tab`.
pub fn request_activate_tab(tab: TabId, window: &mut Window, cx: &mut App) {
    *REQUESTED_TAB.lock() = Some(tab);
    window.dispatch_action(Box::new(ActivateRequestedTab), cx);
}

impl Workspace {
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
                    noshape_model::drawing::create::create_drawing(&template, src.as_ref(), bounds, suggested)
                })
                .await;
            let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                let mut new_id = None;
                this.doc.update(cx, |doc, cx| {
                    doc.edit_document(&format!("Insert tab : {name}"), cx, |d| {
                        let name = if name.is_empty() || d.tabs.iter().any(|t| t.name == name) { d.next_tab_name("Drawing") } else { name.clone() };
                        let id = d.add_tab(&name, TabContent::Drawing(drawing));
                        // The new drawing tab lands right after its source tab (Onshape).
                        if let Some(src) = source.as_ref().map(|s| s.tab)
                            && let Some(at) = d.settings.tab_order.iter().position(|t| *t == src)
                        {
                            d.settings.tab_order.retain(|t| *t != id);
                            d.settings.tab_order.insert(at + 1, id);
                        }
                        new_id = Some(id);
                    });
                });
                let Some(id) = new_id else { return };
                this.activate(id, window, cx);
                if let (false, Some(TabView::Drawing(v))) = (suggested, this.views.get(&id)) {
                    match &source {
                        Some(s) => {
                            let tool = crate::drawing_view::Tool::InsertView { source: s.tab, orientation: noshape_model::drawing::ViewOrientation::Front, part: s.part.clone() };
                            v.update(cx, |v, cx| v.set_tool(tool, cx));
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
            });
            self.tab_manager = Some((tm, sub));
        }
        cx.notify();
    }
}


/// File types "Import files…" translates.
pub(crate) const IMPORT_EXTS: &[&str] = &["step", "stp", "iges", "igs", "brep", "stl", "obj"];

/// Tab (and document) name of an imported file: the file name without its
/// extension; and the full file name.
pub(crate) fn import_names(path: &std::path::Path) -> (String, String) {
    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let stem = path.file_stem().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| file.clone());
    (stem, file)
}

/// Read a CAD or mesh file as BREP text. Mesh files carry no unit: they are
/// read in the document's length unit.
pub(crate) fn read_foreign(path: &std::path::Path, units: &noshape_units::UnitPrefs) -> anyhow::Result<String> {
    use noshape_kernel as k;
    let mesh_unit = {
        use noshape_units::LengthUnit as U;
        match units.length {
            U::Centimeter => k::LengthUnit::Centimeter,
            U::Meter => k::LengthUnit::Meter,
            U::Inch => k::LengthUnit::Inch,
            U::Foot | U::Yard => k::LengthUnit::Foot,
            _ => k::LengthUnit::Millimeter,
        }
    };
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let shape = match ext.as_str() {
        "step" | "stp" => {
            let parts = k::read_step(path, k::LengthUnit::Meter)?;
            k::Shape::compound(&parts.into_iter().map(|p| p.shape).collect::<Vec<_>>())
        }
        "iges" | "igs" => k::read_iges(path, k::LengthUnit::Meter)?,
        "brep" => k::read_brep(path)?,
        // Small meshes get coplanar triangles merged into faces.
        "stl" | "obj" => k::read_mesh(path, mesh_unit, 20_000)?,
        other => anyhow::bail!("Unsupported file type .{other}"),
    };
    Ok(String::from_utf8_lossy(&shape.to_brep_bytes()?).to_string())
}

/// Append the "Import <file>" feature holding `brep` to `ps`.
pub(crate) fn add_import_feature(ps: &mut noshape_model::PartStudio, file: &str, brep: String, units: &noshape_units::UnitPrefs) {
    if let Some(mut f) = ps.new_feature("importForeign", units) {
        f.name = format!("Import {file}");
        f.custom_name = true;
        f.params.set("blobData", noshape_model::ParamValue::Text(brep));
        f.params.set("foreignId", noshape_model::ParamValue::Text(file.to_string()));
        ps.insert_at_rollback(f);
    }
}
