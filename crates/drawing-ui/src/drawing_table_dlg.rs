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
    pub bottom_to_top: bool,
    pub corner: TableCorner,
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
                TableTool::CutList => !src.assembly,
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
                self.table_dlg = Some(TableDlg { kind: k, table: None, original: None, view, structured: false, bottom_to_top: false, corner: TableCorner::TopRight });
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
        let (kind, view, structured, bottom_to_top) = match t.kind {
            TableKind::Bom { view, structured, bottom_to_top, .. } => (TableTool::Bom, view, structured, bottom_to_top),
            TableKind::CutList { view, bottom_to_top } => (TableTool::CutList, view, false, bottom_to_top),
            _ => return,
        };
        self.tool = Tool::Select;
        self.table_dlg = Some(TableDlg { kind, table: Some(id), original: Some((t.kind.clone(), t.corner)), view: Some(view), structured, bottom_to_top, corner: t.corner });
        cx.notify();
    }

    /// Editing a table: apply the dialog live.
    fn apply_table_dlg(&mut self, cx: &mut Context<Self>) {
        if let Some(d) = &self.table_dlg
            && let Some(id) = d.table
        {
            let (structured_v, bottom, corner) = (d.structured, d.bottom_to_top, d.corner);
            self.edit_table(id, if d.kind == TableTool::Bom { "BOM table properties" } else { "Cut list table properties" }, cx, |t| {
                match &mut t.kind {
                    TableKind::Bom { structured, bottom_to_top, .. } => (*structured, *bottom_to_top) = (structured_v, bottom),
                    TableKind::CutList { bottom_to_top, .. } => *bottom_to_top = bottom,
                    _ => {}
                }
                t.corner = corner;
            });
        }
        cx.notify();
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
                    edit("Columns", Box::new(move |t| {
                        for c in (c0..=c1).rev() {
                            t.delete_col(c);
                        }
                    })),
                    edit("Rows", Box::new(move |t| {
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
        .map(|e| e.disabled(!multi))
        .collect();
        out.push(MenuEntry::Submenu("Merge".into(), merge));
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
    pub fn table_dlg_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(d) = &self.table_dlg else { return false };
        if d.table.is_some() {
            return false;
        }
        let Some(view) = d.view else { return true };
        let c = self.cursor;
        let id = self.current_sheet().next_table_id();
        let (kind, name) = match d.kind {
            TableTool::Bom => (TableKind::Bom { view, structured: d.structured, bottom_to_top: d.bottom_to_top, overrides: Vec::new() }, "BOM table"),
            _ => (TableKind::CutList { view, bottom_to_top: d.bottom_to_top }, "Cut list table"),
        };
        let corner = d.corner;
        self.edit_sheet(name, cx, |s| {
            let mut t = noshape_model::drawing::Table::new(id, kind, c);
            t.corner = corner;
            s.tables.push(t);
        });
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
        let source_label = sources.iter().find(|s| Some(s.0) == d.view).map(|s| s.1.clone()).unwrap_or_default();
        type Set = std::rc::Rc<dyn Fn(&mut TableDlg)>;
        let select = |id: &'static str, label: String, w: f32, items: Vec<(String, bool, Set)>| {
            let ed = ed.clone();
            Button::new(id).label(label).ghost().xsmall().w(px(w)).dropdown_caret(true).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).rounded(px(0.)).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                for (l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| {
                        e.update(cx, |t, cx| {
                            if let Some(d) = &mut t.table_dlg {
                                f(d);
                            }
                            t.apply_table_dlg(cx);
                        })
                    }));
                }
                m
            })
        };
        let source_items: Vec<(String, bool, Set)> = sources.iter().map(|(v, n)| {
            let v = *v;
            (n.clone(), Some(v) == d.view, std::rc::Rc::new(move |d: &mut TableDlg| d.view = Some(v)) as Set)
        }).collect();
        let types: Vec<(String, bool, Set)> = vec![
            ("Flattened".into(), !d.structured, std::rc::Rc::new(|d: &mut TableDlg| d.structured = false) as Set),
            ("Structured".into(), d.structured, std::rc::Rc::new(|d: &mut TableDlg| d.structured = true) as Set),
        ];
        let orders: Vec<(String, bool, Set)> = vec![
            ("Top to bottom".into(), !d.bottom_to_top, std::rc::Rc::new(|d: &mut TableDlg| d.bottom_to_top = false) as Set),
            ("Bottom to top".into(), d.bottom_to_top, std::rc::Rc::new(|d: &mut TableDlg| d.bottom_to_top = true) as Set),
        ];
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(28.)).px(px(8.)).child(div().w(px(70.)).text_size(px(11.)).child(label)).child(body).into_any_element();
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
        Some(
            v_flex()
                .id("dr-table-dlg")
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
                        .when(editing, |h| h.child(div().id("table-dlg-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(pal.accept).cursor_pointer().child(crate::ui::glyph("check", 16., gpui_kit::white())).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_table_dlg(true, cx)))))
                        .child(div().id("table-dlg-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_table_dlg(false, cx)))),
                )
                // Properties: the Reference link to the source tab (help TB18);
                // inserting: Insert + the drawing's suitable sources.
                .when(editing, |v| {
                    v.child(row(
                        "Reference",
                        div()
                            .id("table-dlg-ref")
                            .text_size(px(12.))
                            .text_color(gpui_kit::rgb(0x1651b0))
                            .cursor_pointer()
                            .child(source_label.clone())
                            .when_some(source_tab, |l, tab| l.on_click(move |_, w, cx| crate::actions::request_activate_tab(tab, w, cx)))
                            .into_any_element(),
                    ))
                })
                .when(!editing, |v| {
                    v.child(
                        h_flex()
                            .px(px(4.))
                            .gap(px(4.))
                            // Insert lists the drawing's suitable sources, like the select.
                            .child(select("table-dlg-insert", "Insert".into(), 64., source_items.clone()))
                            .child(select("table-dlg-source", source_label.clone(), 120., source_items)),
                    )
                })
                .when(bom, |v| v.child(row("Template", div().w(px(120.)).text_size(px(11.)).child("Follow assembly").into_any_element())).child(row("BOM type", select("table-dlg-type", if d.structured { "Structured".into() } else { "Flattened".into() }, 120., types).into_any_element())))
                .when_some(d.table.filter(|_| editing && !bom), |v, id| {
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
                .child(row("Order", select("table-dlg-order", if d.bottom_to_top { "Bottom to top".into() } else { "Top to bottom".into() }, 120., orders).into_any_element()))
                .child(div().px(px(8.)).pt(px(4.)).text_size(px(11.)).child("Select fixed corner"))
                .child(h_flex().px(px(8.)).pt(px(4.)).gap(px(26.)).children(corners))
                .into_any_element(),
        )
    }
}
