//! Drawing editor: Onshape's Custom table (help Drawing__custom_table,
//! custom-tables-01..16): the "Custom table" dialog (Add + the Part Studio,
//! Insert + the custom table, Order, Select fixed corner; a click places
//! the table), its "Select a Part Studio" and "Select a custom table"
//! pickers, the "Custom table properties" dialog (Document, Workspace,
//! Custom table and Feature Studio links with the version, Sheet, Order,
//! fixed corner; ✓ / ✕) and the background evaluation of the drawing's
//! custom tables into [`SheetInfo::custom_tables`].

use crate::custom_tables::{self as ct, TableRef};
use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::document::TabId;
use noshape_model::drawing::tables::{CustomTableKey, custom_table_keys};
use noshape_model::drawing::{SheetLayer, Table, TableCorner, TableKind};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const CORNERS: [TableCorner; 4] = [TableCorner::TopLeft, TableCorner::TopRight, TableCorner::BottomLeft, TableCorner::BottomRight];

/// The picker open over the dialog.
#[derive(Clone, PartialEq)]
pub enum Picker {
    /// "Select a Part Studio".
    Studio,
    /// "Select a custom table": the table chosen in its dropdown so far.
    Table { pending: Option<TableRef>, list_open: bool },
}

/// The Custom table dialog (inserting) or Custom table properties (editing).
pub struct CustomDlg {
    /// The table edited (None while inserting).
    pub table: Option<u64>,
    original: Option<(TableKind, TableCorner)>,
    pub studio: Option<TabId>,
    pub chosen: Option<TableRef>,
    pub bottom_to_top: bool,
    pub corner: TableCorner,
    pub picker: Option<Picker>,
}

/// Custom tables of the drawing editor: the dialog and the evaluations.
#[derive(Default)]
pub struct CustomState {
    pub dlg: Option<CustomDlg>,
    running: HashSet<CustomTableKey>,
    /// Source and Feature Studio hashes of each evaluated table.
    stamps: HashMap<CustomTableKey, u64>,
}

/// The table a custom table kind shows.
fn table_ref(kind: &TableKind, doc: &noshape_model::Document) -> Option<TableRef> {
    let TableKind::Custom { fs, name, fs_version, .. } = kind else { return None };
    let ext = fs_version.as_deref().and_then(crate::derived::ExternalRef::parse);
    let snapshot = ext.as_ref().and_then(|e| crate::derived::external_snapshot(&e.path, e.microversion).ok());
    let fs_doc = snapshot.as_deref().unwrap_or(doc);
    let fs_name = fs_doc.tab(*fs).map(|t| t.name.clone()).unwrap_or_default();
    let display = match fs_doc.tab(*fs).map(|t| &t.content) {
        Some(TabContent::FeatureStudio(f)) => noshape_model::custom::compile(*fs, &f.source).table(name).map(|d| d.display_name.clone()),
        _ => None,
    }
    .unwrap_or_else(|| name.clone());
    let version_label = ext.and_then(|e| crate::derived::document_versions(&e.path).and_then(|(_, vs)| vs.into_iter().find(|v| v.microversion == e.microversion).map(|v| v.label)));
    Some(TableRef { fs: *fs, name: name.clone(), display, version: fs_version.clone(), version_label, fs_name })
}

impl DrawingEditor {
    /// Part Studios of the document (the "Select a Part Studio" list).
    fn part_studios(&self, cx: &App) -> Vec<(TabId, String)> {
        self.doc.read(cx).session.doc.ordered_tabs().into_iter().filter(|t| matches!(t.content, TabContent::PartStudio(_))).map(|t| (t.id, t.name.clone())).collect()
    }

    /// Part Studios shown in the drawing (the dialog's dropdown; live:
    /// "Part Studios already in the drawing appear in the dropdown"), plus
    /// the chosen one.
    fn drawing_studios(&self, chosen: Option<TabId>, cx: &App) -> Vec<(TabId, String)> {
        let keys = noshape_model::drawing::views::referenced_sources(&self.drawing);
        self.part_studios(cx).into_iter().filter(|(t, _)| keys.iter().any(|k| k.tab == *t) || Some(*t) == chosen).collect()
    }

    /// Create / drop the dialog with the Custom table tool (from render).
    pub fn sync_custom_dlg(&mut self, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Table { kind: TableTool::Custom, .. });
        let inserting = self.custom_tables.dlg.as_ref().is_some_and(|d| d.table.is_none());
        if active && self.custom_tables.dlg.is_none() {
            // The selected view's Part Studio, else the first in the drawing.
            let from_view = match &self.tool {
                Tool::Table { view: Some(v), .. } => noshape_model::drawing::views::view_source(self.current_sheet(), *v).map(|(k, _)| k.tab),
                _ => None,
            };
            let studios = self.drawing_studios(None, cx);
            let studio = from_view.filter(|t| studios.iter().any(|s| s.0 == *t)).or_else(|| studios.first().map(|s| s.0));
            self.custom_tables.dlg = Some(CustomDlg { table: None, original: None, studio, chosen: None, bottom_to_top: false, corner: TableCorner::TopRight, picker: None });
        } else if !active && inserting {
            self.custom_tables.dlg = None;
        }
    }

    /// Right-click › Custom table properties… (or a double-click on the
    /// fixed corner).
    pub fn open_custom_props(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(t) = self.current_sheet().table(id).cloned() else { return };
        let TableKind::Custom { studio, bottom_to_top, .. } = &t.kind else { return };
        let chosen = table_ref(&t.kind, &self.doc.read(cx).session.doc);
        self.tool = Tool::Select;
        self.custom_tables.dlg = Some(CustomDlg { table: Some(id), original: Some((t.kind.clone(), t.corner)), studio: Some(*studio), chosen, bottom_to_top: *bottom_to_top, corner: t.corner, picker: None });
        cx.notify();
    }

    /// The dialog's table kind (keeping the pinned Part Studio version).
    fn custom_kind(d: &CustomDlg, old: Option<&TableKind>) -> Option<TableKind> {
        let (studio, t) = (d.studio?, d.chosen.as_ref()?);
        Some(noshape_scene::drawing_tools::custom_table_kind(studio, t.fs, t.name.clone(), t.version.clone(), d.bottom_to_top, old))
    }

    /// Editing a table: apply the dialog live.
    fn apply_custom_dlg(&mut self, cx: &mut Context<Self>) {
        if let Some(d) = &self.custom_tables.dlg
            && let Some(id) = d.table
            && let Some(kind) = Self::custom_kind(d, d.original.as_ref().map(|o| &o.0))
        {
            let corner = d.corner;
            self.edit_table(id, "Custom table properties", cx, |t| {
                t.kind = kind;
                t.corner = corner;
            });
        }
        cx.notify();
    }

    fn close_custom_dlg(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(d) = self.custom_tables.dlg.take() else { return };
        match (d.table, d.original) {
            (Some(id), Some((kind, corner))) if !accept => self.edit_table(id, "Custom table properties", cx, |t| {
                t.kind = kind;
                t.corner = corner;
            }),
            (None, _) => self.tool = Tool::Select,
            _ => {}
        }
        cx.notify();
    }

    /// A sheet click with the dialog open: place the table (once a custom
    /// table is chosen). True when the click was used.
    pub fn custom_dlg_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(d) = &self.custom_tables.dlg else { return false };
        if d.table.is_some() || d.picker.is_some() {
            return d.picker.is_some();
        }
        let Some(kind) = Self::custom_kind(d, None) else { return true };
        let corner = d.corner;
        let c = self.cursor;
        let id = self.current_sheet().next_table_id();
        self.edit_sheet("Custom table", cx, |s| s.tables.push(noshape_scene::drawing_tools::custom_table(id, kind, corner, c)));
        self.custom_tables.dlg = None;
        self.tool = Tool::Select;
        self.selection = vec![Sel::Table(id)];
        self.ensure_sources(false, cx);
        cx.notify();
        true
    }

    /// Whether the dialog wants the table outline at the cursor.
    pub fn custom_dlg_placing(&self) -> bool {
        self.custom_tables.dlg.as_ref().is_some_and(|d| d.table.is_none() && d.picker.is_none() && d.studio.is_some() && d.chosen.is_some())
    }

    /// Evaluate the drawing's custom tables whose Part Studio or Feature
    /// Studio changed (in the background; the sheet rebuilds when done).
    pub fn ensure_custom_tables(&mut self, cx: &mut Context<Self>) {
        let keys = custom_table_keys(self.display());
        self.info.custom_tables.retain(|k, _| keys.contains(k));
        for key in keys {
            let Some(src) = self.sources.get(&key.source) else { continue };
            let fs_hash = match (&key.fs_version, self.doc.read(cx).session.doc.tab(key.fs).map(|t| &t.content)) {
                (None, Some(TabContent::FeatureStudio(f))) => {
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    f.source.hash(&mut h);
                    h.finish()
                }
                _ => 0,
            };
            let stamp = src.hash ^ fs_hash.rotate_left(7);
            if self.custom_tables.stamps.get(&key) == Some(&stamp) || self.custom_tables.running.contains(&key) {
                continue;
            }
            self.custom_tables.running.insert(key.clone());
            let ctx = src.context.clone();
            let doc = self.doc.read(cx).session.doc.clone();
            let k2 = key.clone();
            cx.spawn(async move |this, cx| {
                let k3 = k2.clone();
                let result = cx
                    .background_spawn(async move {
                        let fs_doc = match k3.fs_version.as_deref() {
                            None => Some(Arc::new(doc)),
                            Some(v) => crate::derived::ExternalRef::parse(v).and_then(|e| crate::derived::external_snapshot(&e.path, e.microversion).ok()),
                        };
                        match fs_doc {
                            Some(d) => noshape_model::drawing::tables::evaluate_custom(&d, k3.fs, &k3.name, k3.fs_version.is_some(), &ctx).tables,
                            None => Err("The document version defining this table could not be opened".into()),
                        }
                    })
                    .await;
                this.update(cx, |this, cx| {
                    this.custom_tables.running.remove(&k2);
                    this.custom_tables.stamps.insert(k2.clone(), stamp);
                    // Live drawing/277: a table that throws keeps its last
                    // good content, with an info toast.
                    let had = this.info.custom_tables.get(&k2).is_some_and(|r| r.is_ok());
                    if result.is_err() && had {
                        this.status = Some("Table creation failed. For details, open the FeatureScript notices pane and view the table in the Part Studio side pane.".into());
                    } else {
                        this.info.custom_tables.insert(k2, Arc::new(result));
                    }
                    this.refresh(cx);
                })
                .ok();
            })
            .detach();
        }
    }

    /// Whether the cursor is on a corner of table `id` (a double-click there
    /// opens its properties; help H:table).
    pub fn table_corner_at(&self, id: u64) -> bool {
        let Some(l) = self.built.as_ref().and_then(|b| b.table_layout(id)) else { return false };
        let (lo, hi) = (l.min(), l.max());
        let tol = self.tol_mm(6.0);
        [[lo[0], lo[1]], [hi[0], lo[1]], [lo[0], hi[1]], [hi[0], hi[1]]].iter().any(|p| (self.cursor[0] - p[0]).hypot(self.cursor[1] - p[1]) < tol)
    }

    /// Right-click on a BOM, cut list or custom table with the whole table
    /// selected, in Onshape's order (help custom-tables-14, bom_table,
    /// cut_list_table): <type> table properties…, Split …, (custom: Paste,)
    /// Copy, (custom: Paste as table,) Move to ▸, Bring to front, Send to
    /// back | Switch to <source>, Move to sheet… | Clear selection, Zoom to
    /// fit, Delete.
    pub fn data_table_entries(&self, id: u64, cx: &App) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let sheet = self.current_sheet();
        let Some(t) = sheet.table(id) else { return out };
        out.push(match t.kind {
            TableKind::Custom { .. } => MenuEntry::item("Custom table properties...", move |t, _, cx| t.open_custom_props(id, cx)),
            TableKind::Bom { .. } => MenuEntry::item("BOM table properties...", move |t, _, cx| t.open_table_props(id, cx)),
            TableKind::CutList { .. } => MenuEntry::item("Cut list table properties...", move |t, _, cx| t.open_table_props(id, cx)),
            TableKind::Hole { .. } => MenuEntry::item("Hole table properties...", move |t, _, cx| t.open_hole_table_props(id, cx)),
            // General tables (live drawing/549): only the fixed corner.
            _ => MenuEntry::item("Table properties...", move |t, _, cx| t.open_table_props_corner(id, cx)),
        });
        let cell = self.built.as_ref().and_then(|b| b.table_layout(id)).and_then(|l| l.cell_at(self.cursor));
        if !matches!(t.kind, TableKind::General { .. }) {
            out.extend(self.split_entries(t, cell));
        }
        // Live request 1638 (b): a configured Part Studio's cut list.
        if let TableKind::CutList { view, .. } = t.kind
            && let Some((key, _)) = noshape_model::drawing::views::view_source(sheet, view)
            && self.reference_configurable(key.tab, cx)
        {
            out.push(MenuEntry::item("Change configuration...", move |t, w, cx| t.open_table_change_config(id, w, cx)));
        }
        out.push(MenuEntry::item("Copy", move |t, _, cx| {
            if !t.selection.contains(&Sel::Table(id)) {
                t.selection = vec![Sel::Table(id)];
            }
            t.copy_entities(cx);
        }));
        // Live: always present, never greyed.
        out.push(MenuEntry::item("Paste as table", |t, _, cx| {
            t.start_paste_table(cx);
        }));
        let cur = t.layer;
        let layers: Vec<MenuEntry> = SheetLayer::FORMAT
            .iter()
            .copied()
            .chain((!cur.is_drawing()).then_some(SheetLayer::Drawing))
            .filter(|l| *l != cur)
            .map(|l| MenuEntry::item(l.name(), move |t, _, cx| t.edit_table(id, "Move to", cx, |t| t.layer = l)))
            .collect();
        out.push(MenuEntry::Submenu("Move to".into(), layers));
        // The table list is the stacking order. Live drawing/289: greyed only
        // when the table is the only object on its sheet.
        let alone = sheet.tables.len() == 1 && sheet.views.is_empty() && sheet.annotations.is_empty() && sheet.images.is_empty();
        out.push(MenuEntry::item("Bring to front", move |t, _, cx| t.order_table(id, true, cx)).disabled(alone));
        out.push(MenuEntry::item("Send to back", move |t, _, cx| t.order_table(id, false, cx)).disabled(alone));
        out.push(MenuEntry::Separator);
        let doc = &self.doc.read(cx).session.doc;
        if let TableKind::Bom { view, .. } = t.kind
            && let Some((key, _)) = noshape_model::drawing::views::view_source(sheet, view)
            && let Some(name) = doc.tab(key.tab).map(|t| t.name.clone())
        {
            let tab = key.tab;
            out.push(MenuEntry::item(format!("Switch to {name}"), move |_, w, cx| crate::actions::request_activate_tab(tab, w, cx)));
        }
        if let TableKind::Custom { studio, fs, fs_version, .. } = &t.kind {
        let (studio, fs) = (*studio, *fs);
        let studio_name = doc.tab(studio).map(|t| t.name.clone()).unwrap_or_default();
        out.push(MenuEntry::item(format!("Switch to Custom tables : {studio_name}"), move |_, w, cx| crate::actions::request_activate_tab(studio, w, cx)));
        if let Some(e) = fs_version.as_deref().and_then(crate::derived::ExternalRef::parse) {
            let path = e.path;
            out.push(MenuEntry::item("Open Feature Studio in linked document", move |t, _, cx| {
                let gpu = t.gpu.clone();
                if let Err(e) = crate::drawing_hooks::open_document_window(path.clone(), gpu, cx) {
                    t.status = Some(format!("Could not open the document: {e:#}").into());
                }
            }));
        } else {
            let fs_name = doc.tab(fs).map(|t| t.name.clone()).unwrap_or_default();
            out.push(MenuEntry::item(format!("Switch to {fs_name}"), move |_, w, cx| crate::actions::request_activate_tab(fs, w, cx)));
        }
        }
        out.push(MenuEntry::item("Move to sheet...", move |t, _, cx| t.open_move_sheet(crate::drawing_move_sheet::MoveTarget::Table(id), cx)));
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Clear selection", |t, _, cx| {
            t.selection.clear();
            t.redraw(cx);
        }));
        out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
        out.push(MenuEntry::item("Delete", move |t, _, cx| {
            t.selection.clear();
            t.edit_sheet("Delete table", cx, |s| s.tables.retain(|x| x.id != id));
        }));
        out
    }

    /// Bring to front / Send to back: the table list is the stacking order.
    pub fn order_table(&mut self, id: u64, front: bool, cx: &mut Context<Self>) {
        self.edit_sheet(if front { "Bring to front" } else { "Send to back" }, cx, |s| {
            if let Some(i) = s.tables.iter().position(|t| t.id == id) {
                let t = s.tables.remove(i);
                if front {
                    s.tables.push(t);
                } else {
                    s.tables.insert(0, t);
                }
            }
        });
    }

    /// Move to sheet: the table goes to sheet `to` (None: a new sheet) at
    /// the same place.
    pub fn move_table_to_sheet(&mut self, id: u64, to: Option<usize>, cx: &mut Context<Self>) {
        let mut d = self.drawing.clone();
        let Some(i) = d.sheets[self.sheet].tables.iter().position(|t| t.id == id) else { return };
        let to = to.unwrap_or_else(|| d.add_sheet(self.sheet));
        let mut t = d.sheets[self.sheet].tables.remove(i);
        let nid = d.sheets[to].next_table_id();
        t.id = nid;
        d.sheets[to].tables.push(t);
        let name = d.sheets[to].name.clone();
        self.commit("Move to sheet", d, cx);
        self.switch_sheet(to, cx);
        self.selection = vec![Sel::Table(nid)];
        self.status = Some(format!("Moved to {name}").into());
        cx.notify();
    }

    /// Click / double-click a custom table cell (live drawing/269): the
    /// cell is selected and the Cell format panel opens; the text itself is
    /// not editable.
    pub fn open_custom_cell(&mut self, t: &Table, window: &mut Window, cx: &mut Context<Self>) {
        let Some((r, c)) = self.built.as_ref().and_then(|b| b.table_layout(t.id)).and_then(|l| l.cell_at(self.cursor)) else { return };
        self.open_cell_format_panel(t.id, r, c, window, cx);
    }

    fn pick_studio(&mut self, tab: TabId, cx: &mut Context<Self>) {
        if let Some(d) = &mut self.custom_tables.dlg {
            d.studio = Some(tab);
            d.picker = None;
        }
        self.apply_custom_dlg(cx);
    }

    fn pick_table(&mut self, t: TableRef, cx: &mut Context<Self>) {
        if let Some(d) = &mut self.custom_tables.dlg {
            d.chosen = Some(t);
            d.picker = None;
        }
        self.apply_custom_dlg(cx);
    }

    pub fn render_custom_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.custom_tables.dlg.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let ed = cx.entity();
        let editing = d.table.is_some();
        let mine = ct::my_tables();
        let (ws, doc_name, ws_name) = {
            let doc = self.doc.read(cx);
            (ct::workspace_tables(&doc.session.doc), doc.name().to_string(), doc.session.store.branch(doc.session.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into()))
        };
        let studios = self.drawing_studios(d.studio, cx);
        let studio_name = d.studio.and_then(|s| studios.iter().find(|x| x.0 == s)).map(|s| s.1.clone());
        let row = |label: &'static str, body: AnyElement| h_flex().min_h(px(28.)).px(px(8.)).child(div().w(px(90.)).flex_none().text_size(px(12.)).child(label)).child(div().flex_1().min_w_0().child(body)).into_any_element();
        // A select: label ▾ with a natural-width list (live custom-tables-02).
        type Set = std::rc::Rc<dyn Fn(&mut DrawingEditor, &mut Context<DrawingEditor>)>;
        let select = |id: &'static str, label: String, items: Vec<(String, bool, Set)>| {
            let ed = ed.clone();
            Button::new(id).label(label).ghost().xsmall().w_full().dropdown_caret(true).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).rounded(px(0.)).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                for (l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| e.update(cx, |t, cx| f(t, cx))));
                }
                m
            })
        };
        let orders: Vec<(String, bool, Set)> = vec![
            ("Top to bottom".into(), !d.bottom_to_top, std::rc::Rc::new(|t: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| {
                if let Some(d) = &mut t.custom_tables.dlg {
                    d.bottom_to_top = false;
                }
                t.apply_custom_dlg(cx);
            }) as Set),
            ("Bottom to top".into(), d.bottom_to_top, std::rc::Rc::new(|t: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| {
                if let Some(d) = &mut t.custom_tables.dlg {
                    d.bottom_to_top = true;
                }
                t.apply_custom_dlg(cx);
            }) as Set),
        ];
        let corner = d.corner;
        let corners: Vec<AnyElement> = CORNERS
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let k = *k;
                let on = corner == k;
                let (top, left) = (matches!(k, TableCorner::TopLeft | TableCorner::TopRight), matches!(k, TableCorner::TopLeft | TableCorner::BottomLeft));
                div()
                    .id(("ct-corner", i))
                    .size(px(24.))
                    .relative()
                    .cursor_pointer()
                    .border_1()
                    .border_color(gpui_kit::rgb(0x909090))
                    .when(on, |e| e.border_b_2().border_color(pal.accent))
                    .child(crate::ui::glyph("table", 20., ink))
                    .child(div().absolute().size(px(4.)).rounded_full().bg(ink).when(top, |e| e.top(px(1.))).when(!top, |e| e.bottom(px(1.))).when(left, |e| e.left(px(1.))).when(!left, |e| e.right(px(1.))))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.custom_tables.dlg {
                            d.corner = k;
                        }
                        this.apply_custom_dlg(cx);
                    }))
                    .into_any_element()
            })
            .collect();
        // An icon button (Add / Insert): full width until something is chosen.
        let action = |id: &'static str, icon: &'static str, label: &'static str, enabled: bool, full: bool, cx: &mut Context<Self>| {
            h_flex()
                .id(id)
                .h(px(28.))
                .when(full, |b| b.flex_1())
                .when(!full, |b| b.w(px(64.)).flex_none())
                .justify_center()
                .gap(px(4.))
                .border_1()
                .border_color(gpui_kit::rgb(0xcccccc))
                .bg(crate::theme::light().panel)
                .text_size(px(14.))
                .when(!enabled, |b| b.opacity(0.4))
                .when(enabled, |b| b.cursor_pointer().hover(|b| b.bg(p().panel_hover)))
                .child(crate::ui::glyph(icon, 18., ink))
                .child(label)
                .when(enabled, |b| {
                    b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.custom_tables.dlg {
                            d.picker = Some(if id == "ct-add" { Picker::Studio } else { Picker::Table { pending: d.chosen.clone(), list_open: false } });
                        }
                        cx.notify();
                    }))
                })
        };
        let studio_items: Vec<(String, bool, Set)> = studios
            .iter()
            .map(|(t, n)| {
                let t = *t;
                (n.clone(), Some(t) == d.studio, std::rc::Rc::new(move |e: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| e.pick_studio(t, cx)) as Set)
            })
            .collect();
        let table_items: Vec<(String, bool, Set)> = mine
            .iter()
            .chain(ws.iter())
            .map(|t| {
                let t2 = t.clone();
                (t.display.clone(), d.chosen.as_ref() == Some(t), std::rc::Rc::new(move |e: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| e.pick_table(t2.clone(), cx)) as Set)
            })
            .collect();
        let add_row = h_flex().px(px(4.)).pt(px(4.)).gap(px(4.)).child(action("ct-add", "insert", "Add", true, studio_name.is_none(), cx)).when_some(studio_name.clone(), |r, n| r.child(div().flex_1().child(select("ct-studio", n, studio_items))));
        let insert_row = h_flex()
            .px(px(4.))
            .pt(px(4.))
            .gap(px(4.))
            .child(action("ct-insert", "featurescript-table", "Insert", d.studio.is_some(), d.chosen.is_none(), cx))
            .when_some(d.chosen.as_ref().map(|t| t.display.clone()), |r, n| r.child(div().flex_1().child(select("ct-table", n, table_items))));
        let close = |id: &'static str, cx: &mut Context<Self>| {
            div().id(id).w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_custom_dlg(false, cx)))
        };
        let title = h_flex()
            .h(px(28.))
            .pl(px(11.))
            .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(if editing { "Custom table properties" } else { "Custom table" }))
            .when(editing, |h| h.child(div().id("ct-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(pal.accept).cursor_pointer().child(crate::ui::glyph("check", 16., gpui_kit::white())).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_custom_dlg(true, cx)))))
            .child(close("ct-close", cx));
        let mut dlg = v_flex()
            .id("dr-custom-dlg")
            .absolute()
            .top(px(DIALOG_TOP))
            .left(px(self.dialog_left()))
            .w(px(if editing { 300. } else { 210. }))
            .pb(px(6.))
            .bg(pal.dialog_bg)
            .shadow(crate::drawing_chrome::dialog_shadow())
            .text_color(ink)
            .text_size(px(12.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(title);
        if editing {
            // Properties (help custom-tables-16): the document, workspace,
            // the Part Studio and Feature Studio links, the version, the sheet.
            // Links (live drawing/271): tooltip "Switch to the tab: <tab>".
            let tab_name = |tab: Option<TabId>| tab.and_then(|t| self.doc.read(cx).session.doc.tab(t).map(|t| t.name.clone())).unwrap_or_default();
            let link = |id: &'static str, icon: &'static str, text: String, tab: Option<TabId>, tip: String| {
                h_flex()
                    .id(id)
                    .gap(px(4.))
                    .child(crate::assets::icon(icon, 18., cx))
                    .child(div().truncate().text_size(px(13.)).text_color(gpui_kit::rgb(0x1651b0)).child(text))
                    .when_some(tab, |l, tab| l.cursor_pointer().tooltip(crate::ui::tooltip(format!("Switch to the tab: {tip}"))).on_click(move |_, w, cx| crate::actions::request_activate_tab(tab, w, cx)))
                    .into_any_element()
            };
            let chosen = d.chosen.clone();
            let fs_tab = chosen.as_ref().filter(|t| t.version.is_none()).map(|t| t.fs);
            dlg = dlg
                .child(row("Document:", div().text_size(px(13.)).child(crate::ui::user_text(doc_name)).into_any_element()))
                .child(row("Workspace:", div().text_size(px(13.)).child(ws_name).into_any_element()))
                // "Custom table: [Part Studio icon] FS sizes" — the table's
                // name, linking to its Part Studio (live drawing/271).
                .child(row("Custom table:", link("ct-props-studio", "part-studio", chosen.as_ref().map(|t| t.display.clone()).unwrap_or_default(), d.studio, tab_name(d.studio))))
                .child(row("Feature Studio:", link("ct-props-fs", "feature-studio", chosen.as_ref().map(|t| t.fs_name.clone()).unwrap_or_default(), fs_tab, tab_name(fs_tab))))
                .when_some(chosen.and_then(|t| t.version_label), |v, label| v.child(row("", h_flex().gap(px(4.)).child(crate::ui::glyph("version", 14., ink)).child(div().text_size(px(13.)).child(label)).into_any_element())))
                .child(row("Sheet", div().text_size(px(13.)).child(self.current_sheet().name.clone()).into_any_element()));
        } else {
            dlg = dlg.child(add_row).child(insert_row);
        }
        dlg = dlg
            .child(row("Order", select("ct-order", if d.bottom_to_top { "Bottom to top".into() } else { "Top to bottom".into() }, orders).into_any_element()))
            .child(div().px(px(8.)).pt(px(4.)).text_size(px(11.)).child("Select fixed corner"))
            .child(h_flex().px(px(8.)).pt(px(4.)).gap(px(26.)).children(corners));
        // The pickers sit beside the dialog.
        let picker = d.picker.clone().map(|pk| self.render_custom_picker(pk, &mine, &ws, cx));
        Some(div().absolute().top_0().left_0().child(dlg).children(picker).into_any_element())
    }

    /// "Select a Part Studio" / "Select a custom table" (help
    /// custom-tables-02, -03): a 300 px dialog right of the Custom table
    /// dialog.
    fn render_custom_picker(&self, pk: Picker, mine: &[TableRef], ws: &[TableRef], cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let title = if pk == Picker::Studio { "Select a Part Studio" } else { "Select a custom table" };
        let mut body = v_flex().p(px(6.)).gap(px(6.));
        match pk {
            Picker::Studio => {
                for (i, (tab, name)) in self.part_studios(cx).into_iter().enumerate() {
                    body = body.child(
                        h_flex()
                            .id(("ct-pick-studio", i))
                            .h(px(32.))
                            .px(px(6.))
                            .gap(px(6.))
                            .cursor_pointer()
                            .hover(|r| r.bg(gpui_kit::rgb(0xdef1fb)))
                            .child(crate::assets::icon("part-studio", 20., cx))
                            .child(div().text_size(px(14.)).child(crate::ui::user_text(name)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_studio(tab, cx))),
                    );
                }
            }
            Picker::Table { pending, list_open } => {
                let label = pending.as_ref().map(|t| t.display.clone()).unwrap_or_else(|| "Select a custom table".into());
                body = body.child(
                    h_flex()
                        .id("ct-pick-table")
                        .h(px(32.))
                        .px(px(6.))
                        .gap(px(5.))
                        .rounded(px(3.))
                        .border_1()
                        .border_color(gpui_kit::rgb(0xaaaaaa))
                        .bg(gpui_kit::rgb(0xeeeeee))
                        .cursor_pointer()
                        .when_some(pending.as_ref(), |r, t| r.child(ct::badge(&t.display)))
                        .child(div().flex_1().text_size(px(14.)).child(label))
                        .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CHEVRON_DOWN, 14., ink))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(CustomDlg { picker: Some(Picker::Table { list_open, .. }), .. }) = &mut this.custom_tables.dlg {
                                *list_open = !*list_open;
                            }
                            cx.notify();
                        })),
                );
                if list_open {
                    let ed = cx.entity();
                    body = body.child(ct::table_list("ct-pick-list", mine, ws, pending.as_ref(), 286., move |t, _, cx| {
                        ed.update(cx, |this, cx| {
                            if let Some(CustomDlg { picker: Some(Picker::Table { pending, list_open }), .. }) = &mut this.custom_tables.dlg {
                                *pending = Some(t);
                                *list_open = false;
                            }
                            cx.notify();
                        })
                    }));
                }
                let ready = pending.clone();
                body = body.child(
                    h_flex().justify_end().child(
                        Button::new("ct-pick-done")
                            .label("Done")
                            .primary()
                            .small()
                            .disabled(ready.is_none())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(t) = ready.clone() {
                                    this.pick_table(t, cx);
                                }
                            })),
                    ),
                );
            }
        }
        v_flex()
            .id("dr-custom-picker")
            .absolute()
            .top(px(DIALOG_TOP))
            .left(px(self.dialog_left() + 216.))
            .w(px(300.))
            .bg(pal.dialog_bg)
            .shadow(crate::drawing_chrome::dialog_shadow())
            .text_color(ink)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                h_flex()
                    .h(px(30.))
                    .pl(px(8.))
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xdddddd))
                    .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(title))
                    .child(div().id("ct-pick-close").w(px(28.)).h(px(28.)).flex().items_center().justify_center().opacity(0.5).cursor_pointer().child(crate::ui::glyph("close", 14., ink)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.custom_tables.dlg {
                            d.picker = None;
                        }
                        cx.notify();
                    }))),
            )
            .child(body)
            .into_any_element()
    }
}
