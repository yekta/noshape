//! Document window: title bar, active tab content and the bottom tab bar.

use crate::actions::*;
use crate::assembly_view::AssemblyView;
use crate::assets::icon_path;
use crate::doc_model::{DocEvent, DocModel};
use crate::gpu::Gpu;
use crate::history_panel::HistoryPanel;
use crate::part_studio::PartStudioView;
use crate::variable_studio::VariableStudioView;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, IconName, Sizable as _, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
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
        let mut this = Self { doc, gpu, active, views: HashMap::new(), focus: cx.focus_handle(), history: None, recent: Vec::new(), tab_manager: None, create_drawing: None, _thumbnailer: thumbnailer, _subs: subs };
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

    fn delete_tab(&mut self, tab: TabId, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            let name = doc.session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
            if doc.session.doc.tabs.len() > 1 {
                doc.edit_document(&format!("Delete tab : {name}"), cx, |d| {
                    d.remove_tab(tab);
                });
            }
        });
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
                            .flex_none()
                            .w(px(139.))
                            .pl(px(14.))
                            .gap(px(6.))
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

    fn render_doc_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let ws = cx.entity();
        let recent = crate::files::recent();
        let units = self.doc.read(cx).session.doc.settings.units.length;
        Button::new("doc-menu")
            .custom(navbar_variant(cx))
            .child(crate::assets::icon("menu", 20., cx))
            .tooltip("Document menu")
            .w(px(32.))
            .h(px(crate::ui::NAVBAR_H))
            .p_0()
            .rounded(px(0.))
            .dropdown_menu(move |menu, window, cx| {
                let w = ws.clone();
                let mut menu = menu
                    .item(PopupMenuItem::new("Documents").icon(IconName::LayoutDashboard).on_click(|_, _, cx| crate::files::show_start_window(cx)))
                    .separator()
                    .menu("New document", Box::new(NewDocument))
                    .menu("Open…", Box::new(OpenDocument))
                    .submenu("Open recent", window, cx, {
                        let recent = recent.clone();
                        move |mut m, _, _| {
                            if recent.is_empty() {
                                m = m.label("No recent documents");
                            }
                            for r in &recent {
                                let path = r.clone();
                                let label = r.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                                m = m.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                                    let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
                                    let _ = crate::files::open_document_window(path.clone(), None, gpu, cx);
                                }));
                            }
                            m
                        }
                    })
                    .separator();
                let w1 = w.clone();
                menu = menu.item(PopupMenuItem::new("Save a copy as…").on_click(move |_, window, cx| w1.update(cx, |t, cx| t.save_copy(window, cx))));
                let w2 = w.clone();
                menu = menu.submenu("Export active Part Studio", window, cx, move |mut m, _, _| {
                    for f in crate::part_studio::ExportFormat::ALL {
                        let w = w2.clone();
                        m = m.item(PopupMenuItem::new(f.label()).on_click(move |_, window, cx| w.update(cx, |t, cx| t.export(f, window, cx))));
                    }
                    m
                });
                let w4 = w.clone();
                menu = menu.item(PopupMenuItem::new("Import into Part Studio…").on_click(move |_, window, cx| w4.update(cx, |t, cx| t.import(window, cx))));
                let w3 = w.clone();
                menu = menu.separator().submenu("Units", window, cx, move |mut m, _, _| {
                    for u in noshape_units::LengthUnit::ALL {
                        let w = w3.clone();
                        m = m.item(PopupMenuItem::new(format!("{u:?}")).checked(u == units).on_click(move |_, _, cx| w.update(cx, |t, cx| t.set_units(u, cx))));
                    }
                    m
                });
                menu.separator().menu("Close window", Box::new(CloseWindow))
            })
            .into_any_element()
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

    fn export(&mut self, format: crate::part_studio::ExportFormat, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.active_studio() else {
            self.report("Open a Part Studio tab to export its parts".into(), cx);
            return;
        };
        let name = view.read(cx).studio_name(cx);
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

    fn save_copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.doc.read(cx).name();
        let ws = cx.entity();
        crate::files::request_save_path("Save a copy", format!("{name} copy.noshape"), "noshape", window, cx, move |path, cx| {
            ws.update(cx, |this, cx| {
                let r = this.doc.read(cx).session.store.backup_to(&path);
                let msg = match r {
                    Ok(()) => format!("Saved a copy to {}", path.display()),
                    Err(e) => format!("Save failed: {e}"),
                };
                this.report(msg, cx);
            });
        });
    }

    fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.active_studio() else {
            self.report("Open a Part Studio tab to import into".into(), cx);
            return;
        };
        let ws = cx.entity();
        let tab = view.read(cx).tab;
        // Mesh files carry no unit: read them in the document's length unit.
        let mesh_unit = {
            use noshape_kernel::LengthUnit as K;
            use noshape_units::LengthUnit as U;
            match self.doc.read(cx).session.doc.settings.units.length {
                U::Centimeter => K::Centimeter,
                U::Meter => K::Meter,
                U::Inch => K::Inch,
                U::Foot | U::Yard => K::Foot,
                _ => K::Millimeter,
            }
        };
        crate::files::request_open_path("Import CAD file", "CAD and mesh files", &["step", "stp", "iges", "igs", "brep", "stl", "obj"], window, cx, move |path, cx| {
            let result = (|| -> anyhow::Result<String> {
                use noshape_kernel as k;
                let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                let shape = match ext.as_str() {
                    "step" | "stp" => {
                        let parts = k::read_step(&path, k::LengthUnit::Meter)?;
                        k::Shape::compound(&parts.into_iter().map(|p| p.shape).collect::<Vec<_>>())
                    }
                    "iges" | "igs" => k::read_iges(&path, k::LengthUnit::Meter)?,
                    "brep" => k::read_brep(&path)?,
                    // Small meshes get coplanar triangles merged into faces.
                    "stl" | "obj" => k::read_mesh(&path, mesh_unit, 20_000)?,
                    other => anyhow::bail!("Unsupported file type .{other}"),
                };
                Ok(String::from_utf8_lossy(&shape.to_brep_bytes()?).to_string())
            })();
            ws.update(cx, |this, cx| match result {
                Ok(brep) => {
                    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                    let units = this.doc.read(cx).session.doc.settings.units;
                    this.doc.update(cx, |doc, cx| {
                        doc.edit_studio(tab, &format!("Import {file}"), cx, |ps| {
                            if let Some(mut f) = ps.new_feature("importForeign", &units) {
                                f.name = format!("Import {file}");
                                f.custom_name = true;
                                f.params.set("blobData", noshape_model::ParamValue::Text(brep));
                                f.params.set("foreignId", noshape_model::ParamValue::Text(file.clone()));
                                ps.insert_at_rollback(f);
                            }
                        });
                    });
                }
                Err(e) => this.report(format!("Import failed: {e:#}"), cx),
            });
        });
    }

    fn set_units(&mut self, u: noshape_units::LengthUnit, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.session.doc.settings.units.length = u;
            if let Err(e) = doc.session.commit(&[], true, "Change workspace units") {
                cx.emit(DocEvent::Error(format!("{e:#}")));
            }
            cx.emit(DocEvent::Reloaded);
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
        let add = cell("add-tab", cx).w(px(34.)).child(crate::ui::glyph("plus", 20., gpui_kit::rgb(0x555555))).tooltip("Insert new tab").dropdown_menu(move |menu, _, _| {
            let v1 = view.clone();
            let v2 = view.clone();
            let v3 = view.clone();
            let v4 = view.clone();
            let v5 = view.clone();
            menu.item(PopupMenuItem::new("Part Studio").icon(Icon::empty().path(icon_path("part-studio"))).on_click(move |_, w, cx| v1.update(cx, |t, cx| t.add_tab("partstudio", w, cx))))
                .item(PopupMenuItem::new("Assembly").icon(Icon::empty().path(icon_path("assembly"))).on_click(move |_, w, cx| v2.update(cx, |t, cx| t.add_tab("assembly", w, cx))))
                .item(PopupMenuItem::new("Create Drawing").icon(Icon::empty().path(icon_path("drawing"))).on_click(move |_, w, cx| v3.update(cx, |t, cx| t.open_create_drawing(crate::drawing_create::CreateRequest { source: None, name: None }, w, cx))))
                .item(PopupMenuItem::new("Variable Studio").icon(Icon::empty().path(icon_path("variable-studio"))).on_click(move |_, w, cx| v4.update(cx, |t, cx| t.add_tab("variables", w, cx))))
                .item(PopupMenuItem::new("Feature Studio").icon(Icon::empty().path(icon_path("feature-studio"))).on_click(move |_, w, cx| v5.update(cx, |t, cx| t.add_tab("featurestudio", w, cx))))
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
                    .child(div().truncate().child(name))
                    .when(active, |d| d.child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(pal.accent)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.activate(id, window, cx)))
                    .context_menu(move |menu, _, _| {
                        let v1 = view.clone();
                        let v2 = view.clone();
                        let v3 = view.clone();
                        let menu = menu.item(PopupMenuItem::new("Duplicate").on_click(move |_, _, cx| v1.update(cx, |t, cx| t.duplicate_tab(id, cx))));
                        // Onshape: Part Studio and Assembly tabs offer "Create Drawing of <tab>…".
                        let menu = if matches!(kind, "partstudio" | "assembly") {
                            let n = tab_name.clone();
                            menu.item(PopupMenuItem::new(format!("Create Drawing of {tab_name}…")).on_click(move |_, w, cx| {
                                let source = Some(noshape_model::drawing::create::DrawingSource { tab: id, part: None, config: Default::default() });
                                let req = crate::drawing_create::CreateRequest { source, name: Some(n.clone()) };
                                v3.update(cx, |t, cx| t.open_create_drawing(req, w, cx));
                            }))
                        } else {
                            menu
                        };
                        menu.separator()
                            .item(PopupMenuItem::new("Delete").on_click(move |_, _, cx| v2.update(cx, |t, cx| t.delete_tab(id, cx))))
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
                    .child(div().flex_1().h_full().min_w_0().child(content)),
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
                if let (false, Some(s), Some(TabView::Drawing(v))) = (suggested, &source, this.views.get(&id)) {
                    let tool = crate::drawing_view::Tool::InsertView { source: s.tab, orientation: noshape_model::drawing::ViewOrientation::Front, part: s.part.clone() };
                    v.update(cx, |v, cx| v.set_tool(tool, cx));
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

