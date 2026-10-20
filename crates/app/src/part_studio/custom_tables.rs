//! Part Studio › Custom tables panel (live behaviours.md "Custom tables
//! (defineTable)", part-studio/632–647, dom/fs-tables-*): a table selector
//! ("My custom tables" / "Workspace custom tables") with ⋯ "Switch to
//! <Feature Studio>", then a collapsible section per table of the result
//! (⋯ "Copy table") with its grid: grey bold headers, 35 px read-only
//! rows, error cells pink with their tooltip, info cells with ⓘ, missing
//! values "Not set"; hovering a row pre-highlights its entities. A table
//! that throws shows [`TABLE_ERRORS`] and its notices go to the
//! FeatureScript notices pane. The footer's Add custom tables opens the
//! 300 px flyout listing the tables of a document version.

use super::PartStudioView;
use crate::custom_tables::{self as ct, TableRef};
use crate::derived::VersionChoice;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::custom::table::{TABLE_ERRORS, TableResult};
use noshape_model::document::TabId;
use noshape_model::reference::Reference;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

const BLUE: u32 = 0x1651b0;
const GRID: u32 = 0xaaaaaa;

/// What a result was computed for: the table, the regeneration context and
/// the Feature Studio's compiled version.
type Stamp = (TableRef, usize, u64);

/// Panel state (in the Part Studio's UI state).
#[derive(Default)]
pub struct CtState {
    /// The table shown (None: the first one offered).
    pub selected: Option<TableRef>,
    /// Collapsed sections (table titles).
    pub collapsed: HashSet<String>,
    /// The selector's list is open.
    pub list_open: bool,
    /// Entities of the hovered row (pre-highlighted in the graphics).
    pub hover: Option<Vec<Reference>>,
    result: Option<(Stamp, Arc<TableResult>)>,
    task: Option<(Stamp, Task<()>)>,
    pub add: Option<AddState>,
}

/// A document at a version in the Add custom tables flyout.
pub struct DocAt {
    path: PathBuf,
    name: String,
    versions: Vec<VersionChoice>,
    version: Option<usize>,
    /// (Feature Studio, name, its tables); None while loading.
    tables: Option<Vec<(TabId, String, Vec<TableRef>)>>,
    changes: usize,
}

/// The Add custom tables flyout (live 639–645).
pub struct AddState {
    other: bool,
    graph: bool,
    current: Option<DocAt>,
    /// Other documents: the open category and document.
    category: Option<usize>,
    doc: Option<DocAt>,
    search: Entity<InputState>,
    collapsed: HashSet<TabId>,
}

/// Other documents' categories (live 639).
const CATEGORIES: [(&str, &str); 6] = [("Custom table samples", "featurescript-table"), ("Owned by me", "src-owned"), ("Recently opened", "src-recent"), ("Created by me", "src-created"), ("Shared with me", "src-shared"), ("Public", "src-public")];

fn ctx_key(c: &Arc<noshape_model::regen::context::Context>) -> usize {
    Arc::as_ptr(c) as usize
}

impl PartStudioView {
    /// (My custom tables, Workspace custom tables).
    fn offered_tables(&self, cx: &App) -> (Vec<TableRef>, Vec<TableRef>) {
        (ct::my_tables(), ct::workspace_tables(&self.doc.read(cx).session.doc))
    }

    /// The table the panel shows.
    fn shown_table(&self, cx: &App) -> Option<TableRef> {
        let (mine, ws) = self.offered_tables(cx);
        let all: Vec<TableRef> = mine.into_iter().chain(ws).collect();
        self.ui.ct.selected.clone().filter(|s| all.contains(s)).or_else(|| all.into_iter().next())
    }

    /// Run the shown table when the model or the Feature Studio changed.
    fn ensure_table(&mut self, t: &TableRef, cx: &mut Context<Self>) {
        let Some(ctx) = self.context.clone() else { return };
        let fs_version = match &t.version {
            None => noshape_model::custom::CustomFeatureProvider::version(&*self.doc.read(cx).custom_features(), &t.fs.to_string()),
            Some(_) => 0,
        };
        let stamp: Stamp = (t.clone(), ctx_key(&ctx), fs_version);
        if self.ui.ct.result.as_ref().is_some_and(|(s, _)| *s == stamp) || self.ui.ct.task.as_ref().is_some_and(|(s, _)| *s == stamp) {
            return;
        }
        let doc = self.doc.read(cx).session.doc.clone();
        let (t2, s2) = (t.clone(), stamp.clone());
        let task = cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move {
                    match ct::source_document(&t2, &doc) {
                        Some(d) => ct::evaluate(&t2, &d, &ctx),
                        None => TableResult { tables: Err("The document version defining this table could not be opened".into()), output: String::new(), notices: Vec::new() },
                    }
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.ui.ct.result = Some((s2, Arc::new(r)));
                this.ui.ct.task = None;
                this.publish_table_notices(cx);
                cx.notify();
            });
        });
        self.ui.ct.task = Some((stamp, task));
    }

    /// The notices pane's group "<table> (<Part Studio>)" of a table that
    /// threw (live 652).
    fn publish_table_notices(&self, cx: &mut Context<Self>) {
        let doc = self.doc.read(cx);
        let studio = doc.session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let group = self.ui.ct.result.as_ref().filter(|(_, r)| r.tables.is_err()).map(|((t, _, _), r)| ct::NoticeGroup { title: format!("{} ({studio})", t.display), entries: noshape_model::custom::run::notice_entries(&doc.session.doc, &r.notices, &format!("{studio} (main)")), output: r.output.clone() });
        let id = self.doc.entity_id();
        ct::set_notices(id, self.tab, group, cx);
    }

    /// The panel body when there are tables to show (else None: the
    /// empty-state help).
    pub(super) fn custom_tables_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let shown = self.shown_table(cx)?;
        self.ensure_table(&shown, cx);
        let pal = p();
        let (mine, ws) = self.offered_tables(cx);
        let fs_name = shown.fs_name.clone();
        let fs_tab = shown.fs;
        let workspace_fs = shown.version.is_none();
        // The selector (live: 220 × 33, #aaa border, badge + name ▾) and ⋯.
        let selector = h_flex()
            .id("ct-selector")
            .w(px(220.))
            .h(px(33.))
            .px(px(5.))
            .gap(px(4.))
            .rounded(px(3.))
            .border_1()
            .border_color(gpui_kit::rgb(GRID))
            .cursor_pointer()
            .child(ct::badge(&shown.display))
            .child(div().flex_1().min_w_0().truncate().text_size(px(14.)).text_color(pal.text).child(shown.display.clone()))
            .child(crate::os_ui::glyph(crate::os_ui::GLYPH_EXPANDED, 10., pal.text))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.ui.ct.list_open = !this.ui.ct.list_open;
                cx.notify();
            }));
        let more = div()
            .id("ct-more")
            .w(px(42.))
            .h(px(33.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.5))
            .border_1()
            .border_color(gpui_kit::rgb(GRID))
            .cursor_pointer()
            .child(crate::os_ui::glyph(crate::os_ui::GLYPH_OVERFLOW, 20., pal.text))
            .on_click(move |e: &ClickEvent, window, cx| {
                let label = format!("Switch to {fs_name}");
                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                    m.item(crate::os_menu::OsMenuItem::new(label.clone()).disabled(!workspace_fs).on_click(move |_, w, cx| crate::workspace::request_activate_tab(fs_tab, w, cx)))
                });
                crate::os_menu::open(e.position(), menu, window, cx);
            });
        let mut toolbar = h_flex().h(px(41.)).flex_none().gap(px(1.)).relative().child(selector).child(more);
        if self.ui.ct.list_open {
            let ed = cx.entity();
            let list = ct::table_list("ct-list", &mine, &ws, Some(&shown), 218., move |t, _, cx| {
                ed.update(cx, |this, cx| {
                    this.ui.ct.selected = Some(t);
                    this.ui.ct.list_open = false;
                    cx.notify();
                })
            });
            toolbar = toolbar.child(
                deferred(div().absolute().top(px(37.)).left(px(1.)).child(list).on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.ui.ct.list_open = false;
                    cx.notify();
                })))
                .with_priority(1),
            );
        }
        let body = match self.ui.ct.result.as_ref().filter(|((t, _, _), _)| *t == shown).map(|(_, r)| r.clone()) {
            None => div().p(px(8.)).text_size(px(13.33)).text_color(pal.text_muted).child("Loading…").into_any_element(),
            Some(r) => match &r.tables {
                Err(_) => div().p(px(8.)).text_size(px(14.)).text_color(pal.text).child(TABLE_ERRORS).into_any_element(),
                Ok(tables) => v_flex().children(tables.iter().enumerate().map(|(i, t)| self.table_section(i, t, cx))).into_any_element(),
            },
        };
        Some(v_flex().mx(px(-5.)).mt(px(-5.)).child(toolbar).child(body).into_any_element())
    }

    /// One table: the grey header bar (⌄ title, ⋯ Copy table), then the grid.
    fn table_section(&self, index: usize, t: &noshape_model::custom::table::CustomTable, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let open = !self.ui.ct.collapsed.contains(&t.title);
        let title = t.title.clone();
        let tsv = table_tsv(t);
        let header = h_flex()
            .id(("ct-section", index))
            .h(px(35.))
            .px(px(6.))
            .gap(px(4.))
            .bg(gpui_kit::rgb(0xeaeaea))
            .border_t_1()
            .border_b_1()
            .border_l_1()
            .border_color(gpui_kit::rgb(GRID))
            .cursor_pointer()
            .child(crate::os_ui::glyph(if open { crate::os_ui::GLYPH_EXPANDED } else { crate::os_ui::GLYPH_COLLAPSED }, 10., pal.text))
            .child(div().flex_1().min_w_0().truncate().text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(t.title.clone()).children(t.title_suffix.clone().map(|s| format!(" {s}"))))
            .child(
                div()
                    .id(("ct-section-more", index))
                    .child(crate::os_ui::glyph(crate::os_ui::GLYPH_OVERFLOW, 20., gpui_kit::rgb(0x094174)))
                    .on_click(move |e: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        let tsv = tsv.clone();
                        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| m.item(crate::os_menu::OsMenuItem::new("Copy table").on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(tsv.clone())))));
                        crate::os_menu::open(e.position(), menu, window, cx);
                    }),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if !this.ui.ct.collapsed.remove(&title) {
                    this.ui.ct.collapsed.insert(title.clone());
                }
                cx.notify();
            }));
        let mut section = v_flex().child(header);
        if !open {
            return section.into_any_element();
        }
        let grid = gpui_kit::rgb(GRID);
        // Live: 140 px columns + their 1 px right border, a 1 px left border per row.
        let cell = |content: AnyElement| div().w(px(140.)).h_full().flex_none().border_r_1().border_b_1().border_color(grid).child(content);
        let head = h_flex().h(px(34.)).bg(gpui_kit::rgb(0xeaeaea)).border_l_1().border_color(grid).children(t.columns.iter().map(|c| {
            cell(div().size_full().flex().items_center().justify_center().px(px(3.)).text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child(div().truncate().child(c.name.clone())).into_any_element())
        }));
        section = section.child(h_flex().child(head));
        for (ri, row) in t.rows.iter().enumerate() {
            let entities = row.entities.clone();
            let mut r = h_flex()
                .id(SharedString::from(format!("ct-row-{index}-{ri}")))
                .h(px(35.))
                .border_l_1()
                .border_color(grid)
                .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                    let new = (*h && !entities.is_empty()).then(|| entities.clone());
                    if this.ui.ct.hover != new && (*h || this.ui.ct.hover.is_some()) {
                        this.ui.ct.hover = new;
                        this.refresh_scene(false, cx);
                        cx.notify();
                    }
                }));
            for (ci, c) in t.columns.iter().enumerate() {
                let value = row.cell(&c.id);
                let text = row.text(&c.id).to_string();
                let right = c.alignment.as_deref() == Some("RIGHT");
                let center = c.alignment.as_deref() == Some("CENTER");
                let error = value.and_then(|v| v.error.clone());
                let info = value.and_then(|v| v.info.clone());
                let content = h_flex()
                    .id(SharedString::from(format!("ct-cell-{index}-{ri}-{ci}")))
                    .size_full()
                    .px(px(7.))
                    .bg(if error.is_some() { gpui_kit::rgb(0xf9ebec) } else { gpui_kit::rgb(0xf5f5f5) })
                    .when(right, |d| d.justify_end())
                    .when(center, |d| d.justify_center())
                    .text_size(px(13.33))
                    .text_color(pal.text)
                    .child(div().truncate().child(text))
                    .when_some(info, |d, tip| d.child(div().id(SharedString::from(format!("ct-info-{index}-{ri}-{ci}"))).pl(px(3.)).tooltip(crate::ui::tooltip(tip)).child(crate::ui::glyph("info", 14., gpui_kit::rgb(0x888888)))))
                    .when_some(error, |d, tip| d.tooltip(crate::ui::tooltip(tip)));
                r = r.child(cell(content.into_any_element()));
            }
            section = section.child(r);
        }
        section.into_any_element()
    }

    /// The footer's Add custom tables (shown with tables; live 632).
    pub(super) fn custom_tables_footer(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.shown_table(cx)?;
        Some(self.add_custom_tables_button(cx))
    }

    /// The Add custom tables button (live: white, #aaa border, radius
    /// 5.25, icon + 14 px label).
    pub(super) fn add_custom_tables_button(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        h_flex()
            .id("add-custom-tables")
            .h(px(33.))
            .px(px(10.))
            .gap(px(3.))
            .rounded(px(5.25))
            .bg(gpui_kit::white())
            .border_1()
            .border_color(gpui_kit::rgb(GRID))
            .cursor_pointer()
            .hover(|d| d.bg(pal.panel_hover))
            .child(crate::assets::icon("featurescript-table", 20., cx))
            .child(div().text_size(px(14.)).text_color(pal.text).child("Add custom tables"))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_add_tables(window, cx)))
            .into_any_element()
    }

    /// Open the flyout (live: on Other documents).
    fn open_add_tables(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search tables"));
        let path = self.doc.read(cx).session.path.clone();
        let current = doc_at(&path);
        self.ui.ct.add = Some(AddState { other: true, graph: false, current, category: None, doc: None, search, collapsed: HashSet::new() });
        self.load_version_tables(false, cx);
        cx.notify();
    }

    /// Load the tables of the chosen document's chosen version.
    fn load_version_tables(&mut self, other: bool, cx: &mut Context<Self>) {
        let Some(add) = self.ui.ct.add.as_mut() else { return };
        let Some(d) = (if other { add.doc.as_mut() } else { add.current.as_mut() }) else { return };
        let Some(v) = d.version.and_then(|i| d.versions.get(i)).cloned() else {
            d.tables = Some(Vec::new());
            return;
        };
        d.tables = None;
        let path = d.path.clone();
        let (mv, label) = (v.microversion, v.label);
        cx.spawn(async move |this, cx| {
            let p2 = path.clone();
            let (tables, changes) = cx
                .background_spawn(async move {
                    let tables = crate::derived::external_snapshot(&p2, mv).map(|doc| ct::version_tables(&doc, &p2, mv, &label)).unwrap_or_default();
                    (tables, crate::fs_reference::changes_since(&p2, mv))
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                if let Some(add) = this.ui.ct.add.as_mut()
                    && let Some(d) = if other { add.doc.as_mut() } else { add.current.as_mut() }
                    && d.path == path
                    && d.version.and_then(|i| d.versions.get(i)).is_some_and(|x| x.microversion == mv)
                {
                    d.tables = Some(tables);
                    d.changes = changes;
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The Add custom tables flyout over the panel's right edge (live 639–645).
    pub(super) fn render_add_tables(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let add = self.ui.ct.add.as_ref()?;
        let pal = p();
        let blue = gpui_kit::rgb(BLUE);
        let other = add.other;
        let tab = |id: &'static str, label: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex_1()
                .h_full()
                .p(px(5.))
                .flex()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .when(on, |d| d.text_color(blue).font_weight(FontWeight::MEDIUM).border_b_2().border_color(blue))
                .when(!on, |d| d.text_color(p().text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(a) = this.ui.ct.add.as_mut() {
                        a.other = id == "cta-other";
                        a.graph = false;
                    }
                    cx.notify();
                }))
        };
        let shown = if other { add.doc.as_ref() } else { add.current.as_ref() };
        let body: AnyElement = match shown {
            None if other => self.render_categories(cx),
            None => div().p(px(10.)).text_size(px(11.)).font_weight(FontWeight::BOLD).child("No tables available.").into_any_element(),
            Some(d) => self.render_doc_tables(d, other, cx),
        };
        Some(
            v_flex()
                .id("cta-panel")
                .occlude()
                .absolute()
                .top_0()
                .right_0()
                .w(px(300.))
                .h(px(580.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(5.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .flex_none()
                        .bg(gpui_kit::white())
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd7d7d7))
                        .child(div().flex_1().pl(px(4.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Add custom tables"))
                        .child(div().id("cta-close").size(px(28.)).flex().items_center().justify_center().opacity(0.5).cursor_pointer().child(crate::os_ui::glyph(crate::os_ui::GLYPH_CLOSE, 14., pal.text)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ui.ct.add = None;
                            cx.notify();
                        }))),
                )
                .child(h_flex().h(px(32.)).flex_none().child(tab("cta-current", "Current document", !other, cx)).child(tab("cta-other", "Other documents", other, cx)))
                .child(v_flex().flex_1().min_h_0().child(body))
                .child(
                    h_flex()
                        .h(px(23.))
                        .flex_none()
                        .px(px(4.))
                        .border_t_1()
                        .border_color(gpui_kit::rgb(0xeeeeee))
                        .child(div().id("cta-learn").flex_1().text_size(px(13.)).text_color(gpui_kit::rgb(0x094174)).cursor_pointer().child("Learn about FeatureScript").on_click(|_, _, cx| cx.open_url("https://cad.onshape.com/FsDoc/")))
                        .child(div().size(px(12.)).rounded_full().bg(gpui_kit::rgb(GRID)).flex().items_center().justify_center().text_size(px(9.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::white()).child("?")),
                )
                .into_any_element(),
        )
    }

    /// Other documents: the category list, or a category's documents.
    fn render_categories(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(add) = self.ui.ct.add.as_ref() else { return div().into_any_element() };
        let mut list = v_flex().id("cta-categories").flex_1().overflow_y_scroll().pt(px(4.));
        match add.category {
            None => {
                for (i, (label, icon)) in CATEGORIES.iter().enumerate() {
                    list = list.child(
                        h_flex()
                            .id(("cta-cat", i))
                            .h(px(32.))
                            .px(px(14.))
                            .gap(px(6.))
                            .cursor_pointer()
                            .hover(|d| d.bg(gpui_kit::rgb(0xdef1fb)))
                            .child(crate::ui::glyph(icon, 18., pal.text))
                            .child(div().text_size(px(14.)).text_color(pal.text).child(*label))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(a) = this.ui.ct.add.as_mut() {
                                    a.category = Some(i);
                                }
                                cx.notify();
                            })),
                    );
                }
            }
            Some(c) => {
                list = list.child(
                    h_flex()
                        .id("cta-cat-back")
                        .h(px(32.))
                        .px(px(6.))
                        .gap(px(6.))
                        .cursor_pointer()
                        .child(crate::os_ui::glyph(crate::os_ui::GLYPH_BACK, 16., pal.text))
                        .child(div().text_size(px(14.)).font_weight(FontWeight::MEDIUM).child(CATEGORIES[c].0))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(a) = this.ui.ct.add.as_mut() {
                                a.category = None;
                            }
                            cx.notify();
                        })),
                );
                // Local documents are all owned and created by this user;
                // nothing is shared, public or a sample here.
                let docs = if matches!(c, 1..=3) { crate::variable_insert::other_documents(&self.doc.read(cx).session.path) } else { Vec::new() };
                if docs.is_empty() {
                    list = list.child(div().p(px(10.)).text_size(px(11.)).font_weight(FontWeight::BOLD).child("No documents available."));
                }
                for (i, path) in docs.into_iter().enumerate() {
                    let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    list = list.child(
                        h_flex()
                            .id(("cta-doc", i))
                            .h(px(36.))
                            .px(px(8.))
                            .gap(px(6.))
                            .cursor_pointer()
                            .hover(|d| d.bg(gpui_kit::rgb(0xdef1fb)))
                            .child(crate::os_ui::glyph(crate::os_ui::GLYPH_DOCUMENT, 18., pal.text))
                            .child(div().truncate().text_size(px(14.)).child(name))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(a) = this.ui.ct.add.as_mut() {
                                    a.doc = doc_at(&path);
                                }
                                this.load_version_tables(true, cx);
                                cx.notify();
                            })),
                    );
                }
            }
        }
        list.into_any_element()
    }

    /// A document at a version: its header (name, "⑂ V3", Create version…,
    /// Version graph), then the version graph or the search box, the blue
    /// box when Main has later changes, and the Feature Studios' tables.
    fn render_doc_tables(&self, d: &DocAt, other: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let blue = gpui_kit::rgb(BLUE);
        let Some(add) = self.ui.ct.add.as_ref() else { return div().into_any_element() };
        let graph = add.graph;
        let version = d.version.and_then(|i| d.versions.get(i)).map_or_else(|| "Main".to_string(), |v| v.label.clone());
        let square = |id: &'static str, icon: &'static str, tip: &'static str, on: bool| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(gpui_kit::rgb(GRID))
                .cursor_pointer()
                .when(on, |d| d.bg(gpui_kit::rgb(0xeaeaea)))
                .tooltip(crate::ui::tooltip(tip))
                .child(crate::ui::glyph(icon, 20., pal.text))
        };
        let header = h_flex()
            .h(px(40.))
            .flex_none()
            .p(px(2.))
            .pl(px(if other { 2. } else { 6. }))
            .gap(px(6.))
            .border_t_1()
            .border_color(gpui_kit::rgb(0xeeeeee))
            .when(other, |h| {
                h.child(div().id("cta-home").size(px(30.)).flex().items_center().justify_center().cursor_pointer().child(crate::os_ui::glyph(crate::os_ui::GLYPH_HOME, 20., pal.text)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(a) = this.ui.ct.add.as_mut() {
                        a.doc = None;
                        a.category = None;
                        a.graph = false;
                    }
                    cx.notify();
                })))
                .child(div().id("cta-back").size(px(30.)).flex().items_center().justify_center().cursor_pointer().child(crate::os_ui::glyph(crate::os_ui::GLYPH_BACK, 16., pal.text)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(a) = this.ui.ct.add.as_mut() {
                        a.doc = None;
                        a.graph = false;
                    }
                    cx.notify();
                })))
            })
            .child(v_flex().flex_1().min_w_0().child(div().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).child(d.name.clone())).child(h_flex().gap_1().opacity(0.5).text_size(px(12.)).child(crate::ui::glyph("branch", 10., pal.text)).child(version)))
            .child(square("cta-create-version", "version", "Create version…", false).on_click(|_, window, cx| window.dispatch_action(Box::new(crate::actions::CreateVersion), cx)))
            .child(square("cta-version-graph", "version-graph", "Version graph", graph).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(a) = this.ui.ct.add.as_mut() {
                    a.graph = !a.graph;
                }
                cx.notify();
            })));
        let mut col = v_flex().flex_1().min_h_0().child(header);
        if graph {
            // Legend, then Main (italic grey, hollow, not selectable) and the versions.
            col = col.child(h_flex().h(px(30.)).px(px(6.)).gap(px(10.)).text_size(px(12.)).child("○ Workspace").child("● Version"));
            col = col.child(h_flex().h(px(36.)).child(div().w(px(140.)).flex().justify_center().child(div().size(px(13.)).rounded_full().border_2().border_color(blue).bg(gpui_kit::white()))).child(div().text_size(px(13.33)).italic().text_color(gpui_kit::rgb(0x888888)).child("Main")));
            for (i, v) in d.versions.iter().enumerate() {
                let on = d.version == Some(i);
                col = col.child(
                    h_flex()
                        .id(("cta-version", i))
                        .h(px(36.))
                        .cursor_pointer()
                        .when(on, |r| r.bg(gpui_kit::rgb(0xb8dcf8)))
                        .hover(|r| r.bg(gpui_kit::rgb(0xdef1fb)))
                        .child(div().w(px(140.)).h_full().relative().flex().items_center().justify_center().child(div().absolute().left(px(69.)).w(px(2.)).top_0().bottom_0().bg(blue)).child(div().size(px(9.)).rounded_full().bg(blue)))
                        .child(div().text_size(px(13.33)).child(v.label.clone()))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(a) = this.ui.ct.add.as_mut() {
                                a.graph = false;
                                if let Some(d) = if other { a.doc.as_mut() } else { a.current.as_mut() } {
                                    d.version = Some(i);
                                }
                            }
                            this.load_version_tables(other, cx);
                            cx.notify();
                        })),
                );
            }
            return col.into_any_element();
        }
        let query = add.search.read(cx).value().trim().to_lowercase();
        col = col.child(h_flex().h(px(38.)).flex_none().px(px(6.)).child(div().flex_1().h(px(28.)).rounded(px(3.)).border_1().border_color(gpui_kit::rgb(0xcccccc)).child(Input::new(&add.search).appearance(false).cleanable(true).text_size(px(12.)).h(px(26.)))));
        let mut rows = v_flex().id("cta-list").flex_1().min_h_0().overflow_y_scroll();
        if d.changes > 0 {
            let n = d.changes;
            rows = rows.child(
                v_flex()
                    .m(px(4.))
                    .p(px(4.))
                    .rounded(px(5.25))
                    .border_1()
                    .border_color(gpui_kit::rgb(0xb2ddf6))
                    .bg(gpui_kit::rgb(0xdef1fb))
                    .text_size(px(14.))
                    .text_color(gpui_kit::rgb(0x094174))
                    .child(format!("{n} change{} since the last version.", if n == 1 { "" } else { "s" }))
                    .child(div().id("cta-create-link").py(px(4.)).flex().justify_center().font_weight(FontWeight::SEMIBOLD).underline().cursor_pointer().child(format!("Create a version in {}.", d.name)).on_click(|_, window, cx| window.dispatch_action(Box::new(crate::actions::CreateVersion), cx))),
            );
        }
        let tables = d.tables.clone().unwrap_or_default();
        let mine = ct::my_tables();
        let mut any = false;
        for (fi, (fs, fs_name, list)) in tables.into_iter().enumerate() {
            let shown: Vec<TableRef> = list.into_iter().filter(|t| query.is_empty() || t.display.to_lowercase().contains(&query) || fs_name.to_lowercase().contains(&query)).collect();
            if shown.is_empty() {
                continue;
            }
            any = true;
            let open = !add.collapsed.contains(&fs);
            rows = rows.child(
                h_flex()
                    .id(("cta-fs", fi))
                    .h(px(40.))
                    .px(px(4.))
                    .gap(px(8.))
                    .cursor_pointer()
                    .child(crate::os_ui::glyph(if open { crate::os_ui::GLYPH_EXPANDED } else { crate::os_ui::GLYPH_COLLAPSED }, 10., pal.text))
                    .child(crate::assets::icon("feature-studio", 28., cx))
                    .child(div().text_size(px(13.33)).child(fs_name.clone()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(a) = this.ui.ct.add.as_mut()
                            && !a.collapsed.remove(&fs)
                        {
                            a.collapsed.insert(fs);
                        }
                        cx.notify();
                    })),
            );
            if !open {
                continue;
            }
            for (ti, t) in shown.into_iter().enumerate() {
                let added = mine.contains(&t);
                let display = t.display.clone();
                rows = rows.child(
                    h_flex()
                        .id(SharedString::from(format!("cta-table-{fi}-{ti}")))
                        .h(px(40.))
                        .pl(px(78.))
                        .gap(px(14.))
                        .cursor_pointer()
                        .hover(|r| r.bg(gpui_kit::rgb(0xdef1fb)))
                        .when(added, |r| r.opacity(0.5))
                        .tooltip(crate::ui::tooltip(display.clone()))
                        .child(ct::badge(&t.display))
                        .child(div().text_size(px(13.33)).child(display))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            // Live 645: the table joins My custom tables and is shown.
                            ct::add_my_table(t.clone());
                            this.ui.ct.selected = Some(t.clone());
                            this.ui.ct.add = None;
                            cx.notify();
                        })),
                );
            }
        }
        if !any && d.tables.is_some() {
            rows = rows.child(div().p(px(10.)).text_size(px(11.)).font_weight(FontWeight::BOLD).child("No tables available."));
        }
        col.child(rows).into_any_element()
    }
}

/// A document opened at its latest version.
fn doc_at(path: &std::path::Path) -> Option<DocAt> {
    let (name, versions) = crate::fs_reference::document_versions(path)?;
    let version = (!versions.is_empty()).then_some(0);
    Some(DocAt { path: path.to_path_buf(), name, versions, version, tables: None, changes: 0 })
}

/// The table as tab-separated text (⋯ › Copy table).
fn table_tsv(t: &noshape_model::custom::table::CustomTable) -> String {
    let mut out = t.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join("\t");
    for r in &t.rows {
        out.push('\n');
        out.push_str(&t.columns.iter().map(|c| r.text(&c.id)).collect::<Vec<_>>().join("\t"));
    }
    out
}
