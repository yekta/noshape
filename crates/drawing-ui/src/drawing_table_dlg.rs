//! Drawing editor: Onshape's BOM table and Cut list table dialogs (live
//! drawing/188, help cutlist-table-01): Insert + the source (an assembly
//! for BOMs, a Part Studio with frames for cut lists, from the drawing's
//! views), Template "Follow assembly" and BOM type (BOM only), Order, and
//! Select fixed corner. The table's outline follows the cursor; a click
//! places it.

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::views::{self, SourceKey};
use noshape_model::drawing::{TableCorner, TableKind, ViewId};

const CORNERS: [TableCorner; 4] = [TableCorner::TopLeft, TableCorner::TopRight, TableCorner::BottomLeft, TableCorner::BottomRight];

/// BOM / cut list table dialog state.
pub struct TableDlg {
    pub kind: TableTool,
    /// The table edited (BOM / Cut list table properties); None while inserting.
    pub table: Option<u64>,
    original: Option<(TableKind, TableCorner)>,
    /// The view whose source the table lists.
    pub view: Option<ViewId>,
    pub structured: bool,
    /// Structured - Top level (else Multi level).
    pub top_level: bool,
    pub bottom_to_top: bool,
    pub corner: TableCorner,
    /// Template: "Onshape Default BOM" (else "Follow assembly").
    pub default_template: bool,
    /// A BOM of an assembly no view of the sheet shows (this document's or
    /// another one's; drawing.md TB14), instead of `view`'s.
    pub bom_source: Option<noshape_model::drawing::BomSource>,
    /// BOM data: an uploaded file's rows (the first the header), placed as a
    /// static table (help bom_table: "BOM data (an uploaded file)").
    pub bom_data: Option<Vec<Vec<String>>>,
}

impl DrawingEditor {
    /// Views of the current sheet whose source suits the table (assemblies
    /// for BOMs, Part Studios for cut lists), one per source, with names.
    fn table_sources(&self, kind: TableTool) -> Vec<(ViewId, String)> {
        let sheet = self.current_sheet();
        let mut seen: Vec<SourceKey> = Vec::new();
        let mut out = Vec::new();
        for v in sheet.real_views() {
            let Some((key, _)) = views::view_source(sheet, v.id) else { continue };
            let Some(src) = self.sources.get(&key) else { continue };
            let fits = match kind {
                TableTool::Bom => src.assembly,
                // A Part Studio with frames (live: none listed for a plain block).
                TableTool::CutList => !src.assembly && !src.context.frame_members.is_empty(),
                _ => false,
            };
            if fits && !seen.contains(&key) {
                seen.push(key);
                out.push((v.id, src.name.clone()));
            }
        }
        out
    }

    /// Create / drop the dialog with the BOM table and Cut list table tools.
    pub fn sync_table_dlg(&mut self) {
        let kind = match &self.tool {
            Tool::Table { kind: k @ (TableTool::Bom | TableTool::CutList), .. } => Some(*k),
            _ => None,
        };
        match kind {
            Some(k) if self.table_dlg.as_ref().is_none_or(|d| d.kind != k) => {
                // The selected view (if it fits), else the first fitting one.
                let sources = self.table_sources(k);
                let preferred = match &self.tool {
                    Tool::Table { view: Some(v), .. } => sources.iter().find(|(x, _)| x == v).map(|s| s.0),
                    _ => None,
                };
                let view = preferred.or_else(|| sources.first().map(|s| s.0));
                self.table_dlg = Some(TableDlg { kind: k, table: None, original: None, view, structured: false, top_level: false, bottom_to_top: false, corner: TableCorner::TopRight, default_template: false, bom_source: None, bom_data: None });
            }
            Some(_) => {}
            // The properties dialog of a placed table stays.
            None if self.table_dlg.as_ref().is_some_and(|d| d.table.is_none()) => self.table_dlg = None,
            None => {}
        }
    }

    /// Right-click › BOM table properties… / Cut list table properties….
    pub fn open_table_props(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(t) = self.current_sheet().table(id).cloned() else { return };
        let default_template = matches!(t.kind, TableKind::Bom { default_template: true, .. });
        let bom_source = match &t.kind {
            TableKind::Bom { source, .. } => source.clone(),
            _ => None,
        };
        let (kind, view, structured, top_level, bottom_to_top) = match t.kind {
            TableKind::Bom { view, structured, top_level, bottom_to_top, .. } => (TableTool::Bom, view, structured, top_level, bottom_to_top),
            TableKind::CutList { view, bottom_to_top, .. } => (TableTool::CutList, view, false, false, bottom_to_top),
            _ => return,
        };
        self.tool = Tool::Select;
        self.table_dlg = Some(TableDlg { kind, table: Some(id), original: Some((t.kind.clone(), t.corner)), view: Some(view), structured, top_level, bottom_to_top, corner: t.corner, default_template, bom_source, bom_data: None });
        cx.notify();
    }

    /// Editing a table: apply the dialog live.
    fn apply_table_dlg(&mut self, cx: &mut Context<Self>) {
        if let Some(d) = &self.table_dlg
            && let Some(id) = d.table
        {
            let (structured_v, top_v, bottom, corner, template, src) = (d.structured, d.top_level, d.bottom_to_top, d.corner, d.default_template, d.bom_source.clone());
            self.edit_table(id, if d.kind == TableTool::Bom { "BOM table properties" } else { "Cut list table properties" }, cx, |t| {
                match &mut t.kind {
                    TableKind::Bom { structured, top_level, bottom_to_top, default_template, source, .. } => (*structured, *top_level, *bottom_to_top, *default_template, *source) = (structured_v, top_v, bottom, template, src),
                    TableKind::CutList { bottom_to_top, .. } => *bottom_to_top = bottom,
                    _ => {}
                }
                t.corner = corner;
            });
        }
        cx.notify();
    }

    /// BOM data…: a CSV file (this document's, another one's or imported)
    /// as the table's rows.
    fn pick_bom_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ed = cx.entity().downgrade();
        crate::drawing_hooks::pick_file(self.doc.clone(), "BOM data", &[noshape_model::file_tab::FileKind::Csv], &["csv"], window, cx, move |file, _, cx| {
            let text = file.bytes().map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
            let rows = parse_csv(&text);
            let _ = ed.update(cx, |t, cx| {
                if let Some(d) = &mut t.table_dlg
                    && !rows.is_empty()
                {
                    d.bom_data = Some(rows);
                    d.bom_source = None;
                }
                t.update_preview(cx);
                cx.notify();
            });
        });
    }

    /// Other documents…: an assembly of another document (its latest
    /// version, else its workspace as it is now).
    fn pick_other_assembly(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::component::WindowExt as _;
        let here = self.doc.read(cx).session.path.clone();
        // The library scan reads every document: off the UI thread.
        let docs: Entity<Option<Vec<crate::library::DocEntry>>> = cx.new(|_| None);
        let scan = docs.clone();
        cx.spawn(async move |_, cx| {
            let found = cx
                .background_spawn(async move {
                    // The window's path may be relative (opened from a shell).
                    let canon = |p: &std::path::Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
                    let here = canon(&here);
                    crate::drawing_hooks::library_docs().into_iter().filter(|d| !d.broken && !d.trashed && canon(&d.path) != here).collect::<Vec<_>>()
                })
                .await;
            scan.update(cx, |d, cx| {
                *d = Some(found);
                cx.notify();
            });
        })
        .detach();
        let opened: Entity<Option<OpenedDoc>> = cx.new(|_| None);
        let search = cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search documents"));
        let ed = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let pal = p();
            let q = search.read(cx).value().to_lowercase();
            let mut list = v_flex().id("bom-other-list").h(px(260.)).overflow_y_scroll().border_1().border_color(gpui_kit::rgb(0xd3d7d9));
            let row = |id: SharedString| h_flex().id(id).h(px(30.)).px(px(8.)).gap(px(8.)).text_size(px(13.)).cursor_pointer().hover(|d| d.bg(gpui_kit::rgb(0xdef1fb)));
            match opened.read(cx).clone() {
                Some((name, items)) => {
                    let (back, search) = (opened.clone(), search.clone());
                    list = list.child(row("bom-other-back".into()).child("‹").child(div().font_weight(FontWeight::SEMIBOLD).child(name)).on_click(move |_, window, cx| {
                        search.update(cx, |s, cx| s.set_value("", window, cx));
                        back.update(cx, |o, cx| {
                            *o = None;
                            cx.notify();
                        })
                    }));
                    if items.is_empty() {
                        list = list.child(div().p(px(10.)).text_size(px(13.)).text_color(pal.text_muted).child("No assemblies in this document."));
                    }
                    for (i, (src, label)) in items.into_iter().filter(|(_, l)| l.to_lowercase().contains(&q)).enumerate() {
                        let ed = ed.clone();
                        list = list.child(row(SharedString::from(format!("bom-other-asm-{i}"))).child(crate::assets::icon("assembly", 16., cx)).child(label).on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            let src = src.clone();
                            let _ = ed.update(cx, |t, cx| {
                                if let Some(d) = &mut t.table_dlg {
                                    d.bom_source = Some(src);
                                    d.bom_data = None;
                                }
                                t.ensure_sources(false, cx);
                                t.update_preview(cx);
                                cx.notify();
                            });
                        }));
                    }
                }
                None => {
                    let docs = docs.read(cx).clone();
                    if docs.is_none() {
                        list = list.child(div().p(px(10.)).text_size(px(13.)).text_color(pal.text_muted).child("Loading documents…"));
                    }
                    for (i, doc) in docs.iter().flatten().filter(|d| d.name.to_lowercase().contains(&q)).enumerate() {
                        let (open, doc, search) = (opened.clone(), doc.clone(), search.clone());
                        list = list.child(row(SharedString::from(format!("bom-other-doc-{i}"))).child(crate::assets::icon("document", 16., cx)).child(div().flex_1().truncate().child(doc.name.clone())).on_click(move |_, window, cx| {
                            search.update(cx, |s, cx| s.set_value("", window, cx));
                            let items = (|| {
                                let store = noshape_store::Document::open_read_only(&doc.path).ok()?;
                                let graph = store.graph().ok()?;
                                let (mv, vname) = match graph.versions.last() {
                                    Some(v) => (v.microversion, v.name.clone()),
                                    None => (store.head(store.default_branch().ok()?).ok()?, "Main".to_string()),
                                };
                                let snap = crate::derived::external_snapshot(&doc.path, mv).ok()?;
                                Some(
                                    snap.ordered_tabs()
                                        .into_iter()
                                        .filter(|t| matches!(t.content, noshape_model::document::TabContent::Assembly(_)))
                                        .map(|t| {
                                            let key = crate::derived::ExternalRef { path: doc.path.clone(), microversion: mv, tab: t.id.to_string() }.key();
                                            let label = format!("{} ({} {vname})", t.name, doc.name);
                                            (noshape_model::drawing::BomSource { tab: t.id, version: Some(key), label: label.clone() }, label)
                                        })
                                        .collect::<Vec<_>>(),
                                )
                            })()
                            .unwrap_or_default();
                            open.update(cx, |o, cx| {
                                *o = Some((doc.name.clone(), items));
                                cx.notify();
                            });
                        }));
                    }
                }
            }
            crate::os_ui::modal(dialog, "Other documents", 520.).child(crate::os_ui::modal_body().gap(px(8.)).child(gpui_kit::component::input::Input::new(&search).h(px(30.)).text_size(px(13.))).child(list)).footer(crate::os_ui::modal_footer().child(crate::os_ui::modal_cancel(crate::os_ui::cancel_button("bom-other-cancel", "Cancel", cx))))
        });
    }

    pub fn close_table_dlg(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(d) = self.table_dlg.take() else { return };
        match (d.table, d.original) {
            (Some(id), Some((kind, corner))) if !accept => self.edit_table(id, "Table properties", cx, |t| {
                t.kind = kind;
                t.corner = corner;
            }),
            (None, _) => self.tool = Tool::Select,
            _ => {}
        }
        // The placement preview goes with it (qa DWG-TABLE-ESC-GHOST).
        self.update_preview(cx);
        cx.notify();
    }

    /// Right-click with cells of a general table selected (help
    /// drawings-table-contextmenu-04): Insert ▸ Column Left / Column Right /
    /// Row Above / Row Below, Remove ▸, Duplicate ▸ | Size ▸ | Merge ▸,
    /// Unmerge, Resize… | Copy.
    pub fn general_cell_entries(&self, t: &noshape_model::drawing::Table, cs: crate::drawing_cells::CellSel) -> Vec<MenuEntry> {
        let id = t.id;
        let (r0, r1, c0, c1) = (*cs.rows().start(), *cs.rows().end(), *cs.cols().start(), *cs.cols().end());
        let edit = |label: &'static str, f: Box<dyn Fn(&mut noshape_model::drawing::Table)>| {
            let f = std::rc::Rc::new(f);
            MenuEntry::item(label, move |ed, _, cx| {
                let f = f.clone();
                ed.cell_sel = None;
                ed.edit_table(id, label, cx, move |t| f(t));
            })
        };
        // Remove ▸ Columns / Rows: one cell stays selected where the first removed one was (live: its Cell panel
        // stays open).
        let remove = |label: &'static str, f: Box<dyn Fn(&mut noshape_model::drawing::Table)>| {
            let f = std::rc::Rc::new(f);
            MenuEntry::item(label, move |ed, _, cx| {
                let f = f.clone();
                ed.cell_sel = None;
                ed.edit_table(id, label, cx, move |t| f(t));
                let dims = ed.current_sheet().table(id).and_then(|t| match &t.kind {
                    noshape_model::drawing::TableKind::General { cells, .. } => Some((cells.len(), cells.first().map_or(0, Vec::len))),
                    _ => None,
                });
                if let Some((rows, cols)) = dims.filter(|d| d.0 > 0 && d.1 > 0) {
                    let at = (r0.min(rows - 1), c0.min(cols - 1));
                    ed.cell_sel = Some(crate::drawing_cells::CellSel { table: id, a: at, b: at });
                    cx.notify();
                }
            })
        };
        let mut out = vec![
            MenuEntry::Submenu(
                "Insert".into(),
                vec![
                    edit("Column Left", Box::new(move |t| t.insert_col(c0))),
                    edit("Column Right", Box::new(move |t| t.insert_col(c1 + 1))),
                    edit("Row Above", Box::new(move |t| t.insert_row(r0))),
                    edit("Row Below", Box::new(move |t| t.insert_row(r1 + 1))),
                ],
            ),
            MenuEntry::Submenu(
                "Remove".into(),
                vec![
                    remove("Columns", Box::new(move |t| {
                        for c in (c0..=c1).rev() {
                            t.delete_col(c);
                        }
                    })),
                    remove("Rows", Box::new(move |t| {
                        for r in (r0..=r1).rev() {
                            t.delete_row(r);
                        }
                    })),
                ],
            ),
            MenuEntry::Submenu("Duplicate".into(), vec![edit("Columns", Box::new(move |t| t.duplicate_cols(c0, c1))), edit("Rows", Box::new(move |t| t.duplicate_rows(r0, r1)))]),
            MenuEntry::Separator,
        ];
        let mut size = self.size_entries(t, cs, "Resize...");
        let resize = size.pop();
        out.extend(size);
        out.push(MenuEntry::Separator);
        // Live drawing/538: Merge ▸ All / Horizontal / Vertical (greyed for
        // one cell); a block has no Resize….
        let multi = r1 > r0 || c1 > c0;
        let merge = vec![
            edit("All", Box::new(move |t| {
                t.merge_cells((r0, c0), (r1, c1));
            })),
            edit("Horizontal", Box::new(move |t| {
                for r in r0..=r1 {
                    t.merge_cells((r, c0), (r, c1));
                }
            })),
            edit("Vertical", Box::new(move |t| {
                for c in c0..=c1 {
                    t.merge_cells((r0, c), (r1, c));
                }
            })),
        ]
        .into_iter()
        .collect();
        // Live (behaviours 1498, qa DWG-CELL-MERGE-GREY): for one cell
        // "Merge ▸" itself is greyed (an empty submenu).
        out.push(MenuEntry::Submenu("Merge".into(), if multi { merge } else { Vec::new() }));
        out.push(edit("Unmerge", Box::new(move |t| {
            t.unmerge(r0, c0);
        })).disabled(t.merge_at(r0, c0).is_none()));
        let resize = resize.filter(|_| !multi);
        out.extend(resize);
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Copy", |ed, _, cx| {
            ed.copy_cells(cx);
        }));
        out
    }

    /// Right-click with cells of a BOM, cut list or custom table selected
    /// (help BOM-table-context-menu-01): Size ▸ Columns Equally / Rows
    /// Equally (of the selected columns / rows), Resize.
    pub fn cell_size_entries(&self, t: &noshape_model::drawing::Table, cs: crate::drawing_cells::CellSel) -> Vec<MenuEntry> {
        self.size_entries(t, cs, "Resize")
    }

    /// Size ▸ and the resize item (`resize`: its label).
    fn size_entries(&self, t: &noshape_model::drawing::Table, cs: crate::drawing_cells::CellSel, resize: &'static str) -> Vec<MenuEntry> {
        let id = t.id;
        let Some(l) = self.built.as_ref().and_then(|b| b.table_layout(id)).cloned() else { return Vec::new() };
        let widths: Vec<f64> = l.xs.windows(2).map(|w| w[1] - w[0]).collect();
        let heights: Vec<f64> = l.ys.windows(2).skip(l.title as usize).map(|w| w[0] - w[1]).collect();
        let (cols, rows): (Vec<usize>, Vec<usize>) = (cs.cols().filter(|c| *c < widths.len()).collect(), cs.rows().filter(|r| *r < heights.len()).collect());
        let (l1, l2, c1, r1) = (l.clone(), l.clone(), cols.clone(), rows.clone());
        let size = vec![
            MenuEntry::item("Columns Equally", move |t, _, cx| {
                t.edit_table(id, "Size columns equally", cx, |t| noshape_scene::drawing_tools::table_edit::columns_equally_of(t, &l1, &c1));
            })
            .disabled(cols.len() < 2),
            MenuEntry::item("Rows Equally", move |t, _, cx| {
                t.edit_table(id, "Size rows equally", cx, |t| noshape_scene::drawing_tools::table_edit::rows_equally_of(t, &l2, &r1));
            })
            .disabled(rows.len() < 2),
        ];
        let col = cs.a.1;
        let (w, h) = (widths.get(col).copied().unwrap_or(20.0), heights.get(cs.a.0).copied().unwrap_or(t.row_height));
        vec![
            MenuEntry::Submenu("Size".into(), size),
            MenuEntry::item(resize, move |t, win, cx| {
                let width = t.input(&noshape_model::drawing::format::format_number(w, None, 2), win, cx);
                let height = t.input(&noshape_model::drawing::format::format_number(h, None, 2), win, cx);
                t.panel = Some(Panel::CellResize { table: id, col, width, height });
                cx.notify();
            }),
        ]
    }

    /// A click with the dialog open: place the table there. True when used.
    /// The BOM or cut list table the dialog would place at sheet point `at`
    /// (inserting, a source chosen), and its edit's name.
    fn table_dlg_table(&self, at: [f64; 2]) -> Option<(noshape_model::drawing::Table, &'static str)> {
        let d = self.table_dlg.as_ref().filter(|d| d.table.is_none())?;
        let id = self.current_sheet().next_table_id();
        if let Some(rows) = &d.bom_data {
            let mut t = noshape_model::drawing::Table::new(id, TableKind::General { cells: rows.clone(), title: None, header: true }, at);
            t.corner = d.corner;
            return Some((t, "BOM table"));
        }
        let view = if d.bom_source.is_some() { ViewId(0) } else { d.view? };
        let (kind, name) = match d.kind {
            TableTool::Bom => (TableKind::Bom { view, structured: d.structured, top_level: d.top_level, bottom_to_top: d.bottom_to_top, overrides: Vec::new(), default_template: d.default_template, source: d.bom_source.clone() }, "BOM table"),
            _ => (TableKind::CutList { view, bottom_to_top: d.bottom_to_top, config: None }, "Cut list table"),
        };
        let mut t = noshape_model::drawing::Table::new(id, kind, at);
        t.corner = d.corner;
        Some((t, name))
    }

    /// The BOM / cut list table being placed, at the pointer (as the hole
    /// table's).
    pub fn table_dlg_preview(&self) -> Option<noshape_model::drawing::Table> {
        self.table_dlg_table(self.cursor).map(|(t, _)| t)
    }

    pub fn table_dlg_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(d) = &self.table_dlg else { return false };
        if d.table.is_some() {
            return false;
        }
        let Some((t, name)) = self.table_dlg_table(self.cursor) else { return true };
        let id = t.id;
        self.edit_sheet(name, cx, |s| s.tables.push(t));
        self.table_dlg = None;
        self.tool = Tool::Select;
        self.selection = vec![Sel::Table(id)];
        cx.notify();
        true
    }

    pub fn render_table_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.table_dlg.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let ed = cx.entity();
        let bom = d.kind == TableTool::Bom;
        let editing = d.table.is_some();
        let source_tab = d.view.and_then(|v| views::view_source(self.current_sheet(), v)).map(|(k, _)| k.tab);
        let sources = self.table_sources(d.kind);
        // The listed view of the table's source (one per source; the table's
        // own view may not be the one listed).
        let key_of = |v: ViewId| views::view_source(self.current_sheet(), v).map(|(k, _)| k);
        let source_label = match &d.bom_source {
            _ if d.bom_data.is_some() => "BOM data".to_string(),
            Some(s) => s.label.clone(),
            None => sources.iter().find(|s| Some(s.0) == d.view || (d.view.is_some() && key_of(s.0) == d.view.and_then(key_of))).map(|s| s.1.clone()).unwrap_or_default(),
        };
        // Cut list table properties › Configurations (live request 1638 (b)).
        let config_line = d
            .table
            .filter(|_| d.kind == TableTool::CutList)
            .and_then(|id| self.current_sheet().table(id))
            .and_then(|t| views::table_source(self.current_sheet(), t))
            .map(|k| self.config_summary(k.tab, &k.config, cx))
            .filter(|s| !s.is_empty());
        type Set = std::rc::Rc<dyn Fn(&mut TableDlg)>;
        let select = |id: &'static str, label: String, w: f32, items: Vec<(String, bool, Set)>| {
            let ed = ed.clone();
            // "Onshape Default BOM" is wider than the button: that list
            // takes its own width.
            let as_select = id != "table-dlg-template";
            Button::new(id).label(label).ghost().xsmall().w(px(w)).dropdown_caret(true).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).rounded(px(0.)).os_dropdown_menu(SharedString::from(format!("{id}-dd")), as_select, move |mut m, _, _| {
                for (l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| {
                        e.update(cx, |t, cx| {
                            if let Some(d) = &mut t.table_dlg {
                                f(d);
                            }
                            // Another assembly: its source for the preview.
                            t.ensure_sources(false, cx);
                            t.apply_table_dlg(cx);
                        })
                    }));
                }
                m
            })
        };
        let mut source_items: Vec<(String, bool, Set)> = sources.iter().map(|(v, n)| {
            let v = *v;
            (n.clone(), Some(v) == d.view && d.bom_source.is_none(), std::rc::Rc::new(move |d: &mut TableDlg| {
                d.view = Some(v);
                (d.bom_source, d.bom_data) = (None, None);
            }) as Set)
        }).collect();
        // BOMs: this document's other assemblies too (help bom_table:
        // Current document › Assemblies; drawing.md TB14).
        if bom {
            let shown: Vec<noshape_model::document::TabId> = sources.iter().filter_map(|(v, _)| key_of(*v).map(|k| k.tab)).collect();
            for t in self.doc.read(cx).session.doc.ordered_tabs() {
                if !matches!(t.content, noshape_model::document::TabContent::Assembly(_)) || shown.contains(&t.id) {
                    continue;
                }
                let s = noshape_model::drawing::BomSource { tab: t.id, version: None, label: t.name.clone() };
                let on = d.bom_source.as_ref() == Some(&s);
                source_items.push((t.name.clone(), on, std::rc::Rc::new(move |d: &mut TableDlg| (d.bom_source, d.bom_data) = (Some(s.clone()), None)) as Set));
            }
        }
        // Live (behaviours 11140; 2131): Flattened / Structured - Top level / Structured - Multi level.
        let types: Vec<(String, bool, Set)> = vec![
            ("Flattened".into(), !d.structured, std::rc::Rc::new(|d: &mut TableDlg| d.structured = false) as Set),
            ("Structured - Top level".into(), d.structured && d.top_level, std::rc::Rc::new(|d: &mut TableDlg| (d.structured, d.top_level) = (true, true)) as Set),
            ("Structured - Multi level".into(), d.structured && !d.top_level, std::rc::Rc::new(|d: &mut TableDlg| (d.structured, d.top_level) = (true, false)) as Set),
        ];
        let type_label = types.iter().find(|t| t.1).map(|t| t.0.clone()).unwrap_or_default();
        // Live request 1627: exactly these two.
        let templates: Vec<(String, bool, Set)> = vec![
            ("Follow assembly".into(), !d.default_template, std::rc::Rc::new(|d: &mut TableDlg| d.default_template = false) as Set),
            ("Onshape Default BOM".into(), d.default_template, std::rc::Rc::new(|d: &mut TableDlg| d.default_template = true) as Set),
        ];
        let orders: Vec<(String, bool, Set)> = vec![
            ("Top to bottom".into(), !d.bottom_to_top, std::rc::Rc::new(|d: &mut TableDlg| d.bottom_to_top = false) as Set),
            ("Bottom to top".into(), d.bottom_to_top, std::rc::Rc::new(|d: &mut TableDlg| d.bottom_to_top = true) as Set),
        ];
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(28.)).px(px(8.)).child(div().min_w(px(70.)).pr(px(6.)).flex_none().whitespace_nowrap().text_size(px(11.)).child(label)).child(body).into_any_element();
        let corner = d.corner;
        let corner_btn = |k: TableCorner, i: usize, cx: &mut Context<Self>| {
            let on = corner == k;
            let (top, left) = (matches!(k, TableCorner::TopLeft | TableCorner::TopRight), matches!(k, TableCorner::TopLeft | TableCorner::BottomLeft));
            div()
                .id(("table-corner", i))
                .size(px(24.))
                .relative()
                .cursor_pointer()
                .border_1()
                .border_color(gpui_kit::rgb(0x909090))
                .when(on, |e| e.border_b_2().border_color(pal.accent))
                .child(crate::ui::glyph("table", 20., ink))
                .child(div().absolute().size(px(4.)).rounded_full().bg(ink).when(top, |e| e.top(px(1.))).when(!top, |e| e.bottom(px(1.))).when(left, |e| e.left(px(1.))).when(!left, |e| e.right(px(1.))))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.table_dlg {
                        d.corner = k;
                    }
                    this.apply_table_dlg(cx);
                }))
        };
        let corners: Vec<AnyElement> = CORNERS.iter().enumerate().map(|(i, k)| corner_btn(*k, i, cx).into_any_element()).collect();
        let (doc_name, ws_name) = {
            let doc = self.doc.read(cx);
            (doc.name().to_string(), doc.session.store.branch(doc.session.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into()))
        };
        let ref_text = if bom { format!("BOM : {source_label}") } else { source_label.clone() };
        crate::ui_script::note_dialog(|| {
            // Every text shown, in reading order (README "### Drawings"; live 2131): the selects' options too.
            let title = match (bom, editing) {
                (true, false) => "BOM table",
                (false, false) => "Cut list table",
                (true, true) => "BOM table properties",
                (false, true) => "Cut list table properties",
            };
            let mut text: Vec<String> = vec![title.into()];
            if editing {
                text.extend(["Document:".into(), doc_name.clone(), "Workspace:".into(), ws_name.clone(), "Reference:".into(), ref_text.clone(), "In progress".into()]);
            } else {
                text.extend(["Insert".into(), source_label.clone()]);
            }
            if let Some(c) = &config_line {
                text.extend(["Configurations".into(), c.clone()]);
            }
            if editing {
                text.push("Sheet".into());
                text.extend(self.drawing.sheets.iter().map(|s| s.name.clone()));
            }
            if bom {
                text.extend(["Template", "Follow assembly", "Onshape Default BOM", "BOM type", "Flattened", "Structured - Top level", "Structured - Multi level"].map(String::from));
            }
            text.extend(["Order", "Top to bottom", "Bottom to top", "Select fixed corner"].map(String::from));
            text.retain(|t| !t.is_empty());
            serde_json::json!({ "kind": "drawing-panel", "title": title, "text": text })
        });
        Some(
            v_flex()
                .id("dr-table-dlg")
                .capture_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                    if e.keystroke.key == "escape" {
                        cx.stop_propagation();
                        this.close_table_dlg(false, cx);
                    }
                }))
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(210.))
                .pb(px(6.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .text_size(px(12.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(match (bom, editing) {
                            (true, false) => "BOM table",
                            (false, false) => "Cut list table",
                            (true, true) => "BOM table properties",
                            (false, true) => "Cut list table properties",
                        }))
                        .when(editing, |h| h.child(div().id("table-dlg-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(pal.accept).cursor_pointer().child(crate::drawing_chrome::ok_check(16.)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_table_dlg(true, cx)))))
                        .child(div().id("table-dlg-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_table_dlg(false, cx)))),
                )
                // Properties: the Reference link to the source tab (help TB18);
                // inserting: Insert + the drawing's suitable sources.
                // Live 2132: Document:, Workspace:, Reference: "BOM : <assembly>" with its "In progress" badge.
                .when(editing, |v| {
                    v.child(row("Document:", div().text_size(px(12.)).child(crate::ui::user_text(doc_name.clone())).into_any_element()))
                        .child(row("Workspace:", div().text_size(px(12.)).child(ws_name.clone()).into_any_element()))
                        .child(row(
                            "Reference:",
                            h_flex()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .id("table-dlg-ref")
                                        .text_size(px(12.))
                                        .text_color(gpui_kit::rgb(0x1651b0))
                                        .cursor_pointer()
                                        .child(ref_text.clone())
                                        .when_some(source_tab, |l, tab| l.on_click(move |_, w, cx| crate::actions::request_activate_tab(tab, w, cx))),
                                )
                                .child(div().px(px(4.)).rounded(px(3.)).bg(gpui_kit::rgb(0x9ccef0)).text_size(px(11.)).text_color(gpui_kit::rgb(0x333333)).child("In progress"))
                                .into_any_element(),
                        ))
                })
                .when_some(config_line, |v, text| v.child(row("Configurations", div().text_size(px(12.)).text_color(ink).child(text).into_any_element())))
                .when(!editing, |v| {
                    v.child(
                        h_flex()
                            .px(px(4.))
                            .gap(px(4.))
                            // Insert lists the drawing's suitable sources, like the select.
                            .child(select("table-dlg-insert", "Insert".into(), 64., source_items.clone()))
                            .child(select("table-dlg-source", source_label.clone(), 120., source_items)),
                    )
                    // Other documents' assemblies and BOM data files (help
                    // bom_table; drawing.md TB14).
                    .when(bom, |v| {
                        let link = |id: &'static str, label: &'static str| div().id(id).text_size(px(12.)).text_color(gpui_kit::rgb(0x1651b0)).cursor_pointer().child(label);
                        v.child(
                            h_flex()
                                .px(px(8.))
                                .py(px(2.))
                                .gap(px(12.))
                                .child(link("table-dlg-other", "Other documents…").on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.pick_other_assembly(window, cx))))
                                .child(link("table-dlg-data", "BOM data…").on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.pick_bom_data(window, cx)))),
                        )
                    })
                })
                .when_some(d.table.filter(|_| editing), |v, id| {
                    let names: Vec<String> = self.drawing.sheets.iter().map(|s| s.name.clone()).collect();
                    let cur = self.sheet;
                    let ed = ed.clone();
                    let sheet = Button::new("table-dlg-sheet").label(names.get(cur).cloned().unwrap_or_default()).ghost().xsmall().w(px(120.)).dropdown_caret(true).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).rounded(px(0.)).os_dropdown_menu("table-dlg-sheet-dd", true, move |mut m, _, _| {
                        for (i, n) in names.iter().enumerate() {
                            let e = ed.clone();
                            m = m.item(crate::os_menu::OsMenuItem::new(n.clone()).checked(i == cur).on_click(move |_, _, cx| {
                                e.update(cx, |t, cx| {
                                    if i != cur {
                                        t.close_table_dlg(true, cx);
                                        t.move_table_to_sheet(id, Some(i), cx);
                                    }
                                })
                            }));
                        }
                        m
                    });
                    v.child(row("Sheet", sheet.into_any_element()))
                })
                .when(bom, |v| v.child(row("Template", select("table-dlg-template", if d.default_template { "Onshape Default BOM".into() } else { "Follow assembly".into() }, 120., templates).into_any_element())).child(row("BOM type", select("table-dlg-type", type_label.clone(), 120., types).into_any_element())))
                .child(row("Order", select("table-dlg-order", if d.bottom_to_top { "Bottom to top".into() } else { "Top to bottom".into() }, 120., orders).into_any_element()))
                .child(div().px(px(8.)).pt(px(4.)).text_size(px(11.)).child("Select fixed corner"))
                .child(h_flex().px(px(8.)).pt(px(4.)).gap(px(26.)).children(corners))
                .into_any_element(),
        )
    }
}

/// Other documents…: the opened document's name and its assemblies (with
/// their labels).
type OpenedDoc = (String, Vec<(noshape_model::drawing::BomSource, String)>);

/// Rows of a CSV file (quoted fields, "" inside quotes; blank lines dropped).
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let (mut cells, mut cur, mut quoted) = (Vec::new(), String::new(), false);
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' if quoted && chars.peek() == Some(&'"') => {
                    cur.push('"');
                    chars.next();
                }
                '"' => quoted = !quoted,
                ',' if !quoted => cells.push(std::mem::take(&mut cur).trim().to_string()),
                _ => cur.push(c),
            }
        }
        cells.push(cur.trim().to_string());
        rows.push(cells);
    }
    rows
}

#[cfg(test)]
mod tests {
    #[test]
    fn csv_rows_keep_quoted_commas() {
        assert_eq!(super::parse_csv("Item,Name\n1,\"Bolt, M6\"\n\n2,\"Nut \"\"A\"\"\"\n"), vec![vec!["Item", "Name"], vec!["1", "Bolt, M6"], vec!["2", "Nut \"A\""]]);
    }
}

