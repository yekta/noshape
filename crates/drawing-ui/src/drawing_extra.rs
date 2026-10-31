//! Drawing editor, second part: sheet tabs (add / rename / duplicate /
//! reorder / delete), jogged and aligned section tools, crop and break
//! views, view rotation, ordinate / chamfer / arc length dimensions, the
//! dimension properties panel (precision, units, tolerances), GD&T, datum
//! and surface finish symbols, tables (general, BOM, hole) and the drags
//! and context menus that go with them.

use noshape_model::document::TabId;
use crate::drawing_scene::Overlay;
use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea};
use gpui_kit::component::{Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::TabContent;
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::format;
use noshape_model::drawing::views;
use noshape_model::drawing::*;

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}
fn mul(a: [f64; 2], k: f64) -> [f64; 2] {
    [a[0] * k, a[1] * k]
}
fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn len(a: [f64; 2]) -> f64 {
    dot(a, a).sqrt()
}
fn norm(a: [f64; 2]) -> [f64; 2] {
    let l = len(a).max(1e-12);
    [a[0] / l, a[1] / l]
}

/// Sheet rows dragged in the Sheets panel. Live draws no ghost at the
/// pointer: only the insertion bar moves (behaviours "spec: dx drawing.md
/// S8", qa DWG-SHEET-DRAG-GHOST).
#[derive(Clone)]
pub struct SheetDrag {
    rows: Vec<usize>,
}

impl Render for SheetDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// A view or table row dragged in the Sheets panel onto a sheet row:
/// moved to that sheet (help sheets, drawing.md S9 / TB30). No ghost.
#[derive(Clone)]
pub struct ItemDrag {
    view: Option<ViewId>,
    table: Option<u64>,
}

impl Render for ItemDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Width of the open Sheets panel (live drawing/30).
pub const SHEETS_W: f32 = 258.;

/// The row a Sheets panel "Rename" popup (Shift+N, live K20) renames.
#[derive(Clone, Copy, PartialEq)]
pub enum RowRename {
    Sheet(usize),
    View(ViewId),
}

impl DrawingEditor {
    // ------------------------------------------------------------ sheets

    pub fn switch_sheet(&mut self, i: usize, cx: &mut Context<Self>) {
        let i = i.min(self.drawing.sheets.len().saturating_sub(1));
        if i == self.sheet {
            return;
        }
        // Onshape: a sheet switch is an undoable change (Undo goes back to
        // the previous sheet).
        if self.drawing.active_sheet != i {
            self.sheet = i;
            let d = self.drawing.clone();
            self.commit("Switch sheet", d, cx);
        }
        self.show_sheet(i, cx);
    }

    /// Show sheet `i` (no undo entry: undo / redo and reloads).
    pub fn show_sheet(&mut self, i: usize, cx: &mut Context<Self>) {
        self.sheet = i;
        self.selection.clear();
        self.tool = Tool::Select;
        self.panel = None;
        self.preview = None;
        self.overlay = Overlay::default();
        self.geoms.clear();
        self.fitted = false;
        self.ensure_sources(false, cx);
        self.refresh(cx);
    }

    pub fn add_sheet_after(&mut self, i: usize, cx: &mut Context<Self>) {
        let mut d = self.drawing.clone();
        let at = d.add_sheet(i);
        self.commit("Add sheet", d, cx);
        self.switch_sheet(at, cx);
    }

    /// The Sheets panel sheet-row menu (live dom/context-menus-drawing-sheets.txt
    /// with one sheet: Rename... | Properties... | Duplicate | Copy; behaviours.md
    /// "Sheets panel with several sheets": Rename... · Duplicate · Paste · Copy
    /// | Move up · Move down | Delete, no Move up on the first sheet). Paste is
    /// offered with a copied sheet on the clipboard.
    fn sheet_entries(&self, i: usize, cx: &App) -> Vec<MenuEntry> {
        let n = self.drawing.sheets.len();
        let mut out = vec![MenuEntry::item("Rename...", move |t, w, cx| t.open_sheet_name_panel(i, w, cx))];
        if n == 1 {
            out.push(MenuEntry::item("Properties...", move |t, w, cx| t.open_sheet_props(i, w, cx)));
        }
        out.push(MenuEntry::item("Duplicate", move |t, _, cx| {
            let mut d = t.drawing.clone();
            if let Some(j) = d.duplicate_sheet(i) {
                t.commit("Duplicate sheet", d, cx);
                t.switch_sheet(j, cx);
            }
        }));
        if let Some(paste) = self.sheet_paste_entry(cx) {
            out.push(paste);
        }
        out.push(Self::sheet_copy_entry(vec![i]));
        let mv = |label: &'static str, to: usize| {
            MenuEntry::item(label, move |t, _, cx| {
                let cur = t.sheet;
                if cur == i {
                    t.sheet = to;
                } else if cur == to {
                    t.sheet = i;
                }
                t.edit("Move sheet", cx, |d| {
                    d.move_sheet(i, to);
                });
            })
        };
        if n > 1 {
            out.push(MenuEntry::Separator);
            if i > 0 {
                out.push(mv("Move up", i - 1));
            }
            if i + 1 < n {
                out.push(mv("Move down", i + 1));
            }
            out.push(MenuEntry::Separator);
            out.push(MenuEntry::item("Delete", move |t, _, cx| {
                let mut d = t.drawing.clone();
                if d.delete_sheet(i) {
                    let cur = t.sheet;
                    t.selection.clear();
                    t.sheet = if cur > i || cur >= d.sheets.len() { cur.saturating_sub(1) } else { cur };
                    t.commit("Delete sheet", d, cx);
                    t.fitted = false;
                }
            }));
        }
        out
    }

    /// Paste of copied sheets: appended at the end of the list (live).
    fn sheet_paste_entry(&self, cx: &App) -> Option<MenuEntry> {
        let copied = cx.read_from_clipboard().and_then(|c| c.text()).and_then(|t| clipboard::SheetClipboard::from_json(&t))?;
        Some(MenuEntry::item("Paste", move |t, _, cx| {
            let mut d = t.drawing.clone();
            let mut last = None;
            for s in &copied.sheets {
                last = Some(d.paste_sheet(s.clone()));
            }
            t.commit("Paste sheet", d, cx);
            if let Some(j) = last {
                t.switch_sheet(j, cx);
            }
        }))
    }

    fn sheet_copy_entry(rows: Vec<usize>) -> MenuEntry {
        MenuEntry::item("Copy", move |t, _, cx| {
            let sheets: Vec<Sheet> = rows.iter().filter_map(|i| t.drawing.sheets.get(*i).cloned()).collect();
            if !sheets.is_empty() {
                cx.write_to_clipboard(ClipboardItem::new_string(clipboard::SheetClipboard::new(sheets).to_json()));
            }
        })
    }

    /// Several selected sheet rows: Paste · Copy · Delete (live).
    fn sheet_multi_entries(&self, rows: Vec<usize>, cx: &App) -> Vec<MenuEntry> {
        let mut out: Vec<MenuEntry> = self.sheet_paste_entry(cx).into_iter().collect();
        out.push(Self::sheet_copy_entry(rows.clone()));
        if rows.len() < self.drawing.sheets.len() {
            out.push(MenuEntry::item("Delete", move |t, _, cx| {
                let mut d = t.drawing.clone();
                let mut rows = rows.clone();
                rows.sort_unstable_by(|a, b| b.cmp(a));
                for i in rows {
                    d.delete_sheet(i);
                }
                t.selection.clear();
                t.sheet_rows.clear();
                t.sheet = t.sheet.min(d.sheets.len() - 1);
                t.commit("Delete sheets", d, cx);
                t.fitted = false;
            }));
        }
        out
    }

    fn open_sheet_menu(&mut self, i: usize, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        // Live: a right-click adds the row to the selection (with the panel
        // closed, a sheet chip's menu is that sheet's alone).
        if !self.sheets_open {
            self.sheet_rows = vec![i];
        } else if !self.sheet_rows.contains(&i) {
            self.sheet_rows.push(i);
        }
        // Imported sheets (live drawing/951): Model has no menu; a locked R12
        // paper space only Rename… · Duplicate · Paste · Copy, greyed.
        let imported = self.drawing.sheets.get(i).and_then(|s| s.imported.clone());
        if imported.as_ref().is_some_and(|m| m.model) {
            return;
        }
        let entries = if imported.as_ref().is_some_and(|m| m.locked) {
            ["Rename...", "Duplicate", "Paste", "Copy"].into_iter().map(|l| MenuEntry::item(l, |_, _, _| {}).disabled(true)).collect()
        } else if self.sheet_rows.len() > 1 {
            self.sheet_multi_entries(self.sheet_rows.clone(), cx)
        } else {
            self.sheet_entries(i, cx)
        };
        let ed = cx.entity();
        // Onshape's right-click menu (os_menu: 12 px, 22 px rows).
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| fill_os_menu(m, &entries, &ed, window, cx));
        if !menu.read(cx).is_empty() {
            crate::os_menu::open(position, menu, window, cx);
        }
        cx.notify();
    }

    /// Sheets dragged in the panel and dropped before sheet `to` (live "dx
    /// sheet row drag"): one edit; the active sheet and the selected rows
    /// stay with their sheets.
    pub fn drop_sheets(&mut self, rows: &[usize], to: usize, cx: &mut Context<Self>) {
        self.sheet_drop = None;
        let mut d = self.drawing.clone();
        let Some(order) = d.move_sheets(rows, to) else {
            cx.notify();
            return;
        };
        let new_index = |old: usize| order.iter().position(|i| *i == old).unwrap_or(old);
        self.sheet = new_index(self.sheet);
        self.sheet_rows = self.sheet_rows.iter().map(|r| new_index(*r)).collect();
        self.edit("Move sheet", cx, |dr| *dr = d);
    }

    /// Name of the reference (part or source tab) a view's base view shows.
    fn view_reference(&self, sheet: &Sheet, id: ViewId, cx: &App) -> String {
        let mut root = id;
        while let Some(p) = sheet.parent_of(root) {
            root = p;
        }
        let Some(ViewKind::Base { source, part, .. }) = sheet.view(root).map(|v| &v.kind) else { return String::new() };
        let src = self.sources.iter().find(|(k, _)| k.tab == *source).map(|(_, m)| m);
        let name = match (src, part) {
            (Some(m), Some(_)) => m.part(part.as_ref()).map(|p| p.name.clone()).unwrap_or_else(|| m.name.clone()),
            (Some(m), None) => m.name.clone(),
            (None, _) => self.doc.read(cx).session.doc.tab(*source).map(|t| t.name.clone()).unwrap_or_default(),
        };
        // A pinned reference: "<name>\u{1}<version name>" (its own group).
        match sheet.view(root).and_then(|v| v.pinned.as_ref()) {
            Some(p) => format!("{name}\u{1}{}", p.name),
            None => name,
        }
    }

    /// The source tab and part of a view's reference.
    fn reference_source(&self, sheet: &Sheet, id: ViewId) -> Option<(TabId, Option<noshape_model::BodyId>)> {
        match &sheet.view(views::root_view(sheet, id)?)?.kind {
            ViewKind::Base { source, part, .. } => Some((*source, part.clone())),
            _ => None,
        }
    }

    /// Pin view reference / Unpin view reference: every base view of the
    /// source (and part), on every sheet.
    pub fn pin_reference(&mut self, tab: TabId, part: Option<noshape_model::BodyId>, pin: Option<noshape_model::drawing::PinnedVersion>, cx: &mut Context<Self>) {
        let name = if pin.is_some() { "Pin view reference" } else { "Unpin view reference" };
        self.edit(name, cx, |d| {
            for v in d.sheets.iter_mut().flat_map(|s| s.views.iter_mut()) {
                if matches!(&v.kind, ViewKind::Base { source, part: p, .. } if *source == tab && *p == part) {
                    v.pinned = pin.clone();
                }
            }
        });
        self.ensure_sources(false, cx);
    }

    /// Drawing tab › Change to version…: every base view on every sheet shows
    /// the document at that version (None: back to the workspace).
    pub fn pin_drawing(&mut self, pin: Option<noshape_model::drawing::PinnedVersion>, cx: &mut Context<Self>) {
        self.edit("Change to version", cx, |d| {
            for v in d.sheets.iter_mut().flat_map(|s| s.views.iter_mut()).filter(|v| matches!(v.kind, ViewKind::Base { .. })) {
                v.pinned = pin.clone();
            }
        });
        self.ensure_sources(false, cx);
    }

    /// The versions menu of Change to version…: the workspace, then each
    /// version (the current one checked).
    pub fn version_menu(&self, cx: &App) -> Vec<MenuEntry> {
        let cur = self.drawing.sheets.iter().flat_map(|s| &s.views).find_map(|v| v.pinned.clone());
        let mut out = vec![MenuEntry::check("Main (workspace)", cur.is_none(), |t, _, cx| t.pin_drawing(None, cx))];
        for v in self.doc.read(cx).session.store.list_versions().unwrap_or_default() {
            let pin = noshape_model::drawing::PinnedVersion { id: v.id.to_string(), name: v.name.clone() };
            let on = cur.as_ref().is_some_and(|c| c.id == pin.id);
            out.push(MenuEntry::check(v.name, on, move |t, _, cx| t.pin_drawing(Some(pin.clone()), cx)));
        }
        out
    }

    /// The reference row's menu (live dom/context-menus-drawing-sheets.txt):
    /// Switch to reference | Replace... (on every sheet; the chooser) |
    /// Change configuration... (configured sources), then Pin view
    /// reference ▸ versions, or Unpin.
    fn reference_menu(&self, tab: TabId, part: Option<noshape_model::BodyId>, pinned: bool, cx: &App) -> Vec<MenuEntry> {
        let replaced = part.clone();
        let assembly = matches!(self.doc.read(cx).session.doc.tab(tab).map(|t| &t.content), Some(noshape_model::document::TabContent::Assembly(_)));
        let mut out = vec![
            MenuEntry::item("Switch to reference", move |_, w, cx| crate::actions::request_activate_tab(tab, w, cx)),
            MenuEntry::Separator,
            MenuEntry::item("Replace...", move |t, _, cx| {
                t.insert.replace = Some(tab);
                t.insert.replace_part = Some(replaced.clone());
                t.insert.chooser = Some(assembly as u8);
                t.ensure_sources(false, cx);
                cx.notify();
            }),
        ];
        // Live S6: on a configured source's reference row (not on view rows).
        if self.reference_configurable(tab, cx) {
            let part = part.clone();
            out.push(MenuEntry::item("Change configuration...", move |t, w, cx| t.open_change_config(tab, part.clone(), w, cx)));
        }
        if pinned {
            out.push(MenuEntry::item("Unpin view reference", move |t, _, cx| t.pin_reference(tab, part.clone(), None, cx)));
        } else {
            let versions = self.doc.read(cx).session.store.list_versions().unwrap_or_default();
            let items: Vec<MenuEntry> = versions
                .into_iter()
                .map(|v| {
                    let pin = noshape_model::drawing::PinnedVersion { id: v.id.to_string(), name: v.name.clone() };
                    let part = part.clone();
                    MenuEntry::item(v.name, move |t, _, cx| t.pin_reference(tab, part.clone(), Some(pin.clone()), cx))
                })
                .collect();
            // Pinning needs a version of the document.
            if !items.is_empty() {
                out.push(MenuEntry::Submenu("Pin view reference".into(), items));
            }
        }
        out
    }

    /// A BOM / cut list table row of the Sheets panel, with Onshape's menu:
    /// Properties, Switch to BOM, Delete, Replace.
    fn sheet_table_row(&self, t: &Table, indent: f32, cx: &mut Context<Self>) -> AnyElement {
        let id = t.id;
        let (label, icon) = match t.kind {
            TableKind::CutList { .. } => ("Cut list", "cut-list"),
            _ => ("Bill of materials", "drawing-bom"),
        };
        // The assembly the BOM lists (its view's root source).
        let source = t.view().and_then(|v| views::root_view(self.current_sheet(), v)).and_then(|r| match &self.current_sheet().view(r)?.kind {
            ViewKind::Base { source, .. } => Some(*source),
            _ => None,
        });
        crate::ui::tree_row(SharedString::from(format!("dr-table-{id}")), self.selection.contains(&Sel::Table(id)))
            .on_drag(ItemDrag { view: None, table: Some(id) }, |d, _, _, cx| cx.new(|_| d.clone()))
            .pl(px(indent))
            .text_color(rgb(0x666666))
            .child(sheets_icon(icon, cx))
            .child(div().flex_1().truncate().child(label))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.selection = vec![Sel::Table(id)];
                this.redraw(cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    let mut entries = vec![MenuEntry::Submenu("Properties".into(), this.table_entries(id, cx).into_iter().filter(|x| !matches!(x, MenuEntry::Separator)).collect())];
                    if let Some(tab) = source {
                        entries.push(MenuEntry::item("Switch to BOM", move |_, w, cx| crate::actions::request_activate_tab(tab, w, cx)));
                    }
                    entries.push(MenuEntry::item("Delete", move |t, _, cx| {
                        t.selection.clear();
                        t.edit_sheet("Delete table", cx, |s| s.tables.retain(|x| x.id != id));
                    }));
                    if let Some(tab) = source {
                        entries.push(MenuEntry::item("Replace", move |t, _, cx| {
                            t.insert.replace = Some(tab);
                            t.insert.chooser = Some(1);
                            cx.notify();
                        }));
                    }
                    let ed = cx.entity();
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| fill_os_menu(m, &entries, &ed, window, cx));
                    crate::os_menu::open(e.position, menu, window, cx);
                }),
            )
            .into_any_element()
    }

    /// A view's Sheets panel row: its name and icon.
    fn view_row_label(&self, sheet: &Sheet, v: &DrawingView) -> (String, &'static str) {
        let linked = sheet.linked_parent(v.id).is_some();
        let named = |fallback: String| v.name.clone().unwrap_or(fallback);
        let (label, icon) = match &v.kind {
            // Onshape lists views by name: "Front", and projected views by the
            // orientation they show ("Right").
            // Onshape's tree shows base views with its "standard views" icon,
            // not the Insert view tool icon; a linked view gets the grey
            // "-Linked" variant with a dark ↗ (live drawing/760, 1040).
            ViewKind::Base { .. } => (named(crate::drawing_props::default_view_name(&self.drawing, sheet, v)), "drawing-standard-views"),
            ViewKind::Projected { .. } => (named(crate::drawing_props::default_view_name(&self.drawing, sheet, v)), if linked { "projected-view-linked" } else { "projected-view" }),
            ViewKind::Auxiliary { .. } => (named("Auxiliary view".to_string()), if linked { "auxiliary-view-linked" } else { "auxiliary-view" }),
            ViewKind::Section { label, .. } => (named(format!("Section {label}")), if linked { "section-view-linked" } else { "section-view" }),
            ViewKind::Detail { label, .. } => (named(format!("Detail {label}")), if linked { "detail-view-linked" } else { "detail-view" }),
            _ => (named("Section view".to_string()), if linked { "section-view-linked" } else { "section-view" }),
        };
        (v.label.clone().filter(|l| !l.is_empty()).unwrap_or(label), icon)
    }

    fn sheet_view_row(&self, sheet: &Sheet, v: &DrawingView, indent: f32, cx: &mut Context<Self>) -> AnyElement {
        // Derived views nest under their parent (20 px per level).
        // A view linked to a parent on another sheet sits at the top level
        // with an ↗ badge on its icon (live drawing/1040).
        let mut depth = 0;
        let mut cur = v.id;
        while let Some(p) = sheet.parent_of(cur).filter(|p| !p.is_ghost()) {
            depth += 1;
            cur = p;
        }
        let (label, icon) = self.view_row_label(sheet, v);
        let id = v.id;
        // Live (K20): a click selects the view (Ctrl+click adds); the current
        // sheet's only, the selection being the current sheet's.
        let current = std::ptr::eq(sheet, self.current_sheet());
        let selected = current && self.selection.contains(&Sel::View(id));
        let popup = (self.row_rename.as_ref().map(|r| r.0) == Some(RowRename::View(id))).then(|| self.row_rename_popup(cx));
        crate::ui::tree_row(SharedString::from(format!("dr-view-{}", v.id.0)), selected)
            .relative()
            .pl(px(indent + 20. * depth as f32))
            .text_color(rgb(0x666666))
            .child(sheets_icon(icon, cx))
            .child(div().flex_1().truncate().child(label))
            .when(current, |row| {
                row.on_drag(ItemDrag { view: Some(id), table: None }, |d, _, _, cx| cx.new(|_| d.clone())).on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                    let m = e.modifiers();
                    if m.control || m.platform {
                        match this.selection.iter().position(|s| *s == Sel::View(id)) {
                            Some(k) => {
                                this.selection.remove(k);
                            }
                            None => this.selection.push(Sel::View(id)),
                        }
                    } else {
                        this.selection = vec![Sel::View(id)];
                    }
                    this.redraw(cx);
                }))
            })
            .when_some(popup, |row, p| row.child(p))
            // Onshape's view row menu (live dom/context-menus-drawing-sheets.txt):
            // Properties... | Replace... (the view's reference) | Delete.
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    let src = this.reference_source(this.current_sheet(), id);
                    let assembly = src.as_ref().is_some_and(|(tab, _)| matches!(this.doc.read(cx).session.doc.tab(*tab).map(|t| &t.content), Some(TabContent::Assembly(_))));
                    let entries = vec![
                        MenuEntry::item("Properties...", move |t, w, cx| t.open_view_props(id, w, cx)),
                        MenuEntry::item("Replace...", move |t, _, cx| {
                            if let Some((tab, part)) = src.clone() {
                                t.insert.replace = Some(tab);
                                t.insert.replace_part = Some(part);
                                t.insert.chooser = Some(assembly as u8);
                                t.ensure_sources(false, cx);
                                cx.notify();
                            }
                        }),
                        MenuEntry::item("Delete", move |t, _, cx| {
                            t.selection.clear();
                            t.edit_sheet("Delete view", cx, |s| s.remove_view(id));
                        }),
                    ];
                    let ed = cx.entity();
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| fill_os_menu(m, &entries, &ed, window, cx));
                    crate::os_menu::open(e.position, menu, window, cx);
                }),
            )
            .into_any_element()
    }

    /// Onshape's sheet tabs at the top-left of the drawing area ("Sheet1",
    /// the active one on a `#c7c7c7` chip) while the Sheets panel is closed.
    /// The canvas draws them under its tool dialogs, which cover them.
    pub fn render_sheet_tabs(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let mut tabs = h_flex().absolute().top(px(4.)).left(px(4.)).gap(px(2.));
        for (i, s) in self.drawing.sheets.iter().enumerate() {
            let active = i == self.sheet;
            tabs = tabs.child(
                div()
                    .id(("dr-sheet-chip", i))
                    .occlude()
                    .h(px(20.))
                    .px(px(5.))
                    .flex()
                    .items_center()
                    .text_size(px(13.))
                    .text_color(pal.text)
                    .cursor_pointer()
                    .when(active, |d| d.bg(rgb(0xc7c7c7)))
                    .when(!active, |d| d.hover(|d| d.bg(rgb(0xd0d0d0))))
                    .child(crate::ui::user_text(s.name.clone()))
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        if this.drawing.sheets.get(i).and_then(|s| s.imported.as_ref()).is_some_and(|m| m.locked) {
                            return;
                        }
                        if e.click_count() >= 2 {
                            this.open_sheet_name_panel(i, window, cx);
                        } else {
                            this.switch_sheet(i, cx);
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.open_sheet_menu(i, e.position, window, cx);
                        }),
                    ),
            );
        }
        div().absolute().top_0().left_0().size_full().child(tabs).into_any_element()
    }

    /// The panel toggle on the canvas's left edge (the panel's edge when
    /// open); drawn under the dialogs.
    pub fn sheets_toggle(&self, left: f32, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let open = self.sheets_open;
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(left))
            .flex()
            .items_center()
            .child(
                div()
                    .id("dr-sheets-toggle")
                    .occlude()
                    .w(px(33.))
                    .h(px(34.))
                    .ml(px(-1.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(pal.panel)
                    .border_1()
                    .border_color(pal.border_strong)
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon("sheet", 20., cx))
                    .tooltip(crate::ui::tooltip(if open { "Collapse the Sheets panel" } else { "Show the Sheets panel" }))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.toggle_sheets_panel(cx);
                    })),
            )
            .into_any_element()
    }

    /// Onshape's Sheets panel: a 258 px `#fafafa` panel beside (left of) the
    /// drawing area. Blue "Sheets (n)" title with the Insert sheet button,
    /// a 2 px blue rule, "Sort by sheet | Sort by reference" tabs, then the
    /// tree (sheet › reference › views; the active sheet `#e9eef2` with a
    /// 4 px blue bar). Closed, only the sheet chips and the toggle show.
    /// Left of the drawing tool dialogs: beside the Sheets panel when open.
    /// Dialogs open at the canvas's left edge (the Sheets panel pushes the
    /// canvas, so it never covers them).
    pub fn dialog_left(&self) -> f32 {
        0.
    }

    pub fn render_sheet_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if !self.sheets_open {
            return self.render_sheet_tabs(cx);
        }
        const W: f32 = SHEETS_W;
        let blue = rgb(0x1b5faa);
        let pal = p();
        let si = self.sheet;
        let by_ref = self.sheets_by_ref;
        let mut list = gpui_kit::component::v_flex().id("dr-sheets").flex_1().min_h_0().pt(px(2.)).overflow_y_scroll();
        // Sheets before `i` that are an imported model space (not numbered).
        let models: Vec<bool> = self.drawing.sheets.iter().map(|s| s.imported.as_ref().is_some_and(|m| m.model)).collect();
        let model_before = move |i: usize| models[..i.min(models.len())].iter().filter(|m| **m).count();
        let n_sheets = self.drawing.sheets.len();
        let drop_at = self.sheet_drop.filter(|_| cx.has_active_drag());
        let sel_rows = self.sheet_rows.clone();
        let sheet_row = |i: usize, s: &Sheet, active: bool, selected: bool, cx: &mut Context<Self>| {
            // Dragged: the selected rows when this one is among them.
            let rows: Vec<usize> = if selected && sel_rows.len() > 1 { sel_rows.clone() } else { vec![i] };
            let drag = SheetDrag { rows };
            crate::ui::tree_row(("dr-sheet-tab", i), selected && !active)
                .relative()
                .on_drag(drag, |d, _, _, cx| cx.new(|_| d.clone()))
                // The drop goes before this row (upper half) or after it.
                .on_drag_move(cx.listener(move |this, e: &DragMoveEvent<SheetDrag>, _, cx| {
                    let b = e.bounds;
                    let y = e.event.position.y;
                    if y < b.top() || y >= b.bottom() || e.event.position.x < b.left() || e.event.position.x >= b.right() {
                        return;
                    }
                    let at = if y < b.top() + b.size.height / 2. { i } else { i + 1 };
                    if this.sheet_drop != Some(at) {
                        this.sheet_drop = Some(at);
                        cx.notify();
                    }
                }))
                .on_drop(cx.listener(|this, d: &SheetDrag, _, cx| {
                    if let Some(to) = this.sheet_drop {
                        this.drop_sheets(&d.rows, to, cx);
                    }
                }))
                // A view or table row dropped on another sheet moves there.
                .on_drop(cx.listener(move |this, d: &ItemDrag, _, cx| {
                    if i == this.sheet {
                        return;
                    }
                    if let Some(v) = d.view {
                        this.move_selected_views(v, Some(i), cx);
                    } else if let Some(t) = d.table {
                        this.move_table_to_sheet(t, Some(i), cx);
                    }
                }))
                // Live: a blue line across the panel where the rows go.
                .when(drop_at == Some(i), |r| r.child(div().absolute().left_0().right_0().top(px(-1.)).h(px(2.)).bg(blue)))
                .when(drop_at == Some(n_sheets) && i + 1 == n_sheets, |r| r.child(div().absolute().left_0().right_0().bottom(px(-1.)).h(px(2.)).bg(blue)))
                // Live drawing/281: chevron at x 18, page icon at 37, name at 60.
                .pl(px(13.))
                .cursor_pointer()
                .when(active, |d| d.bg(rgb(0xe9eef2)).text_color(blue).font_weight(FontWeight::SEMIBOLD).border_b_1().border_color(rgb(0x90cef1)))
                .child(crate::ui::caret(active))
                .child(div().w(px(20.)).flex_none().flex().items_center().justify_center().child(crate::assets::icon("sheet-page", 18., cx)))
                // Live: "Sheet2 (2)" = the name and the sheet's position; an
                // imported model space is "Model" (unnumbered, the layouts count
                // from 1 after it) and a locked R12 paper space is grey.
                .child(div().flex_1().truncate().child(match &s.imported {
                    Some(m) if m.model => s.name.clone(),
                    _ => format!("{} ({})", s.name, i + 1 - model_before(i)),
                }))
                .when(s.imported.as_ref().is_some_and(|m| m.locked), |d| d.text_color(rgb(0x666666)))
                .when(active, |d| d.child(div().absolute().left(px(3.)).top_0().bottom_0().w(px(4.)).bg(blue)))
                // Live: a click selects the row (Ctrl+click adds), a
                // double-click makes the sheet active.
                .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                    let m = e.modifiers();
                    let imported = this.drawing.sheets.get(i).and_then(|s| s.imported.clone());
                    if e.click_count() >= 2 && imported.as_ref().is_some_and(|m| m.locked) {
                        // A locked paper space can't be opened.
                    } else if e.click_count() >= 2 && i == this.sheet && imported.as_ref().is_some_and(|m| m.model) {
                        // Live drawing/970: the active Model's Sheet properties (Size only).
                        this.open_sheet_props(i, window, cx);
                    } else if e.click_count() >= 2 {
                        this.sheet_rows = vec![i];
                        this.switch_sheet(i, cx);
                    } else if m.control || m.platform {
                        match this.sheet_rows.iter().position(|r| *r == i) {
                            Some(k) => {
                                this.sheet_rows.remove(k);
                            }
                            None => this.sheet_rows.push(i),
                        }
                    } else {
                        this.sheet_rows = vec![i];
                    }
                    cx.notify();
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.open_sheet_menu(i, e.position, window, cx);
                    }),
                )
        };
        let ref_row = |this: &Self, key: String, name: &str, src: Option<(TabId, Option<noshape_model::BodyId>)>, indent: f32, cx: &mut Context<Self>| {
            let (name, version) = match name.split_once('\u{1}') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (name.to_string(), None),
            };
            let menu = src.map(|(tab, part)| this.reference_menu(tab, part, version.is_some(), cx));
            crate::ui::tree_row(SharedString::from(key), false)
                .cursor_pointer()
                .tooltip(crate::ui::tooltip("Reference manager"))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_ref_manager(cx)))
                .when_some(menu, |row, entries| {
                    row.on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |_, e: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            let entries = entries.clone();
                            let ed = cx.entity();
                            let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| fill_os_menu(m, &entries, &ed, window, cx));
                            crate::os_menu::open(e.position, menu, window, cx);
                        }),
                    )
                })
                .pl(px(indent))
                .text_color(rgb(0x666666))
                .child(crate::ui::caret(true))
                .child(sheets_icon("part", cx))
                .child(div().min_w_0().truncate().child(crate::ui::user_text(name)))
                // Pinned: a pin and the version name; else Onshape's "In progress" badge.
                .child(match version {
                    Some(v) => div().ml(px(8.)).flex().items_center().gap(px(3.)).child(crate::ui::glyph("pin", 11., rgb(0x666666))).child(div().text_size(px(11.)).child(v)),
                    None => div().ml(px(8.)).px(px(4.)).rounded(px(3.)).bg(rgb(0x9ccef0)).text_size(px(11.)).text_color(rgb(0x333333)).child("In progress"),
                })
                .child(div().flex_1())
        };
        // (reference, views) groups of a sheet, in first-view order.
        let groups = |this: &Self, s: &Sheet, cx: &App| {
            let mut g: Vec<(String, Vec<DrawingView>)> = Vec::new();
            for v in s.real_views() {
                let r = this.view_reference(s, v.id, cx);
                match g.iter_mut().find(|(n, _)| *n == r) {
                    Some((_, vs)) => vs.push(v.clone()),
                    None => g.push((r, vec![v.clone()])),
                }
            }
            g
        };
        if by_ref {
            let mut refs: Vec<(String, Vec<(usize, DrawingView)>)> = Vec::new();
            for (sheet_i, s) in self.drawing.sheets.iter().enumerate() {
                for (r, vs) in groups(self, s, cx) {
                    let entry = match refs.iter().position(|(n, _)| *n == r) {
                        Some(k) => &mut refs[k].1,
                        None => {
                            refs.push((r, Vec::new()));
                            &mut refs.last_mut().expect("just pushed").1
                        }
                    };
                    entry.extend(vs.into_iter().map(|v| (sheet_i, v)));
                }
            }
            for (k, (r, vs)) in refs.iter().enumerate() {
                let src = vs.first().and_then(|(si, v)| self.reference_source(&self.drawing.sheets[*si], v.id));
                list = list.child(ref_row(self, format!("dr-ref-{k}"), r, src, 6., cx));
                // Its sheets, with their views underneath.
                let mut last = None;
                for (sheet_i, v) in vs {
                    if last != Some(*sheet_i) {
                        last = Some(*sheet_i);
                        let s = &self.drawing.sheets[*sheet_i];
                        list = list.child(
                            crate::ui::tree_row(SharedString::from(format!("dr-ref-{k}-sheet-{sheet_i}")), false)
                                .pl(px(24.))
                                .text_color(rgb(0x666666))
                                .child(div().w(px(20.)).flex_none().flex().items_center().justify_center().child(crate::assets::icon("sheet-page", 18., cx)))
                                .child(div().flex_1().truncate().child(crate::ui::user_text(s.name.clone()))),
                        );
                    }
                    list = list.child(self.sheet_view_row(&self.drawing.sheets[*sheet_i], v, 44., cx));
                }
            }
        } else {
            for (i, s) in self.drawing.sheets.iter().enumerate() {
                let active = i == self.sheet;
                let popup = (self.row_rename.as_ref().map(|r| r.0) == Some(RowRename::Sheet(i))).then(|| self.row_rename_popup(cx));
                list = list.child(sheet_row(i, s, active, self.sheet_rows.contains(&i), cx).when_some(popup, |row, p| row.child(p)));
                if active {
                    for (k, (r, vs)) in groups(self, s, cx).iter().enumerate() {
                        let src = vs.first().and_then(|v| self.reference_source(s, v.id));
                        list = list.child(ref_row(self, format!("dr-sheet-ref-{k}"), r, src, 34., cx));
                        for v in vs {
                            list = list.child(self.sheet_view_row(s, v, 75., cx));
                        }
                    }
                    // BOM and cut list tables.
                    for t in s.tables.iter().filter(|t| matches!(t.kind, TableKind::Bom { .. } | TableKind::CutList { .. })) {
                        list = list.child(self.sheet_table_row(t, 54., cx));
                    }
                }
            }
        }
        let tab = |id: &'static str, label: &'static str, selected: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex_1()
                .h_full()
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .when(selected, |d| d.text_color(blue).font_weight(FontWeight::MEDIUM).child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(blue)))
                .when(!selected, |d| d.text_color(pal.text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.sheets_by_ref = id == "dr-sort-ref";
                    cx.notify();
                }))
        };
        let panel = gpui_kit::component::v_flex()
            .id("dr-sheets-panel")
            .occlude()
            .absolute()
            .top_0()
            .left_0()
            .bottom_0()
            .w(px(W))
            .bg(rgb(0xfafafa))
            .border_r_1()
            .border_color(rgb(0xd1d1d1))
            .text_size(px(13.))
            .child(
                h_flex()
                    .relative()
                    .h(px(37.))
                    .flex_none()
                    .justify_center()
                    .child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(blue).child(format!("Sheets ({})", self.drawing.sheets.len())))
                    .child(
                        div()
                            .id("dr-add-sheet")
                            .absolute()
                            .right(px(2.))
                            .top(px(4.))
                            .size(px(29.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .bg(rgb(0xffffff))
                            .shadow_sm()
                            .cursor_pointer()
                            .hover(|d| d.bg(rgb(0xeaeaea)))
                            .child(crate::assets::icon("sheet-insert", 20., cx))
                            .tooltip(crate::ui::tooltip("Insert sheet"))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.add_sheet_after(si, cx))),
                    ),
            )
            .child(div().h(px(2.)).flex_none().bg(blue))
            .child(h_flex().h(px(29.)).flex_none().bg(rgb(0xffffff)).border_b_1().border_color(rgb(0xdbdbdb)).child(tab("dr-sort-sheet", "Sort by sheet", !by_ref, cx)).child(tab("dr-sort-ref", "Sort by reference", by_ref, cx)))
            .child(list);
        // The panel takes its width beside the canvas (live drawing/30).
        div().relative().flex_none().w(px(W)).h_full().child(panel).into_any_element()
    }

    // ------------------------------------------------------------ tools

    /// Enter / double-click: finish a jogged cutting line.
    pub fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.more_confirm(window, cx) {
            return;
        }
        if let Tool::JoggedSection { parent: Some(pid), points, done: false } = self.tool.clone()
            && points.len() >= 2
        {
            let pts = self.extend_line_ends(pid, &points);
            self.tool = Tool::JoggedSection { parent: Some(pid), points: pts, done: true };
            self.update_preview(cx);
        } else if let Tool::Ordinate { .. } = self.tool {
            self.set_tool(Tool::Select, cx);
        }
    }

    /// Extend the end legs of a cutting line past the view's geometry.
    fn extend_line_ends(&self, view: ViewId, points: &[[f64; 2]]) -> Vec<[f64; 2]> {
        self.pk().extend_line_ends(view, points)
    }

    /// A point in a view: snap, else the cursor position in the view.
    pub fn view_point(&self, view: Option<ViewId>) -> Option<PickedPoint> {
        self.pk().view_point(self.cursor, view)
    }

    /// Orthogonal continuation (in the view's frame) from `last` toward `target`.
    fn ortho_view(last: [f64; 2], target: [f64; 2]) -> [f64; 2] {
        noshape_scene::drawing_tools::ortho_view(last, target)
    }

    /// Help crop_view: crops can't be made from detail, break or flat
    /// pattern views (says so in the status line).
    pub fn crop_refused(&mut self, v: ViewId) -> bool {
        let why = self.pk().view_refusal("crop", v);
        if let Some(why) = why {
            self.status = Some(why.into());
        }
        why.is_some()
    }

    pub fn next_datum_label(&self) -> String {
        self.pk().next_datum_label()
    }

    pub fn extra_overlay(&self, overlay: &mut Overlay) {
        let c = self.hover_snap.unwrap_or(self.cursor);
        match &self.tool {
            Tool::JoggedSection { parent: Some(pid), points, done } if !points.is_empty() => {
                let Some(pl) = self.placement(*pid) else { return };
                let mut pts: Vec<[f64; 2]> = points.iter().map(|p| pl.to_sheet(*p)).collect();
                if !done {
                    let last = *points.last().unwrap();
                    pts.push(pl.to_sheet(Self::ortho_view(last, pl.to_view(c))));
                }
                overlay.lines.push((pts, true));
            }
            Tool::AlignedSection { parent: Some(pid), center: Some(ctr), a, b: None } => {
                let Some(pl) = self.placement(*pid) else { return };
                let cs = pl.to_sheet(*ctr);
                let mut pts = vec![];
                if let Some(a) = a {
                    pts.push(pl.to_sheet(*a));
                }
                pts.push(cs);
                pts.push(c);
                overlay.lines.push((pts, true));
                overlay.points.push(cs);
            }
            Tool::Crop { parent: Some(_), first: Some(f), circle } => {
                if *circle {
                    overlay.circle(*f, len(sub(c, *f)), true);
                } else {
                    overlay.rect(*f, c, true);
                }
            }
            Tool::Break { parent: Some(pid), first, vertical } => {
                let Some(pl) = self.placement(*pid) else { return };
                let mut xs = vec![self.cursor];
                if let Some(f) = first {
                    xs.push(*f);
                }
                for x in xs {
                    if *vertical {
                        overlay.lines.push((vec![[x[0], pl.min[1] - 3.0], [x[0], pl.max[1] + 3.0]], true));
                    } else {
                        overlay.lines.push((vec![[pl.min[0] - 3.0, x[1]], [pl.max[0] + 3.0, x[1]]], true));
                    }
                }
            }
            Tool::Ordinate { origin: Some(o), .. } => {
                if let Some(s) = self.pick_sheet(o) {
                    overlay.points.push(s);
                }
            }
            // The hole table itself follows the pointer (`hole_table_preview`).
            Tool::Table { kind: TableTool::Hole, origin: Some(o), .. } => {
                if let Some(s) = self.pick_sheet(o) {
                    overlay.points.push(s);
                }
            }
            Tool::Table { kind: TableTool::Custom, .. } => {
                if self.custom_dlg_placing() {
                    overlay.rect([self.cursor[0] - 90.0, self.cursor[1] - 20.0], self.cursor, true);
                }
            }
            Tool::Table { kind: TableTool::General, .. } => {}
            // BOM and cut list tables with a source follow the pointer
            // themselves (`table_dlg_preview`).
            Tool::Table { kind: TableTool::Bom | TableTool::CutList, .. } if self.table_dlg_preview().is_some() => {}
            Tool::Table { kind, view, .. } if matches!(kind, TableTool::Revision | TableTool::Bom | TableTool::CutList) || view.is_some() => {
                overlay.rect([self.cursor[0] - 90.0, self.cursor[1] - 20.0], self.cursor, true);
            }
            Tool::Datum { anchor: Some(a) } if self.datum_on.is_none() => overlay.lines.push((vec![*a, self.cursor], true)),
            Tool::Datum { anchor: None } => {
                if let Some((p, _)) = self.datum_attach_at(self.cursor) {
                    let r = self.tol_mm(7.0);
                    overlay.tinted.push((vec![[p[0] - r, p[1]], [p[0] + r, p[1]]], [0xdc, 0x28, 0x28, 255], 1.5));
                    overlay.tinted.push((vec![[p[0], p[1] - r], [p[0], p[1] + r]], [0xdc, 0x28, 0x28, 255], 1.5));
                }
            }
            Tool::RevisionCallout { .. } => {
                let (p, s) = (self.cursor, 8.0);
                let y0 = p[1] - s * 0.866 / 3.0;
                overlay.lines.push((vec![[p[0] - s / 2.0, y0], [p[0] + s / 2.0, y0], [p[0], y0 + s * 0.866], [p[0] - s / 2.0, y0]], true));
            }
            _ => self.more_overlay(overlay),
        }
    }

    /// Clicks of the tools defined here; false when the tool is not one of them.
    pub fn extra_tool_click(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.more_tool_click(window, cx) {
            return true;
        }
        let c = self.cursor;
        match self.tool.clone() {
            Tool::JoggedSection { parent, points, done: false } => {
                if points.is_empty() || parent.is_none() {
                    if let Some(pp) = self.view_point(parent) {
                        self.tool = Tool::JoggedSection { parent: Some(pp.view), points: vec![pp.p], done: false };
                    }
                } else if let Some(pid) = parent
                    && let Some(pp) = self.view_point(Some(pid))
                {
                    let last = *points.last().unwrap();
                    let q = Self::ortho_view(last, pp.p);
                    let mut pts = points.clone();
                    if len(sub(q, last)) > 1e-7 {
                        pts.push(q);
                    }
                    self.tool = Tool::JoggedSection { parent: Some(pid), points: pts, done: false };
                }
            }
            // Aligned: the panel's Section id and Show cut geometry only.
            Tool::AlignedSection { b: Some(_), .. } => self.place_aligned_section(cx),
            Tool::JoggedSection { done: true, .. } => {
                if let Some(v) = self.pending_view() {
                    let id = v.id;
                    let name = if matches!(v.kind, ViewKind::AlignedSection { .. }) { "Aligned section view" } else { "Section view" };
                    self.edit_sheet(name, cx, |s| s.views.push(v));
                    self.tool = Tool::Select;
                    self.selection = vec![Sel::View(id)];
                }
            }
            Tool::AlignedSection { parent, center: None, .. } => {
                if let Some(pp) = self.view_point(parent) {
                    self.tool = Tool::AlignedSection { parent: Some(pp.view), center: Some(pp.p), a: None, b: None };
                }
            }
            Tool::AlignedSection { parent: Some(pid), center: Some(ctr), a, b: None } => {
                if let Some(pp) = self.view_point(Some(pid))
                    && len(sub(pp.p, ctr)) > 1e-7
                {
                    let ext = self.extend_line_ends(pid, &[pp.p, ctr]);
                    match a {
                        None => self.tool = Tool::AlignedSection { parent: Some(pid), center: Some(ctr), a: Some(ext[0]), b: None },
                        Some(a) => self.tool = Tool::AlignedSection { parent: Some(pid), center: Some(ctr), a: Some(a), b: Some(ext[0]) },
                    }
                }
            }
            Tool::Crop { parent, first: None, circle } => {
                let v = parent.or_else(|| self.view_at(c));
                if v.is_some_and(|v| self.crop_refused(v)) {
                    return true;
                }
                if v.is_some() {
                    let p = self.snap_at(c).and_then(|s| self.pick_sheet(&s)).unwrap_or(c);
                    self.tool = Tool::Crop { parent: v, first: Some(p), circle };
                }
            }
            Tool::Crop { parent: Some(v), first: Some(f), circle } => {
                let Some(pl) = self.placement(v) else { return true };
                let crop = noshape_scene::drawing_tools::view_crop(&pl, circle, f, c);
                // The Crop view dialog's ✓ applies it.
                match crop {
                    Some(crop) => self.crop_done(v, crop, cx),
                    None => self.tool = Tool::Crop { parent: None, first: None, circle },
                }
            }
            Tool::Break { parent, first: None, vertical } => {
                let v = parent.or_else(|| self.view_at(c));
                // Onshape: no breaks on detail views, nor on views with angular cut lines.
                if let Some(why) = v.and_then(|v| self.pk().view_refusal("break", v)) {
                    self.status = Some(why.into());
                    return true;
                }
                if v.is_some() {
                    self.tool = Tool::Break { parent: v, first: Some(c), vertical };
                }
            }
            Tool::Break { parent: Some(v), first: Some(f), vertical } => {
                let Some(pl) = self.placement(v) else { return true };
                let (style, gap_mm) = self.break_dlg.as_ref().map(|d| (d.style, d.gap)).unwrap_or((BreakStyle::ZigZag, 6.0));
                if let Some(b) = noshape_scene::drawing_tools::view_break(&pl, vertical, f, c, style, gap_mm) {
                    self.edit_view("Break view", v, cx, |dv| dv.breaks.push(b));
                }
                self.tool = Tool::Select;
            }
            Tool::Ordinate { origin: None, .. } => {
                if let Some(p) = self.any_point(c) {
                    self.tool = Tool::Ordinate { origin: Some(p), horizontal: None, group: None };
                }
            }
            Tool::Ordinate { origin: Some(o), group, .. } => {
                if let Some(a) = self.pending_annotation() {
                    let AnnotationKind::OrdinateGroup { horizontal: h, .. } = a.kind.clone() else { return true };
                    let gid = a.id;
                    match group {
                        None => self.edit_sheet("Ordinate dimension", cx, |s| s.annotations.push(a)),
                        Some(_) => self.edit_sheet("Add to ordinate group", cx, |s| {
                            if let Some(x) = s.annotations.iter_mut().find(|x| x.id == gid) {
                                *x = a;
                            }
                        }),
                    }
                    self.tool = Tool::Ordinate { origin: Some(o), horizontal: Some(h), group: Some(gid) };
                }
            }
            // Placed by the Table dialog (drawing_table_tool).
            Tool::Table { kind: TableTool::General, .. } => {}
            Tool::Table { kind: TableTool::Revision, .. } => self.place_revision_table(c, cx),
            Tool::RevisionCallout { revision } => self.place_revision_callout(revision, c, cx),
            // Placed by the Custom table dialog.
            Tool::Table { kind: TableTool::Custom, .. } => {}
            Tool::Table { kind, view: None, origin } => {
                if let Some(v) = self.view_at(c) {
                    self.tool = Tool::Table { kind, view: Some(v), origin };
                }
            }
            Tool::Table { kind: TableTool::Hole, .. } if self.hole_x_dir_click(cx) || self.hole_exclude_click(cx) => {}
            Tool::Table { kind: TableTool::Hole, view: Some(v), origin: None } => {
                if let Some(p) = self.snap_at(c).filter(|p| p.view == v) {
                    self.tool = Tool::Table { kind: TableTool::Hole, view: Some(v), origin: Some(p) };
                }
            }
            Tool::Table { kind, view: Some(v), origin } => {
                let id = self.current_sheet().next_table_id();
                let (mut table, name) = noshape_scene::drawing_tools::view_table(id, kind, v, origin, c);
                // The Hole table dialog's choices.
                self.apply_hole_table_dlg(&mut table);
                self.edit_sheet(name, cx, |s| s.tables.push(table));
                self.tool = Tool::Select;
                self.selection = vec![Sel::Table(id)];
            }
            Tool::Datum { anchor: None } => {
                // On a GD&T frame edge or a dimension's lines (the red
                // crosshair), else on the geometry.
                let on = self.datum_attach_at(c);
                self.datum_on = on.map(|o| o.1);
                // On a view's geometry: kept as that view point (it follows the view).
                let pp = if on.is_none() { self.any_point(c) } else { None };
                self.datum_at = pp.as_ref().map(|s| (s.view, s.p));
                let p = on.map(|o| o.0).or_else(|| pp.as_ref().and_then(|s| self.pick_sheet(s))).unwrap_or(c);
                self.tool = Tool::Datum { anchor: Some(p) };
            }
            Tool::Datum { anchor: Some(_) } => {
                if let Some(a) = self.pending_annotation() {
                    self.edit_sheet("Datum", cx, |s| s.annotations.push(a));
                    // Help datum: the tool stays active for more datums.
                    self.tool = Tool::Datum { anchor: None };
                    self.datum_on = None;
                    self.datum_at = None;
                    self.datum_placed(window, cx);
                }
            }
            // Keep placing until ✓ (help surface_finish).
            Tool::SurfaceFinish => {
                if !matches!(self.panel, Some(Panel::SurfaceFinish { edit: None, .. })) {
                    self.open_surface_finish_panel(None, c, noshape_scene::drawing_tools::SURFACE_FINISH_DEFAULT, &Default::default(), window, cx);
                }
                self.place_surface_finish(c, cx);
            }
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------ drags

    /// The grips of the one selected note: sides (resize) and, without
    /// leaders, the middle (rotate), on the sheet.
    pub fn note_grips(&self) -> Option<(u64, Vec<NoteGrip>)> {
        if !matches!(self.tool, Tool::Select) {
            return None;
        }
        let [Sel::Annotation(id)] = self.selection.as_slice() else { return None };
        // None while the note is edited in place (live 225).
        if self.inplace_hidden == Some(*id) {
            return None;
        }
        let a = self.current_sheet().annotations.iter().find(|a| a.id == *id)?;
        let (b, angle) = note_box(a)?;
        let AnnotationKind::Note { style, .. } = &a.kind else { return None };
        let mid = noshape_model::drawing::note::center(&b);
        let rot = |p: [f64; 2]| {
            let (s, co) = angle.sin_cos();
            let d = sub(p, mid);
            [mid[0] + d[0] * co - d[1] * s, mid[1] + d[0] * s + d[1] * co]
        };
        // Side squares at the text ends; free notes also rotate from a grip
        // about 20 px above the text (live drawing/234).
        let mut out = vec![(0, rot([b[0], mid[1]])), (1, rot([b[2], mid[1]]))];
        if style.leaders.is_empty() {
            out.push((2, rot([mid[0], b[3] + 20.0 / self.zoom.max(1e-6)])));
        }
        Some((*id, out))
    }

    /// The selected note's grips as live drawing/229b and 234 draw them: 10 px
    /// squares (#cc925c, #e6c9ae edge) at the text ends; free notes add a
    /// round grip at the bottom centre and the rotate grip (a circle with two
    /// arrowheads on a short stem) above the text.
    pub fn note_grip_overlay(&self, o: &mut crate::drawing_scene::Overlay) {
        // Rotating: the note inside a dotted outline (live 235 / 236).
        if let Drag::NoteGrip { id, grip: 2, .. } = &self.drag
            && let Some(a) = self.display().sheets.get(self.sheet).and_then(|s| s.annotations.iter().find(|a| a.id == *id))
            && let Some((b, angle)) = note_box(a)
        {
            let mid = noshape_model::drawing::note::center(&b);
            let (sn, co) = angle.sin_cos();
            let rot = |p: [f64; 2]| {
                let d = sub(p, mid);
                [mid[0] + d[0] * co - d[1] * sn, mid[1] + d[0] * sn + d[1] * co]
            };
            // Black dots, 2 px on / 2 px off.
            let k = 1.0 / self.zoom.max(1e-6);
            let corners = [rot([b[0], b[1]]), rot([b[2], b[1]]), rot([b[2], b[3]]), rot([b[0], b[3]]), rot([b[0], b[1]])];
            for w in corners.windows(2) {
                let (d, n) = (sub(w[1], w[0]), len(sub(w[1], w[0])));
                let steps = (n / (4.0 * k)).floor() as usize;
                for i in 0..steps {
                    let t0 = i as f64 * 4.0 * k / n;
                    let t1 = (t0 + 2.0 * k / n).min(1.0);
                    o.tinted.push((vec![[w[0][0] + d[0] * t0, w[0][1] + d[1] * t0], [w[0][0] + d[0] * t1, w[0][1] + d[1] * t1]], [0, 0, 0, 255], 1.0));
                }
            }
            return;
        }
        let Some((id, grips)) = self.note_grips() else { return };
        let k = 1.0 / self.zoom.max(1e-6);
        const TAN: [u8; 4] = [0xcc, 0x92, 0x5c, 255];
        const EDGE: [u8; 4] = [0xe6, 0xc9, 0xae, 255];
        let ring = |c: [f64; 2], r: f64| -> Vec<[f64; 2]> {
            (0..=24).map(|i| {
                let t = i as f64 / 24.0 * std::f64::consts::TAU;
                [c[0] + r * t.cos(), c[1] + r * t.sin()]
            }).collect()
        };
        for (g, p) in &grips {
            match g {
                0 | 1 => {
                    let h = 5.0 * k;
                    o.tinted.push((vec![[p[0] - h, p[1]], [p[0] + h, p[1]]], TAN, 10.0));
                    o.tinted.push((vec![[p[0] - h, p[1] - h], [p[0] + h, p[1] - h], [p[0] + h, p[1] + h], [p[0] - h, p[1] + h], [p[0] - h, p[1] - h]], EDGE, 1.0));
                }
                _ => {
                    // The rotate circle, its two arrowheads and the stem dot.
                    o.tinted.push((ring(*p, 5.5 * k), TAN, 1.5));
                    for s in [-1.0, 1.0] {
                        let x = p[0] + s * 7.5 * k;
                        o.tinted.push((vec![[x - s * 2.5 * k, p[1] + 2.5 * k], [x + s * 1.0 * k, p[1] - 0.5 * k], [x - s * 2.5 * k, p[1] - 2.5 * k]], TAN, 2.0));
                    }
                    o.tinted.push((vec![[p[0], p[1] - 10.0 * k], [p[0], p[1] - 13.0 * k]], TAN, 3.0));
                }
            }
        }
        // Free notes: the round grip at the bottom centre of the text.
        if grips.iter().any(|(g, _)| *g == 2)
            && let Some(a) = self.current_sheet().annotations.iter().find(|a| a.id == id)
            && let Some((b, _)) = note_box(a)
        {
            let c = [(b[0] + b[2]) / 2.0, b[1]];
            o.tinted.push((vec![[c[0] - 2.5 * k, c[1]], [c[0] + 2.5 * k, c[1]]], TAN, 9.0));
            o.tinted.push((ring(c, 5.0 * k), [255, 255, 255, 255], 1.2));
        }
    }

    /// A drag starting on a note grip.
    pub fn note_grip_drag(&self, start: [f64; 2]) -> Option<Drag> {
        let (id, grips) = self.note_grips()?;
        let tol = self.tol_mm(6.0);
        let (grip, _) = grips.into_iter().find(|(_, p)| len(sub(*p, start)) < tol)?;
        let orig = self.current_sheet().annotations.iter().find(|a| a.id == id)?.clone();
        Some(Drag::NoteGrip { id, grip, start, orig })
    }

    pub fn extra_start_drag(&mut self, pick: Option<u32>, start: [f64; 2]) -> Option<Drag> {
        // A snap point of a selected auxiliary view's reference indicator.
        let aux = self.aux_dot_views().into_iter().find_map(|v| Some((v, self.aux_grip_at(v, start)?)));
        if let Some((view, grip)) = aux {
            let orig = self.current_sheet().view(view)?.aux_ref.clone()?;
            return Some(Drag::AuxIndicator { view, grip, orig });
        }
        // A selected table's grips, wherever the pick lands (live 1607 (b)):
        // the resize midpoints, then the corners (a move).
        let tol = self.tol_mm(9.0);
        for s in &self.selection {
            let Sel::Table(id) = *s else { continue };
            let (Some(t), Some(l)) = (self.current_sheet().table(id).cloned(), self.built.as_ref().and_then(|b| b.table_layout(id)).cloned()) else { continue };
            if let Some(axis) = l.resize_grips(t.corner).iter().position(|g| len(sub(start, *g)) < tol) {
                let widths: Vec<f64> = l.xs.windows(2).map(|w| w[1] - w[0]).collect();
                let size = [l.max()[0] - l.min()[0], l.max()[1] - l.min()[1]];
                return Some(Drag::ResizeTable { id, start, orig: t, size, widths, axis });
            }
            if l.corners().iter().any(|g| len(sub(start, *g)) < tol) {
                return Some(Drag::MoveTable { id, start, orig: t.position });
            }
        }
        let target = self.built.as_ref()?.picks.get(pick? as usize)?.clone();
        let sheet = self.drawing.sheets.get(self.sheet)?;
        match target {
            PickTarget::Table(id) => {
                let t = sheet.table(id)?.clone();
                let l = self.built.as_ref()?.table_layout(id)?.clone();
                // A selected table's grips (live 1607 (b)): the edge midpoints
                // away from the fixed corner resize, the corners move it.
                let tol = self.tol_mm(9.0);
                let mid = if self.selection.contains(&Sel::Table(id)) { l.resize_grips(t.corner).iter().position(|g| len(sub(start, *g)) < tol) } else { None };
                let near_grip = mid.is_some() || l.corners().iter().any(|g| len(sub(start, *g)) < tol);
                // A selected table: dragging a divider resizes its cells.
                // Inside a cell (away from its lines): drag-select cells; the
                // lines and grips move or resize the table.
                if !near_grip && l.cell_at(start).is_some() && !self.on_table_line(&l, start) {
                    self.cell_sel = None;
                    return Some(Drag::SelectCells { id, start });
                }
                if self.selection.contains(&Sel::Table(id))
                    && !near_grip
                    && let Some((col, row)) = self.table_divider_at(&l, start)
                {
                    let widths: Vec<f64> = l.xs.windows(2).map(|w| w[1] - w[0]).collect();
                    let heights: Vec<f64> = l.ys.windows(2).skip(l.title as usize).map(|w| w[0] - w[1]).collect();
                    return Some(Drag::TableDivider { id, col, row, start, widths, heights });
                }
                self.selection = vec![Sel::Table(id)];
                if let Some(axis) = mid {
                    let widths: Vec<f64> = l.xs.windows(2).map(|w| w[1] - w[0]).collect();
                    let size = [l.max()[0] - l.min()[0], l.max()[1] - l.min()[1]];
                    Some(Drag::ResizeTable { id, start, orig: t, size, widths, axis })
                } else {
                    Some(Drag::MoveTable { id, start, orig: t.position })
                }
            }
            // A break line: dragging moves the break (help V35).
            PickTarget::BreakLine { view, index } => {
                let orig = sheet.view(view)?.breaks.get(index)?.clone();
                self.selection = vec![Sel::Break(view, index)];
                Some(Drag::MoveBreak { view, index, start, orig })
            }
            PickTarget::SectionLine(v) => {
                self.selection = vec![Sel::View(v)];
                // A line following its model point is dragged where it is
                // drawn (live 1625).
                let drawn = self.built.as_ref().map(|b| &b.resolved).filter(|s| s.view(v).is_some()).unwrap_or(sheet);
                self.cut_drag = Some((v, drawn.view(v).is_some_and(|x| x.cut_lost), drawn.section_points(v).unwrap_or_default()));
                // Near a point of the line: move that point only.
                if let Some(index) = self.section_point_at(v, start) {
                    return Some(Drag::MoveSectionPoint { view: v, index, orig: drawn.section_points(v)? });
                }
                Some(Drag::MoveSectionLine { view: v, start, orig: drawn.view(v)?.kind.clone() })
            }
            PickTarget::AuxIndicator(v) => {
                let grip = self.aux_grip_at(v, start)?;
                Some(Drag::AuxIndicator { view: v, grip, orig: sheet.view(v)?.aux_ref.clone()? })
            }
            PickTarget::DetailCircle(v) => {
                self.selection = vec![Sel::View(v)];
                // Live asbv-8/9/10: the grip on the circle resizes it, the rest
                // moves it.
                if self.detail_grip(v).is_some_and(|(_, _, g)| (g[0] - start[0]).hypot(g[1] - start[1]) < self.tol_mm(5.0)) {
                    return Some(Drag::ResizeDetailCircle { view: v });
                }
                // The label on its leader (Add leader) drags on its own.
                if let (Some(o), Some((c, _, _))) = (sheet.view(v)?.detail_label_at, self.detail_grip_any(v))
                    && (c[0] + o[0] - start[0]).hypot(c[1] + o[1] - start[1]) < self.tol_mm(14.0)
                {
                    return Some(Drag::MoveDetailLabel { view: v, start, orig: o });
                }
                match sheet.view(v)?.kind {
                    ViewKind::Detail { center, .. } => Some(Drag::MoveDetailCircle { view: v, start, orig: center }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `p` is on one of a table's lines (outline or divider).
    pub fn on_table_line(&self, l: &noshape_model::drawing::tables::TableLayout, p: [f64; 2]) -> bool {
        let tol = self.tol_mm(4.0);
        l.xs.iter().any(|x| (p[0] - x).abs() < tol) || l.ys.iter().any(|y| (p[1] - y).abs() < tol)
    }

    /// The divider of a table layout under `p`: (column boundary, row
    /// boundary) indices (0 and the last are the outer edges; the left and
    /// top edges move the table instead).
    pub fn table_divider_at(&self, l: &noshape_model::drawing::tables::TableLayout, p: [f64; 2]) -> Option<(Option<usize>, Option<usize>)> {
        let tol = self.tol_mm(4.0);
        let (lo, hi) = (l.min(), l.max());
        let inside_y = p[1] >= lo[1] - tol && p[1] <= hi[1] + tol;
        let inside_x = p[0] >= lo[0] - tol && p[0] <= hi[0] + tol;
        let col = l.xs.iter().enumerate().skip(1).find(|(_, x)| (p[0] - **x).abs() < tol && inside_y).map(|(i, _)| i);
        let t0 = l.title as usize;
        // Row boundaries below the title row.
        let row = l.ys.iter().enumerate().skip(t0 + 1).find(|(_, y)| (p[1] - **y).abs() < tol && inside_x).map(|(i, _)| i - t0);
        (col.is_some() || row.is_some()).then_some((col, row))
    }

    /// A selected table's grips (live 1607 (b)): round on the corners (the
    /// fixed one black), square on the two resize midpoints; while a corner
    /// sits on the frame's corner, the red snap square with a "+".
    pub fn table_grip_overlay(&self, o: &mut crate::drawing_scene::Overlay) {
        if !matches!(self.tool, Tool::Select) {
            return;
        }
        let Some(b) = self.built.as_ref() else { return };
        let k = 1.0 / self.zoom.max(1e-6);
        const TAN: [u8; 4] = [0xcc, 0x92, 0x5c, 255];
        for s in &self.selection {
            let Sel::Table(id) = s else { continue };
            let (Some(t), Some(l)) = (self.current_sheet().table(*id), b.table_layout(*id)) else { continue };
            let fixed = l.fixed_corner(t.corner);
            for c in l.corners() {
                o.dots.push((c, if c == fixed { [0, 0, 0, 255] } else { TAN }));
            }
            for g in l.resize_grips(t.corner) {
                let h = 3.5 * k;
                o.tinted.push((vec![[g[0] - h, g[1]], [g[0] + h, g[1]]], TAN, 7.0));
            }
        }
        if let Drag::MoveTable { id, .. } = &self.drag
            && let Some(l) = b.table_layout(*id)
        {
            let [x0, y0, x1, y1] = self.current_sheet().size.frame_rect();
            for f in [[x0, y1], [x1, y1], [x0, y0], [x1, y0]] {
                if l.corners().iter().any(|c| len(sub(*c, f)) < 1e-6) {
                    let h = 5.0 * k;
                    const RED: [u8; 4] = [0xdc, 0x28, 0x28, 255];
                    o.tinted.push((vec![[f[0] - h, f[1] - h], [f[0] + h, f[1] - h], [f[0] + h, f[1] + h], [f[0] - h, f[1] + h], [f[0] - h, f[1] - h]], RED, 1.5));
                    o.tinted.push((vec![[f[0] - h, f[1]], [f[0] + h, f[1]]], RED, 1.5));
                    o.tinted.push((vec![[f[0], f[1] - h], [f[0], f[1] + h]], RED, 1.5));
                }
            }
        }
    }

    /// Double-click on a selected table's resize grip (live 1607 (b)): the
    /// columns (or rows) fit their content, the fixed corner stays.
    pub fn fit_table_grip(&mut self, id: u64, cx: &mut Context<Self>) -> bool {
        let Some(l) = self.built.as_ref().and_then(|b| b.table_layout(id)).cloned() else { return false };
        let Some(t) = self.current_sheet().table(id).cloned() else { return false };
        let tol = self.tol_mm(9.0);
        let Some(axis) = l.resize_grips(t.corner).iter().position(|g| len(sub(self.cursor, *g)) < tol) else { return false };
        let data = noshape_model::drawing::tables::table_data_with(&self.drawing, &t, None, None, None, &self.info, &|s| s.to_string());
        let auto = noshape_model::drawing::tables::auto_widths(&t, &data);
        self.edit_table(id, "Fit to content", cx, |t| {
            if axis == 0 {
                t.col_widths = auto;
            } else {
                t.row_heights.clear();
                // A new table's rows for its text (7 mm at 3 mm).
                t.row_height = t.text_height * 7.0 / 3.0;
            }
        });
        true
    }

    pub fn extra_drag_move(&mut self, drag: &Drag, cx: &mut Context<Self>) {
        let c = self.cursor;
        let aux = if let Drag::AuxIndicator { view, .. } = drag { self.aux_indicator_of(*view) } else { None };
        let mut dr = self.drawing.clone();
        let si = self.sheet;
        let Some(sheet) = dr.sheets.get_mut(si) else { return };
        match drag {
            Drag::NoteGrip { id, grip, start, orig } => {
                let Some(a) = sheet.annotations.iter_mut().find(|a| a.id == *id) else { return };
                *a = orig.clone();
                let Some((bbox, angle)) = note_box(orig) else { return };
                let mid = noshape_model::drawing::note::center(&bbox);
                // The cursor in the note's unrotated frame.
                let local = |p: [f64; 2]| {
                    let (s, co) = (-angle).sin_cos();
                    let d = sub(p, mid);
                    [mid[0] + d[0] * co - d[1] * s, mid[1] + d[0] * s + d[1] * co]
                };
                let AnnotationKind::Note { style, height_mm, .. } = &mut a.kind else { return };
                let min = *height_mm * 2.0;
                match grip {
                    1 => style.width = Some((local(c)[0] - bbox[0]).max(min)),
                    0 => {
                        let x = local(c)[0].min(bbox[2] - min);
                        style.width = Some(bbox[2] - x);
                        a.position[0] += x - bbox[0];
                    }
                    _ => {
                        let ang = |p: [f64; 2]| (p[1] - mid[1]).atan2(p[0] - mid[0]);
                        // Whole degrees, as the live angle readout ("335deg").
                        style.angle = (angle + ang(c) - ang(*start)).to_degrees().round().to_radians();
                    }
                }
            }
            Drag::SelectCells { .. } => {}
            Drag::MoveTable { id, start, orig } => {
                let mut pos = add(*orig, sub(c, *start));
                // A corner near the sheet frame's corner snaps onto it (live
                // 1607 (b)).
                if let Some(l) = self.built.as_ref().and_then(|b| b.table_layout(*id))
                    && let Some(t0) = self.current_sheet().table(*id)
                {
                    let d = sub(pos, t0.position);
                    let [x0, y0, x1, y1] = self.current_sheet().size.frame_rect();
                    let tol = self.tol_mm(10.0);
                    let snap = l.corners().iter().flat_map(|k| [[x0, y1], [x1, y1], [x0, y0], [x1, y0]].map(|f| sub(f, add(*k, d)))).filter(|v| len(*v) < tol).min_by(|a, b| len(*a).total_cmp(&len(*b)));
                    if let Some(v) = snap {
                        pos = add(pos, v);
                    }
                }
                if let Some(t) = sheet.table_mut(*id) {
                    t.position = pos;
                }
            }
            Drag::TableDivider { id, col, row, start, widths, heights } => {
                // Live 1613 (b): a dragged grid line resizes only the column
                // (row) on the fixed corner's side of it (fixed top right: the
                // column right of a vertical line, the row above a horizontal
                // one); the table grows, the others keep their size and move.
                // An outer line with nothing on that side resizes the other.
                let d = sub(c, *start);
                let min = 3.0;
                if let Some(t) = sheet.table_mut(*id) {
                    let fixed_right = matches!(t.corner, TableCorner::TopRight | TableCorner::BottomRight);
                    let fixed_top = matches!(t.corner, TableCorner::TopLeft | TableCorner::TopRight);
                    if let Some(i) = *col {
                        let mut w = widths.clone();
                        // (column, its change): right of the line −dx, left of it +dx.
                        let right = (i < w.len()).then_some((i, -d[0]));
                        let left = (i > 0).then_some((i.wrapping_sub(1), d[0]));
                        if let Some((k, dw)) = if fixed_right { right.or(left) } else { left.or(right) } {
                            w[k] = (w[k] + dw).max(min);
                        }
                        t.col_widths = w;
                    }
                    if let Some(j) = *row {
                        let mut h = heights.clone();
                        // (row, its change): above the line −dy, below it +dy (sheet y up).
                        let above = (j > 0).then_some((j.wrapping_sub(1), -d[1]));
                        let below = (j < h.len()).then_some((j, d[1]));
                        if let Some((k, dh)) = if fixed_top { above.or(below) } else { below.or(above) } {
                            h[k] = (h[k] + dh).max(min);
                        }
                        t.row_heights = h;
                    }
                }
            }
            Drag::ResizeTable { id, start, orig, size, widths, axis } => {
                // One direction: every column (or row) scales by the same
                // factor, the fixed corner stays (live 1607 (b)).
                let d = sub(c, *start);
                let sx = if matches!(orig.corner, TableCorner::TopLeft | TableCorner::BottomLeft) { 1.0 } else { -1.0 };
                let sy = if matches!(orig.corner, TableCorner::TopLeft | TableCorner::TopRight) { -1.0 } else { 1.0 };
                if let Some(t) = sheet.table_mut(*id) {
                    if *axis == 0 {
                        let kx = ((size[0] + d[0] * sx) / size[0]).max(0.3);
                        t.col_widths = widths.iter().map(|w| w * kx).collect();
                    } else {
                        let ky = ((size[1] + d[1] * sy) / size[1]).max(0.5);
                        t.row_height = (orig.row_height * ky).max(orig.text_height * 1.4);
                        t.row_heights = orig.row_heights.iter().map(|h| (h * ky).max(orig.text_height * 1.4)).collect();
                    }
                }
            }
            Drag::MoveBreak { view, index, start, orig } => {
                // The break slides along the part (its band keeps its width),
                // so its lines follow the pointer.
                let Some(pl) = self.placement(*view) else { return };
                let nrm = pl.dir_to_sheet(if orig.vertical { [1.0, 0.0] } else { [0.0, 1.0] });
                let dm = dot(sub(c, *start), nrm) / (pl.scale * 1000.0);
                if let Some(b) = sheet.view_mut(*view).and_then(|v| v.breaks.get_mut(*index)) {
                    (b.a, b.b) = (orig.a + dm, orig.b + dm);
                }
            }
            Drag::MoveSectionLine { view, start, orig } => {
                let Some(parent) = sheet.parent_of(*view) else { return };
                let Some(ppl) = self.placement(parent) else { return };
                let dv = sub(ppl.to_view(c), ppl.to_view(*start));
                let kind = match orig.clone() {
                    ViewKind::Section { parent, line, label, cuts } if line.len() >= 2 => {
                        let e = norm(sub(line[1], line[0]));
                        let n = [-e[1], e[0]];
                        let t = dot(dv, n);
                        ViewKind::Section { parent, line: line.iter().map(|p| add(*p, mul(n, t))).collect(), label, cuts: cuts.iter().map(|p| add(*p, mul(n, t))).collect() }
                    }
                    ViewKind::AlignedSection { parent, center, a, b, label } => ViewKind::AlignedSection { parent, center: add(center, dv), a: add(a, dv), b: add(b, dv), label },
                    k => k,
                };
                if let Some(v) = sheet.view_mut(*view) {
                    v.kind = kind;
                }
            }
            Drag::MoveViewLabel { view, start, orig } => {
                let o = orig.unwrap_or_default();
                if let Some(v) = sheet.view_mut(*view) {
                    v.label_at = Some([o[0] + c[0] - start[0], o[1] + c[1] - start[1]]);
                }
            }
            Drag::AuxIndicator { view, grip, orig } => {
                let Some(ind) = aux else { return };
                Self::drag_aux_grip(sheet, *view, *grip, orig, &ind, c, self.drawing.props.view_tab.aux_arrow_format);
            }
            Drag::MoveSectionPoint { view, index, orig } => {
                let Some(parent) = sheet.parent_of(*view) else { return };
                let Some(ppl) = self.placement(parent) else { return };
                let p = self.snap_at(c).filter(|s| s.view == parent).map(|s| s.p).unwrap_or_else(|| ppl.to_view(c));
                let ortho = matches!(sheet.view(*view).map(|v| &v.kind), Some(ViewKind::Section { .. }));
                sheet.set_section_points(*view, views::move_cut_point(orig, *index, p, ortho));
            }
            Drag::MoveDetailCircle { view, start, orig } => {
                let Some(parent) = sheet.parent_of(*view) else { return };
                let Some(ppl) = self.placement(parent) else { return };
                let dv = sub(ppl.to_view(c), ppl.to_view(*start));
                if let Some(ViewKind::Detail { center, .. }) = sheet.view_mut(*view).map(|v| &mut v.kind) {
                    *center = add(*orig, dv);
                }
            }
            Drag::MoveDetailLabel { view, start, orig } => {
                if let Some(x) = sheet.view_mut(*view) {
                    x.detail_label_at = Some(add(*orig, sub(c, *start)));
                }
            }
            Drag::ResizeDetailCircle { view } => {
                // The radius follows the pointer; the detail view grows with it.
                let Some(parent) = sheet.parent_of(*view) else { return };
                let Some(ppl) = self.placement(parent) else { return };
                let p = ppl.to_view(c);
                if let Some(ViewKind::Detail { center, radius, .. }) = sheet.view_mut(*view).map(|v| &mut v.kind) {
                    *radius = (p[0] - center[0]).hypot(p[1] - center[1]).max(1e-4);
                }
            }
            _ => return,
        }
        // A line following its model point: the preview as it will be.
        if let Drag::MoveSectionPoint { view, .. } | Drag::MoveSectionLine { view, .. } = drag {
            self.rebase_cut_anchor(&mut dr, *view);
        }
        self.set_preview(Some(dr), cx);
    }

    // ------------------------------------------------------------ menus

    pub fn view_extra_entries(&self, id: ViewId, cx: &App) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let sheet = &self.drawing.sheets[self.sheet];
        let Some(v) = sheet.view(id).cloned() else { return out };
        let mut crop = vec![
            MenuEntry::item("Rectangle", move |t, _, cx| t.set_tool(Tool::Crop { parent: Some(id), first: None, circle: false }, cx)),
            MenuEntry::item("Circle", move |t, _, cx| t.set_tool(Tool::Crop { parent: Some(id), first: None, circle: true }, cx)),
            MenuEntry::item("Polygon", move |t, _, cx| t.set_tool(Tool::CropPoly { parent: Some(id), points: vec![], spline: false }, cx)),
            MenuEntry::item("Spline", move |t, _, cx| t.set_tool(Tool::CropPoly { parent: Some(id), points: vec![], spline: true }, cx)),
        ];
        if v.crop.is_some() {
            crop.push(MenuEntry::item("Remove crop", move |t, _, cx| t.edit_view("Remove crop", id, cx, |v| v.crop = None)));
        }
        out.push(MenuEntry::Submenu("Crop".into(), crop));
        let mut brk = vec![
            MenuEntry::item("Vertical break lines", move |t, _, cx| t.set_tool(Tool::Break { parent: Some(id), first: None, vertical: true }, cx)),
            MenuEntry::item("Horizontal break lines", move |t, _, cx| t.set_tool(Tool::Break { parent: Some(id), first: None, vertical: false }, cx)),
        ];
        if !v.breaks.is_empty() {
            brk.push(MenuEntry::item("Remove all breaks", move |t, _, cx| t.edit_view("Remove breaks", id, cx, |v| v.breaks.clear())));
        }
        out.push(MenuEntry::Submenu("Break".into(), brk));
        out.push(MenuEntry::Submenu(
            "More section views".into(),
            vec![
                MenuEntry::item("Jogged section view", move |t, _, cx| t.set_tool(Tool::JoggedSection { parent: Some(id), points: vec![], done: false }, cx)),
                MenuEntry::item("Aligned section view", move |t, _, cx| t.set_tool(Tool::AlignedSection { parent: Some(id), center: None, a: None, b: None }, cx)),
                MenuEntry::item("Partial section view", move |t, _, cx| t.set_tool(Tool::Section { parent: Some(id), p0: None, line: None, partial: true, jogs: vec![] }, cx)),
                MenuEntry::item("Broken-out section", move |t, _, cx| t.set_tool(Tool::BrokenOut { parent: Some(id), points: vec![], done: false }, cx)),
            ],
        ));
        if matches!(v.kind, ViewKind::Section { .. } | ViewKind::AlignedSection { .. }) {
            out.push(MenuEntry::item("Flip section direction", move |t, _, cx| t.flip_section(id, cx)));
        }
        // Source-dependent entries (root base view).
        if let Some(root) = views::root_view(sheet, id).and_then(|r| sheet.view(r)).cloned()
            && let ViewKind::Base { source, config, .. } = &root.kind
        {
            let rid = root.id;
            let doc = &self.doc.read(cx).session.doc;
            match doc.tab(*source).map(|t| &t.content) {
                Some(TabContent::PartStudio(ps)) if !ps.configuration.inputs.is_empty() => {
                    let mut cfg_entries = Vec::new();
                    for input in &ps.configuration.inputs {
                        let iid = input.id.clone();
                        match &input.kind {
                            noshape_model::configuration::ConfigInputKind::List { options, default } => {
                                let cur = config.0.get(&iid).cloned().unwrap_or(default.clone());
                                let opts: Vec<MenuEntry> = options
                                    .iter()
                                    .map(|o| {
                                        let (iid, oid) = (iid.clone(), o.id.clone());
                                        MenuEntry::check(o.name.clone(), cur == o.id, move |t, _, cx| t.set_view_config(rid, &iid, &oid, cx))
                                    })
                                    .collect();
                                cfg_entries.push(MenuEntry::Submenu(input.name.clone().into(), opts));
                            }
                            noshape_model::configuration::ConfigInputKind::Checkbox { default } => {
                                let on = config.0.get(&iid).map(|v| v == "true").unwrap_or(*default);
                                cfg_entries.push(MenuEntry::check(input.name.clone(), on, move |t, _, cx| t.set_view_config(rid, &iid, if on { "false" } else { "true" }, cx)));
                            }
                            _ => {}
                        }
                    }
                    if !cfg_entries.is_empty() {
                        out.push(MenuEntry::Submenu("Configuration".into(), cfg_entries));
                    }
                }
                Some(TabContent::Assembly(asm)) => {
                    // Live drawing/1109: Explode/Position ▸ Default | explodes |
                    // positions, Display state ▸ Show all | states; the current
                    // one greyed. A child view gets its own choice.
                    let key = noshape_model::drawing::views::view_source(sheet, id).map(|k| k.0);
                    let cur = match key.as_ref().map(|k| (k.explode, k.position)) {
                        Some((Some(e), _)) => AsmPosition::Explode(e),
                        Some((None, Some(p))) => AsmPosition::Position(p),
                        _ => AsmPosition::Default,
                    };
                    // Section and detail views always follow their parent (help H:view_context_menu).
                    let target = if matches!(v.kind, ViewKind::Section { .. } | ViewKind::AlignedSection { .. } | ViewKind::Detail { .. }) { rid } else { id };
                    let is_root = rid == target;
                    let mut ex = Vec::new();
                    let mut last_group = 0;
                    for (name, p) in crate::drawing_insert::asm_positions(asm) {
                        let group = match p {
                            AsmPosition::Default => 0,
                            AsmPosition::Explode(_) => 1,
                            AsmPosition::Position(_) => 2,
                        };
                        if group != last_group {
                            ex.push(MenuEntry::Separator);
                            last_group = group;
                        }
                        ex.push(
                            MenuEntry::item(name, move |t, _, cx| {
                                t.edit_view("Explode/Position", target, cx, |v| if is_root { v.set_asm_position(p) } else { v.own_position = Some(p) })
                            })
                            .disabled(p == cur),
                        );
                    }
                    out.push(MenuEntry::Submenu("Exploded view".into(), ex));
                    let cur_d = key.and_then(|k| k.display);
                    let mut ds = vec![
                        MenuEntry::item("Show all", move |t, _, cx| t.edit_view("Display state", target, cx, |v| if is_root { v.display_state = None } else { v.own_display = Some(DisplayChoice::ShowAll) }))
                            .disabled(cur_d.is_none()),
                    ];
                    for d in &asm.display_states {
                        let n = d.name.clone();
                        ds.push(
                            MenuEntry::item(d.name.clone(), move |t, _, cx| {
                                let n = n.clone();
                                t.edit_view("Display state", target, cx, |v| if is_root { v.display_state = Some(n) } else { v.own_display = Some(DisplayChoice::State(n)) })
                            })
                            .disabled(cur_d.as_ref() == Some(&d.name)),
                        );
                    }
                    out.push(MenuEntry::Submenu("Display state".into(), ds));
                    out.push(MenuEntry::item("Insert BOM table", move |t, _, cx| t.set_tool(Tool::Table { kind: TableTool::Bom, view: Some(id), origin: None }, cx)));
                }
                _ => {}
            }
        }
        out.push(MenuEntry::item("Insert hole table", move |t, _, cx| t.set_tool(Tool::Table { kind: TableTool::Hole, view: Some(id), origin: None }, cx)));
        out
    }

    pub fn set_view_config(&mut self, view: ViewId, input: &str, value: &str, cx: &mut Context<Self>) {
        let (input, value) = (input.to_string(), value.to_string());
        self.edit_view("View configuration", view, cx, |v| {
            if let ViewKind::Base { config, .. } = &mut v.kind {
                config.0.insert(input, value);
            }
        });
        self.ensure_sources(false, cx);
    }

    /// Move a section view to the other side of its cutting line.
    /// Section line › Flip direction: the arrows and labels swap sides and
    /// the view regenerates in place (Onshape).
    pub fn flip_section(&mut self, id: ViewId, cx: &mut Context<Self>) {
        self.edit_view("Flip direction", id, cx, |v| v.flip = !v.flip);
    }

    pub fn annotation_extra_entries(&self, a: &Annotation) -> Vec<MenuEntry> {
        let mut out = self.callout_align_entries(a);
        let a2 = a.clone();
        if a.kind.is_dimension() {
            out.push(MenuEntry::item("Edit...", move |t, w, cx| t.open_annotation_panel(&a2, w, cx)));
        } else if matches!(a.kind, AnnotationKind::Gdt { .. } | AnnotationKind::Datum { .. } | AnnotationKind::SurfaceFinish { .. } | AnnotationKind::Balloon { .. }) {
            out.push(MenuEntry::item("Edit...", move |t, w, cx| t.open_annotation_panel(&a2, w, cx)));
        }
        // With a BOM table in the drawing: Add stacked callouts….
        if matches!(a.kind, AnnotationKind::Balloon { .. }) && self.drawing.sheets.iter().flat_map(|s| &s.tables).any(|t| matches!(t.kind, TableKind::Bom { .. })) {
            let id = a.id;
            out.push(MenuEntry::item("Add stacked callouts...", move |t, _, cx| t.open_stack_dlg(id, cx)));
        }
        out
    }

    /// Several callouts selected (help callout_balloon): Align horizontally /
    /// Align vertically to the clicked one `a`, then a separator.
    pub fn callout_align_entries(&self, a: &Annotation) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let sheet = &self.drawing.sheets[self.sheet];
        let callouts: Vec<u64> = self.selection.iter().filter_map(|s| if let Sel::Annotation(id) = s { Some(*id) } else { None }).filter(|id| sheet.annotations.iter().any(|x| x.id == *id && matches!(x.kind, AnnotationKind::Balloon { .. }))).collect();
        if callouts.len() >= 2 && matches!(a.kind, AnnotationKind::Balloon { .. }) {
            let at = a.position;
            for (label, axis) in [("Align horizontally", 1usize), ("Align vertically", 0usize)] {
                let ids = callouts.clone();
                out.push(MenuEntry::item(label, move |t, _, cx| {
                    t.edit_sheet(label, cx, |s| {
                        for x in s.annotations.iter_mut().filter(|x| ids.contains(&x.id)) {
                            x.position[axis] = at[axis];
                        }
                    })
                }));
            }
            out.push(MenuEntry::Separator);
        }
        out
    }

    pub fn table_entries(&self, id: u64, cx: &App) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let sheet = &self.drawing.sheets[self.sheet];
        let Some(t) = sheet.table(id).cloned() else { return out };
        // General tables with no cell selected: the whole table's menu.
        if matches!(t.kind, TableKind::General { .. }) && self.cell_sel.is_none_or(|c| c.table != id) {
            return self.data_table_entries(id, cx);
        }
        // BOM, cut list, hole and custom tables: Onshape's menus — the
        // cells' (Size, Resize) with cells selected, else the whole table's
        // (qa HOLE-TABLE-MENU: hole tables had the general table's).
        if matches!(t.kind, TableKind::Bom { .. } | TableKind::CutList { .. } | TableKind::Hole { .. } | TableKind::Custom { .. }) {
            return match self.cell_sel.filter(|c| c.table == id) {
                Some(cs) => self.cell_size_entries(&t, cs),
                None => self.data_table_entries(id, cx),
            };
        }
        let cell = self.built.as_ref().and_then(|b| b.table_layout(id)).and_then(|l| l.cell_at(self.cursor));
        // General table with cells selected: Onshape's cell menu.
        if let (TableKind::General { .. }, Some(cs)) = (&t.kind, self.cell_sel.filter(|c| c.table == id)) {
            return self.general_cell_entries(&t, cs);
        }
        if let TableKind::Revision { .. } = t.kind {
            out.extend(self.revision_table_entries(id, cell));
        }
        out.extend(self.table_more_entries(&t, cell));
        if let TableKind::CutList { bottom_to_top, .. } = t.kind {
            out.push(MenuEntry::Submenu(
                "Order".into(),
                vec![
                    MenuEntry::check("Top to bottom", !bottom_to_top, move |t, _, cx| t.edit_table(id, "Cut list order", cx, |t| if let TableKind::CutList { bottom_to_top, .. } = &mut t.kind { *bottom_to_top = false })),
                    MenuEntry::check("Bottom to top", bottom_to_top, move |t, _, cx| t.edit_table(id, "Cut list order", cx, |t| if let TableKind::CutList { bottom_to_top, .. } = &mut t.kind { *bottom_to_top = true })),
                ],
            ));
        }
        if matches!(t.kind, TableKind::Bom { .. } | TableKind::CutList { .. }) {
            out.extend(self.split_entries(&t, cell));
        }
        if let TableKind::Bom { structured, bottom_to_top, .. } = t.kind {
            out.push(MenuEntry::Submenu(
                "BOM type".into(),
                vec![
                    MenuEntry::check("Flattened", !structured, move |t, _, cx| t.edit_table(id, "BOM type", cx, |t| if let TableKind::Bom { structured, .. } = &mut t.kind { *structured = false })),
                    MenuEntry::check("Structured", structured, move |t, _, cx| t.edit_table(id, "BOM type", cx, |t| if let TableKind::Bom { structured, .. } = &mut t.kind { *structured = true })),
                ],
            ));
            out.push(MenuEntry::Submenu(
                "Order".into(),
                vec![
                    MenuEntry::check("Top to bottom", !bottom_to_top, move |t, _, cx| t.edit_table(id, "BOM order", cx, |t| if let TableKind::Bom { bottom_to_top, .. } = &mut t.kind { *bottom_to_top = false })),
                    MenuEntry::check("Bottom to top", bottom_to_top, move |t, _, cx| t.edit_table(id, "BOM order", cx, |t| if let TableKind::Bom { bottom_to_top, .. } = &mut t.kind { *bottom_to_top = true })),
                ],
            ));
        }
        let corner = t.corner;
        let corners = [("Top right", TableCorner::TopRight), ("Top left", TableCorner::TopLeft), ("Bottom right", TableCorner::BottomRight), ("Bottom left", TableCorner::BottomLeft)]
            .into_iter()
            .map(|(name, k)| {
                MenuEntry::check(name, corner == k, move |t, _, cx| {
                    // Keep the table where it is: move the anchor to the new corner.
                    let l = t.built.as_ref().and_then(|b| b.table_layout(id)).cloned();
                    t.edit_table(id, "Table fixed corner", cx, |t| noshape_scene::drawing_tools::table_edit::set_fixed_corner(t, l.as_ref(), k))
                })
            })
            .collect();
        out.push(MenuEntry::Submenu("Fixed corner".into(), corners));
        let mut text = Vec::new();
        for h in noshape_scene::drawing_tools::table_edit::TEXT_HEIGHTS {
            text.push(MenuEntry::check(format!("{h} mm"), t.own_text_height && (t.text_height - h).abs() < 1e-9, move |t, _, cx| t.edit_table(id, "Table text height", cx, |t| noshape_scene::drawing_tools::table_edit::set_text_height(t, h))));
        }
        out.push(MenuEntry::Submenu("Text height".into(), text));
        out.push(MenuEntry::item("Fit to content", move |t, _, cx| t.edit_table(id, "Fit table", cx, noshape_scene::drawing_tools::table_edit::fit_to_content)));
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Delete table", move |t, _, cx| {
            t.selection.clear();
            t.edit_sheet("Delete table", cx, |s| s.tables.retain(|x| x.id != id));
        }));
        out
    }

    pub fn edit_table(&mut self, id: u64, name: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut Table)) {
        self.edit_sheet(name, cx, |s| {
            if let Some(t) = s.table_mut(id) {
                f(t);
            }
        });
    }

    pub fn break_entries(&self, view: ViewId, index: usize) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let Some(b) = self.drawing.sheets[self.sheet].view(view).and_then(|v| v.breaks.get(index)).cloned() else { return out };
        let styles = [("Zig zag", BreakStyle::ZigZag), ("Small zig zag", BreakStyle::SmallZigZag), ("Curve", BreakStyle::Curve), ("Straight", BreakStyle::Straight)]
            .into_iter()
            .map(|(n, st)| MenuEntry::check(n, b.style == st, move |t, _, cx| t.edit_view("Break style", view, cx, |v| if let Some(b) = v.breaks.get_mut(index) { b.style = st })))
            .collect();
        out.push(MenuEntry::Submenu("Break line style".into(), styles));
        let gaps = [3.0, 6.0, 10.0, 15.0]
            .into_iter()
            .map(|g| MenuEntry::check(format!("{g} mm"), (b.gap_mm - g).abs() < 1e-9, move |t, _, cx| t.edit_view("Break gap", view, cx, |v| if let Some(b) = v.breaks.get_mut(index) { b.gap_mm = g })))
            .collect();
        out.push(MenuEntry::Submenu("Gap".into(), gaps));
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Delete break", move |t, _, cx| {
            t.selection.clear();
            t.edit_view("Delete break", view, cx, |v| {
                if index < v.breaks.len() {
                    v.breaks.remove(index);
                }
            })
        }));
        out
    }

    // ------------------------------------------------------------ panels

    /// Shift+N with the Sheets panel open (live K20): a small "Rename" popup
    /// beside the row, the name fully selected.
    pub fn open_row_rename(&mut self, row: RowRename, window: &mut Window, cx: &mut Context<Self>) {
        let name = match row {
            RowRename::Sheet(i) => self.drawing.sheets.get(i).map(|s| s.name.clone()).unwrap_or_default(),
            RowRename::View(id) => self.current_sheet().view(id).map(|v| self.view_row_label(self.current_sheet(), v).0).unwrap_or_default(),
        };
        let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(name));
        let sub = cx.subscribe(&st, |this: &mut Self, _, ev: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.apply_row_rename(true, cx);
            }
        });
        self._subs.push(sub);
        Self::focus_input(&st, window, cx);
        self.row_rename = Some((row, st));
        cx.notify();
    }

    /// ✓ (`apply`) or ✕ / Esc of the "Rename" popup.
    pub fn apply_row_rename(&mut self, apply: bool, cx: &mut Context<Self>) {
        let Some((row, st)) = self.row_rename.take() else { return };
        let name = st.read(cx).value().trim().to_string();
        if apply && !name.is_empty() {
            match row {
                RowRename::Sheet(i) => {
                    if self.drawing.sheets.get(i).is_some_and(|s| s.name != name) {
                        self.edit("Rename sheet", cx, |d| {
                            d.rename_sheet(i, &name);
                        });
                    }
                }
                RowRename::View(id) => {
                    let sheet = self.current_sheet();
                    if let Some(v) = sheet.view(id).filter(|v| self.view_row_label(sheet, v).0 != name) {
                        let mut unnamed = v.clone();
                        unnamed.name = None;
                        let default = self.view_row_label(sheet, &unnamed).0;
                        let si = self.sheet;
                        self.edit("Rename view", cx, |d| {
                            if let Some(v) = d.sheets[si].view_mut(id) {
                                v.name = (name != default).then_some(name);
                            }
                        });
                    }
                }
            }
        }
        cx.notify();
    }

    /// The "Rename" popup (live drawing/drw-shiftN-*.png: x ≈ 260..465 at
    /// the row's height, green ✓ and red ✕, one field).
    fn row_rename_popup(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some((_, st)) = &self.row_rename else { return div().into_any_element() };
        let pal = p();
        let body = gpui_kit::component::v_flex()
            .id("dr-row-rename")
            .w(px(205.))
            .bg(pal.dialog_bg)
            .shadow(crate::drawing_chrome::dialog_shadow())
            .text_color(rgb(0x333333))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .capture_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    cx.stop_propagation();
                    this.apply_row_rename(false, cx);
                }
            }))
            .child(
                h_flex()
                    .h(px(28.))
                    .pl(px(11.))
                    .border_b_1()
                    .border_color(rgb(0xd3d7d9))
                    .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Rename"))
                    .child(div().id("row-rename-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(rgb(0x009500)).cursor_pointer().child(crate::ui::glyph("check", 18., gpui_kit::white())).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.apply_row_rename(true, cx))))
                    .child(div().id("row-rename-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.apply_row_rename(false, cx)))),
            )
            .child(div().p(px(4.)).child(Input::new(st).small()));
        // Live: the panel spans x 40..300 and the popup starts at 260.
        div().absolute().left(px(220.)).top_0().child(deferred(anchored().child(body)).with_priority(1)).into_any_element()
    }

    pub fn open_sheet_name_panel(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.drawing.sheets.get(index).map(|s| s.name.clone()).unwrap_or_default();
        let input = self.input(&name, window, cx);
        Self::focus_input(&input, window, cx);
        self.panel = Some(Panel::SheetName { index, input });
        cx.notify();
    }

    pub fn open_rotation_panel(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let cur = self.current_sheet().view(view).map(|v| v.rotation.to_degrees()).unwrap_or(0.0);
        let input = self.input(&format::format_number(cur, None, 3), window, cx);
        Self::focus_input(&input, window, cx);
        self.panel = Some(Panel::Rotation { view, input });
        cx.notify();
    }


    /// Apply the edited cell of a general table and edit the next (`step`
    /// 1, row by row), the previous (-1) or the one below (2). Past the
    /// last cell editing ends (live: Enter in the last cell, no row added).
    pub fn cell_step(&mut self, step: i32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Panel::Cell { table, row, col, .. }) = &self.panel else { return };
        let (table, row, col) = (*table, *row, *col);
        self.apply_panel(cx);
        let Some(TableKind::General { cells, .. }) = self.current_sheet().table(table).map(|t| t.kind.clone()) else { return };
        let ncols = cells.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
        let nrows = cells.len().max(1);
        let at = (row * ncols + col) as i64 + if step == 2 { ncols as i64 } else { step as i64 };
        if at < 0 || at >= (nrows * ncols) as i64 {
            // Live 752: editing ends with nothing selected.
            self.selection.clear();
            return;
        }
        let (r, c) = (at as usize / ncols, at as usize % ncols);
        let text = cells.get(r).and_then(|x| x.get(c)).cloned().unwrap_or_default();
        let input = self.textarea(&text, 3, window, cx);
        // Typing adds to a filled cell's text (qa TABLE-SHIFT-TAB-BLANK: the
        // caret sat before it, "QQA2").
        input.update(cx, |s, cx| s.set_selected_range(text.len()..text.len(), cx));
        let note = self.cell_note(table, r, c, window, cx);
        self.panel = Some(Panel::Cell { table, row: r, col: c, input, note });
        cx.notify();
    }

    /// The Note panel style of a general table cell, from its format.
    pub fn cell_note(&mut self, table: u64, row: usize, col: usize, window: &mut Window, cx: &mut Context<Self>) -> Option<Box<CellNote>> {
        use noshape_model::drawing::note::{NoteAlign, NoteStyle, NoteVAlign};
        use noshape_model::drawing::{CellAlign, CellVAlign};
        let t = self.current_sheet().table(table)?;
        if !matches!(t.kind, TableKind::General { .. }) {
            return None;
        }
        let f = t.shown_cell_format(row, col);
        let h = f.height.filter(|v| *v > 0.0).unwrap_or(self.drawing.table_text_height(t));
        let style = NoteStyle {
            font: (f.bold || f.italic || f.font.is_some()).then(|| noshape_model::drawing::sheet::TextFont { family: f.font.clone().unwrap_or_else(|| noshape_model::drawing::font::DEFAULT_FAMILY.into()), bold: f.bold, italic: f.italic }),
            underline: f.underline,
            strikethrough: f.strikethrough,
            align: match f.align {
                CellAlign::Left => NoteAlign::Left,
                CellAlign::Center => NoteAlign::Center,
                CellAlign::Right => NoteAlign::Right,
            },
            valign: match f.valign {
                CellVAlign::Top => NoteVAlign::Top,
                CellVAlign::Middle => NoteVAlign::Middle,
                CellVAlign::Bottom => NoteVAlign::Bottom,
            },
            ..Default::default()
        };
        let height = self.input(&crate::drawing_note::note_height_text(h, self.drawing.units_mm), window, cx);
        Some(Box::new(CellNote { style, height }))
    }

    /// The cell format the Note panel of a general table cell sets.
    fn cell_note_format(t: &noshape_model::drawing::Table, row: usize, col: usize, note: &CellNote, units_mm: bool, cx: &App) -> noshape_model::drawing::CellFormat {
        use noshape_model::drawing::note::{NoteAlign, NoteVAlign};
        use noshape_model::drawing::{CellAlign, CellVAlign};
        let st = &note.style;
        let h = crate::drawing_note::parse_note_height(&note.height.read(cx).value(), units_mm);
        let (bold, italic) = st.font.as_ref().map(|f| (f.bold, f.italic)).unwrap_or((false, false));
        noshape_model::drawing::CellFormat {
            align: match st.align {
                NoteAlign::Left | NoteAlign::Justified => CellAlign::Left,
                NoteAlign::Center => CellAlign::Center,
                NoteAlign::Right => CellAlign::Right,
            },
            valign: match st.valign {
                NoteVAlign::Top => CellVAlign::Top,
                NoteVAlign::Middle => CellVAlign::Middle,
                NoteVAlign::Bottom => CellVAlign::Bottom,
            },
            height: match h {
                Some(h) if (h - t.text_height).abs() > 1e-6 => Some(h),
                Some(_) => None,
                None => t.shown_cell_format(row, col).height,
            },
            bold,
            italic,
            underline: st.underline,
            strikethrough: st.strikethrough,
            font: st.font.as_ref().map(|f| f.family.clone()).filter(|f| f != noshape_model::drawing::font::DEFAULT_FAMILY),
        }
    }

    pub fn open_cell_panel(&mut self, table: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.current_sheet().table(table).cloned() else { return };
        if let TableKind::Revision { .. } = t.kind {
            self.open_revision_cell(&t, window, cx);
            return;
        }
        if let TableKind::Bom { .. } = t.kind {
            self.open_bom_cell(&t, window, cx);
            return;
        }
        if let TableKind::Custom { .. } = t.kind {
            self.open_custom_cell(&t, window, cx);
            return;
        }
        let TableKind::General { cells, .. } = &t.kind else {
            self.status = Some(format!("{} content comes from the model", t.type_name()).into());
            cx.notify();
            return;
        };
        let (row, col) = self.built.as_ref().and_then(|b| b.table_layout(table)).and_then(|l| l.cell_at(self.cursor)).map(|(r, c)| t.anchor_cell(r, c)).unwrap_or((0, 0));
        let text = cells.get(row).and_then(|r| r.get(col)).cloned().unwrap_or_default();
        // Live 1143 / behaviours "Note panel popups" (6): an empty cell is
        // selected (table selected, the cell grey) with the Cell panel; a cell
        // with text is edited in place.
        if text.trim().is_empty() {
            self.selection = vec![Sel::Table(table)];
            self.cell_sel = Some(crate::drawing_cells::CellSel { table, a: (row, col), b: (row, col) });
            cx.notify();
            return;
        }
        let input = self.textarea(&text, 3, window, cx);
        let note = self.cell_note(table, row, col, window, cx);
        // Live 755 / 756: a general table cell is edited with the table
        // unselected and the caret where it was clicked.
        if note.is_some() {
            self.selection.clear();
            let at = self.cell_click_offset(table, row, col, &text);
            input.update(cx, |s, cx| s.set_selected_range(at..at, cx));
        }
        self.panel = Some(Panel::Cell { table, row, col, input, note });
        cx.notify();
    }

    /// The Surface finish tool's click: a symbol there with the panel's value
    /// (the panel stays for more, help surface_finish).
    fn place_surface_finish(&mut self, at: [f64; 2], cx: &mut Context<Self>) {
        let (value, spec) = self.sf_panel_values(cx).unwrap_or_default();
        let id = self.current_sheet().next_annotation_id();
        let pending = self.note_leader.take();
        // On a dimension's line (an extension line) or a view's edge it
        // stands on it (live A45, request 1624).
        let pk = self.pk();
        let on_dim = if pending.is_some() { None } else { pk.surface_finish_on_dimension(at) };
        let ext = on_dim.and_then(|_| pk.hovered_dim_on(at));
        let on = if pending.is_some() { None } else { on_dim.or_else(|| pk.surface_finish_on(at)) };
        // Inside a face region: the first click puts a leader's dot there,
        // the second places the symbol (live request 1624).
        if pending.is_none()
            && on.is_none()
            && let Some(l) = self.face_leader(at)
        {
            self.note_leader = Some(l);
            cx.notify();
            return;
        }
        let mut sf = noshape_scene::drawing_tools::surface_finish(id, at, value, spec);
        let had_leader = pending.is_some();
        if let AnnotationKind::SurfaceFinish { on: o, leaders, ext: e, .. } = &mut sf.kind {
            *o = on;
            *e = ext;
            leaders.extend(pending);
        }
        self.edit_sheet("Surface finish", cx, |s| s.annotations.push(sf));
        if let Some(Panel::SurfaceFinish { placed, .. }) = &mut self.panel {
            placed.push(id);
        }
        // The leader's rubber band goes with the pending leader.
        if had_leader {
            self.update_preview(cx);
        }
    }

    /// A panel's Cancel / Esc: closed, nothing applied; the Surface finish
    /// tool's symbols placed meanwhile go too (help surface_finish: ✕
    /// cancels).
    pub fn cancel_panel(&mut self, cx: &mut Context<Self>) {
        if let Some(Panel::SurfaceFinish { edit: None, placed, .. }) = self.panel.take() {
            if !placed.is_empty() {
                self.edit_sheet("Surface finish", cx, |s| s.annotations.retain(|a| !placed.contains(&a.id)));
            }
            self.tool = Tool::Select;
        }
        cx.notify();
    }

    /// A point on a GD&T frame or a dimension's lines under sheet point
    /// `c` for the Datum tool to sit on.
    pub fn datum_attach_at(&self, c: [f64; 2]) -> Option<([f64; 2], noshape_model::drawing::DatumOn)> {
        self.pk().datum_attach_at(c)
    }

    /// Properties / edit panel of an annotation.
    pub fn open_annotation_panel(&mut self, a: &Annotation, window: &mut Window, cx: &mut Context<Self>) {
        let Some(a) = self.drawing.sheets[self.sheet].annotations.iter().find(|x| x.id == a.id).cloned() else { return };
        match &a.kind {
            AnnotationKind::Gdt { .. } => self.open_gdt_edit(a.id, window, cx),
            AnnotationKind::SurfaceFinish { anchor, value, spec, .. } => self.open_surface_finish_panel(Some(a.id), *anchor, value, spec, window, cx),
            AnnotationKind::Datum { anchor, label, .. } => {
                let input = self.input(label, window, cx);
                Self::focus_input(&input, window, cx);
                self.panel = Some(Panel::Datum { edit: Some(a.id), anchor: *anchor, input });
                cx.notify();
            }
            AnnotationKind::Balloon { .. } => self.open_callout_edit(a.id, window, cx),
            AnnotationKind::HoleCallout { .. } => self.open_hole_callout(a.id, window, cx),
            // A dimension on sketch geometry: its value box (live drawing/921).
            k if k.is_dimension() && a.sketch_dim.is_some() => self.open_sketch_dim_box(a.id, window, cx),
            k if k.is_dimension() => self.open_dim_palette(a.id, window, cx),
            _ => self.open_more_panel(&a, window, cx),
        }
    }

    pub fn label_row(label: &str, el: AnyElement) -> AnyElement {
        let pal = p();
        h_flex().gap_2().items_center().child(div().w(px(84.)).text_xs().text_color(pal.text_muted).child(label.to_string())).child(el).into_any_element()
    }

    /// Title and rows of the panels defined here.
    pub fn extra_panel_rows(&mut self, cx: &mut Context<Self>) -> Option<(&'static str, Vec<AnyElement>)> {
        let pal = p();
        let panel = self.panel.as_ref()?;
        Some(match panel {
            Panel::SheetName { input, .. } => ("Rename sheet", vec![Input::new(input).small().w(px(220.)).into_any_element()]),
            Panel::Rotation { input, .. } => ("View rotation", vec![Self::label_row("Angle (°)", Input::new(input).small().w(px(120.)).into_any_element())]),
            Panel::Cell { input, row, col, .. } => (
                "Edit cell",
                vec![
                    div().text_xs().text_color(pal.text_faint).child(if *row == usize::MAX { "Title".to_string() } else { format!("Row {}, column {} — fields like {{part.name}} allowed", row + 1, col + 1) }).into_any_element(),
                    // Tab / Enter: the next cell, Shift+Tab: the previous one,
                    // Alt+Enter: a new line in this cell.
                    div()
                        .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                            let k = &e.keystroke;
                            let step = match (k.key.as_str(), k.modifiers.shift, k.modifiers.alt) {
                                ("tab", false, _) | ("enter", false, false) => Some(1),
                                ("tab", true, _) => Some(-1),
                                ("enter", _, true) => {
                                    if let Some(Panel::Cell { input, .. }) = &this.panel {
                                        input.update(cx, |s, cx| s.insert("\n", window, cx));
                                    }
                                    cx.stop_propagation();
                                    None
                                }
                                _ => None,
                            };
                            if let Some(step) = step {
                                cx.stop_propagation();
                                this.cell_step(step, window, cx);
                            }
                        }))
                        .child(Textarea::new(input).w(px(300.)))
                        .into_any_element(),
                ],
            ),
            Panel::SurfaceFinish { .. } => ("Surface finish", self.sf_panel_rows(cx)),
            Panel::Datum { input, anchor, .. } => (if anchor[0].is_nan() { "Callout text (empty = BOM item)" } else { "Datum" }, vec![Self::label_row("Label", Input::new(input).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).small().w(px(120.)).into_any_element())]),
            _ => return None,
        })
    }

    pub fn apply_extra_panel(&mut self, panel: Panel, cx: &mut Context<Self>) {
        let val = |e: &Entity<gpui_kit::component::input::InputState>, cx: &App| e.read(cx).value().to_string();
        match panel {
            Panel::SheetName { index, input } => {
                let name = val(&input, cx);
                self.edit("Rename sheet", cx, |d| {
                    d.rename_sheet(index, &name);
                });
            }
            Panel::Rotation { view, input } => match val(&input, cx).trim().trim_end_matches('°').parse::<f64>() {
                Ok(deg) => self.rotate_view("Rotate view", view, deg.to_radians(), cx),
                Err(_) => self.status = Some("Invalid angle".into()),
            },
            Panel::Cell { table, row, col, input, .. } if self.current_sheet().table(table).is_some_and(|t| matches!(t.kind, TableKind::Revision { .. })) => {
                let text = self.note_text(&input, cx);
                self.apply_revision_cell(table, row, col, text, cx);
            }
            Panel::Cell { table, row, col, input, note } => {
                let text = self.note_text(&input, cx);
                let format = note.as_ref().and_then(|n| self.current_sheet().table(table).map(|t| Self::cell_note_format(t, row, col, n, self.drawing.units_mm, cx)));
                self.edit_table(table, "Edit cell", cx, |t| {
                    if let Some(f) = format.clone() {
                        t.set_cell_format(row, col, f);
                    }
                    noshape_scene::drawing_tools::set_general_cell(t, row, col, &text);
                    // Grow the column to fit when needed.
                    let th = t.text_height;
                    let need = text.lines().map(|l| noshape_model::drawing::font::text_width(l, th)).fold(0.0, f64::max) + th * 1.6;
                    let ncols = match &t.kind {
                        TableKind::General { cells, .. } => cells.first().map(|r| r.len()).unwrap_or(0),
                        _ => 0,
                    };
                    if t.col_widths.len() == ncols && let Some(w) = t.col_widths.get_mut(col) {
                        *w = w.max(need);
                    } else {
                        t.col_widths.clear();
                    }
                });
            }
            p @ Panel::SurfaceFinish { .. } => {
                // The values are read with the panel in place.
                self.panel = Some(p);
                let (value, spec) = self.sf_panel_values(cx).unwrap_or_default();
                let Some(Panel::SurfaceFinish { edit, pos, placed, .. }) = self.panel.take() else { return };
                match edit {
                    // ✓: the symbols placed take the panel's values.
                    None if !placed.is_empty() => {
                        self.edit_sheet("Surface finish", cx, |s| {
                            for a in s.annotations.iter_mut().filter(|a| placed.contains(&a.id)) {
                                if let AnnotationKind::SurfaceFinish { value: v, spec: sp, .. } = &mut a.kind {
                                    *v = value.clone();
                                    **sp = spec.clone();
                                }
                            }
                        });
                        self.tool = Tool::Select;
                    }
                    Some(id) => self.edit_sheet("Edit surface finish", cx, |s| {
                        if let Some(AnnotationKind::SurfaceFinish { value: v, spec: sp, .. }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                            *v = value;
                            **sp = spec;
                        }
                    }),
                    None => {
                        let id = self.current_sheet().next_annotation_id();
                        self.edit_sheet("Surface finish", cx, |s| s.annotations.push(noshape_scene::drawing_tools::surface_finish(id, pos, value, spec)));
                        self.tool = Tool::Select;
                    }
                }
            }
            Panel::Datum { edit: Some(id), input, .. } => {
                let label = val(&input, cx);
                self.edit_sheet("Edit annotation", cx, |s| match s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                    Some(AnnotationKind::Datum { label: l, .. }) => *l = label,
                    Some(AnnotationKind::Balloon { text, .. }) => *text = label,
                    _ => {}
                });
            }
            Panel::Datum { edit: None, .. } => {}
            p @ (Panel::BrokenOut { .. } | Panel::CutAngle { .. } | Panel::CellFormat { .. } | Panel::CellResize { .. } | Panel::DatumTarget { .. }) => self.apply_more_panel(p, cx),
            Panel::BomCell { table, item, col, input, over, .. } => {
                let value = input.read(cx).value().to_string();
                self.apply_bom_cell(table, item, col, value, over, cx);
            }
            _ => {}
        }
        cx.notify();
    }
}

/// A note's unrotated box (sheet mm) and its rotation.
pub use noshape_scene::drawing_tools::annotation_edit::note_box;

/// A note grip: 0 / 1 the left / right side, 2 the rotate grip above the
/// text; its sheet point.
pub type NoteGrip = (u8, [f64; 2]);

/// A Sheets panel tree icon: Onshape draws them at 20 px in full #333
/// (18x18 ink, live drawing/760), unlike the feature list's 17 px at 80 %.
fn sheets_icon(name: &str, cx: &App) -> Div {
    div().w(px(20.)).flex_none().flex().items_center().justify_center().child(crate::assets::icon(name, 20., cx))
}
