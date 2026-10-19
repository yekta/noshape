//! Drawing editor: table cell selection, copy and paste. Click a cell to
//! select it, Shift+click another to select the block between them (a row
//! or a column); Ctrl+C copies the cells as tab-separated text (Excel /
//! Sheets / another drawing). Ctrl+V (or "Paste as table") with
//! spreadsheet text on the clipboard places a new editable table with its
//! top-left corner at the next click; the clipboard is used up.

use crate::drawing_view::*;
use gpui_kit::*;
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::{Table, TableCorner, TableKind};

/// BOM / cut list tables showing the same rows (same kind and view).
fn same_list(a: &TableKind, b: &TableKind) -> bool {
    match (a, b) {
        (TableKind::Bom { view: x, .. }, TableKind::Bom { view: y, .. }) | (TableKind::CutList { view: x, .. }, TableKind::CutList { view: y, .. }) => x == y,
        _ => false,
    }
}

/// Selected cells of a table: two corners (row, column), rows counted below
/// the title (the header is row 0), like `TableLayout::cell_at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellSel {
    pub table: u64,
    pub a: (usize, usize),
    pub b: (usize, usize),
}

impl CellSel {
    pub fn rows(&self) -> std::ops::RangeInclusive<usize> {
        self.a.0.min(self.b.0)..=self.a.0.max(self.b.0)
    }
    pub fn cols(&self) -> std::ops::RangeInclusive<usize> {
        self.a.1.min(self.b.1)..=self.a.1.max(self.b.1)
    }
}

/// Spreadsheet cells from clipboard text (tab-separated rows), if it looks
/// like a table (a tab, or several lines).
pub fn parse_cells(text: &str) -> Option<Vec<Vec<String>>> {
    let text = text.trim_end_matches(['\n', '\r']);
    if text.is_empty() || (!text.contains('\t') && !text.contains('\n')) || text.trim_start().starts_with('{') {
        return None;
    }
    let rows: Vec<Vec<String>> = text.lines().map(|l| l.split('\t').map(|c| c.trim_matches('"').to_string()).collect()).collect();
    let n = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    Some(rows.into_iter().map(|mut r| {
        r.resize(n, String::new());
        r
    }).collect())
}

impl DrawingEditor {
    /// A click in Select mode on a table: select the cell (Shift: extend the
    /// block). True when handled. Clicking elsewhere clears the cells.
    pub fn table_cell_click(&mut self, sel: Option<Sel>, shift: bool, cx: &mut Context<Self>) -> bool {
        let Some(Sel::Table(id)) = sel else {
            self.cell_sel = None;
            return false;
        };
        let Some(cell) = self.built.as_ref().and_then(|b| b.table_layout(id)).and_then(|l| l.cell_at(self.cursor)) else {
            self.cell_sel = None;
            return false;
        };
        match self.cell_sel {
            Some(cs) if shift && cs.table == id => self.cell_sel = Some(CellSel { b: cell, ..cs }),
            _ => {
                self.selection = vec![Sel::Table(id)];
                self.cell_sel = Some(CellSel { table: id, a: cell, b: cell });
            }
        }
        self.status = None;
        self.update_preview(cx);
        self.redraw(cx);
        true
    }

    /// The selected cells' outline (sheet mm) for the overlay.
    pub fn cell_sel_rect(&self) -> Option<([f64; 2], [f64; 2])> {
        let cs = self.cell_sel?;
        let l = self.built.as_ref()?.table_layout(cs.table)?;
        let t0 = l.title as usize;
        let (r, c) = (cs.rows(), cs.cols());
        let (x0, x1) = (*l.xs.get(*c.start())?, *l.xs.get(*c.end() + 1)?);
        let (y0, y1) = (*l.ys.get(t0 + *r.start())?, *l.ys.get(t0 + *r.end() + 1)?);
        Some(([x0, y1], [x1, y0]))
    }

    /// Ctrl+C with cells selected: their text, tab-separated. True when used.
    pub fn copy_cells(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(cs) = self.cell_sel else { return false };
        let text: Vec<String> = cs.rows().map(|r| cs.cols().map(|c| self.cell_text(cs.table, r, c)).collect::<Vec<_>>().join("\t")).collect();
        cx.write_to_clipboard(ClipboardItem::new_string(text.join("\n")));
        let n = cs.rows().count() * cs.cols().count();
        self.status = Some(if n == 1 { "Copied 1 cell".into() } else { format!("Copied {n} cells").into() });
        cx.notify();
        true
    }

    /// BOM / cut list row menu: "Split table below" (the rows after the
    /// clicked one move to a new table beside it) and, on the second part,
    /// "Unsplit table".
    pub fn split_entries(&self, t: &Table, cell: Option<(usize, usize)>) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let id = t.id;
        let Some(l) = self.built.as_ref().and_then(|b| b.table_layout(id)).cloned() else { return out };
        // Data rows shown (the header is one displayed row).
        let shown = l.ys.len().saturating_sub(1 + l.title as usize + 1);
        let [a, b] = t.rows.unwrap_or([0, shown]);
        let header_bottom = matches!(t.kind, TableKind::Bom { bottom_to_top: true, .. } | TableKind::CutList { bottom_to_top: true, .. });
        if let Some((r, _)) = cell {
            let data_r = if header_bottom { r } else { r.wrapping_sub(1) };
            if data_r < shown && data_r + 1 < shown {
                let split = a + data_r + 1;
                let width = l.xs.last().copied().unwrap_or(0.0) - l.xs.first().copied().unwrap_or(0.0);
                out.push(MenuEntry::item("Split table below", move |t, _, cx| {
                    let Some(orig) = t.current_sheet().table(id).cloned() else { return };
                    let nid = t.current_sheet().next_table_id();
                    let mut second = orig.clone();
                    second.id = nid;
                    second.rows = Some([split, b]);
                    // Beside the first part: to the right, else to the left.
                    let (sw, _) = t.current_sheet().size.size_mm();
                    let right_edge = l.xs.last().copied().unwrap_or(0.0) + width + 6.0;
                    second.position[0] += if right_edge <= sw { width + 6.0 } else { -(width + 6.0) };
                    t.edit_sheet("Split table", cx, |s| {
                        if let Some(x) = s.table_mut(id) {
                            x.rows = Some([a, split]);
                        }
                        s.tables.push(second);
                    });
                }));
            }
        }
        if a > 0 {
            let kind = t.kind.clone();
            out.push(MenuEntry::item("Unsplit table", move |t, _, cx| {
                let kind = kind.clone();
                t.edit_sheet("Unsplit table", cx, |s| {
                    // A later part continues after this one?
                    let more = s.tables.iter().any(|x| x.id != id && same_list(&x.kind, &kind) && x.rows.is_some_and(|r| r[0] == b));
                    if let Some(prev) = s.tables.iter_mut().find(|x| x.id != id && same_list(&x.kind, &kind) && x.rows.is_some_and(|r| r[1] == a)) {
                        let start = prev.rows.map(|r| r[0]).unwrap_or(0);
                        prev.rows = if start == 0 && !more { None } else { Some([start, b]) };
                    }
                    s.tables.retain(|x| x.id != id);
                });
            }));
        }
        out
    }

    /// The text drawn in a table cell (resolved fields, BOM / hole values).
    pub fn cell_text(&self, table: u64, r: usize, c: usize) -> String {
        let Some(b) = &self.built else { return String::new() };
        let Some(l) = b.table_layout(table) else { return String::new() };
        let picks: Vec<u32> = b.picks.iter().enumerate().filter(|(_, t)| matches!(t, PickTarget::Table(x) if *x == table)).map(|(i, _)| i as u32).collect();
        let t0 = l.title as usize;
        let (Some(&x0), Some(&x1), Some(&ya), Some(&yb)) = (l.xs.get(c), l.xs.get(c + 1), l.ys.get(t0 + r), l.ys.get(t0 + r + 1)) else { return String::new() };
        let mut ts: Vec<&noshape_model::drawing::sheet::Text2> = b
            .geometry
            .texts
            .iter()
            .filter(|t| picks.contains(&t.pick))
            .filter(|t| {
                let p = noshape_model::drawing::font::text_box(t).iter().fold([0.0, 0.0], |a, q| [a[0] + q[0] / 4.0, a[1] + q[1] / 4.0]);
                p[0] > x0 && p[0] < x1 && p[1] < ya && p[1] > yb
            })
            .collect();
        ts.sort_by(|a, b| b.pos[1].total_cmp(&a.pos[1]).then(a.pos[0].total_cmp(&b.pos[0])));
        ts.dedup_by(|a, b| a.text == b.text && (a.pos[1] - b.pos[1]).abs() < 1e-6);
        ts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ")
    }

    /// Hovering a BOM row: the picks of the callouts showing its item
    /// number (Onshape highlights them).
    pub fn table_row_highlight(&self) -> Vec<u32> {
        let (Some(b), Some(h)) = (&self.built, self.hover) else { return Vec::new() };
        let Some(PickTarget::Table(id)) = b.picks.get(h as usize) else { return Vec::new() };
        let Some(t) = self.current_sheet().table(*id) else { return Vec::new() };
        if !matches!(t.kind, TableKind::Bom { .. }) {
            return Vec::new();
        }
        let Some((r, _)) = b.table_layout(*id).and_then(|l| l.cell_at(self.cursor)) else { return Vec::new() };
        let item = self.cell_text(*id, r, 0);
        if item.is_empty() || item == "ITEM" {
            return Vec::new();
        }
        // Callouts whose text is the item number.
        let balloons: Vec<u64> = self.current_sheet().annotations.iter().filter(|a| matches!(a.kind, noshape_model::drawing::AnnotationKind::Balloon { .. })).map(|a| a.id).collect();
        let hits: Vec<u64> = b
            .geometry
            .texts
            .iter()
            .filter(|t| t.text == item)
            .filter_map(|t| match b.picks.get(t.pick as usize) {
                Some(PickTarget::Annotation(a)) if balloons.contains(a) => Some(*a),
                _ => None,
            })
            .collect();
        b.picks.iter().enumerate().filter(|(_, t)| matches!(t, PickTarget::Annotation(a) if hits.contains(a))).map(|(i, _)| i as u32).collect()
    }

    /// Double-click a BOM cell: its value with "Override" (grayed out when
    /// the row has no part number).
    pub fn open_bom_cell(&mut self, t: &Table, window: &mut Window, cx: &mut Context<Self>) {
        let Some((r, c)) = self.built.as_ref().and_then(|b| b.table_layout(t.id)).and_then(|l| l.cell_at(self.cursor)) else { return };
        let item = self.cell_text(t.id, r, 0);
        if item.is_empty() || item == "ITEM" {
            return;
        }
        let value = self.cell_text(t.id, r, c);
        let enabled = !self.cell_text(t.id, r, 3).trim().is_empty();
        let over = matches!(&t.kind, TableKind::Bom { overrides, .. } if overrides.iter().any(|(i, k, _)| *i == item && *k == c));
        let input = self.input(&value, window, cx);
        self.panel = Some(Panel::BomCell { table: t.id, item, col: c, input, over, enabled });
        cx.notify();
    }

    pub fn bom_cell_rows(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        use gpui_kit::component::checkbox::Checkbox;
        use gpui_kit::component::input::Input;
        use gpui_kit::component::Sizable as _;
        use gpui_kit::prelude::FluentBuilder as _;
        let Some(Panel::BomCell { input, over, enabled, .. }) = &self.panel else { return Vec::new() };
        let (input, over, enabled) = (input.clone(), *over, *enabled);
        vec![
            div().w(px(220.)).when(!over, |d| d.opacity(0.5)).child(Input::new(&input).small()).into_any_element(),
            div()
                .when(!enabled, |d| d.opacity(0.4))
                .child(Checkbox::new("bom-override").label("Override").small().checked(over).when(enabled, |c| c.on_click(cx.listener(|this, v: &bool, _, cx| {
                    if let Some(Panel::BomCell { over, .. }) = &mut this.panel {
                        *over = *v;
                    }
                    cx.notify();
                }))))
                .into_any_element(),
        ]
    }

    pub fn apply_bom_cell(&mut self, table: u64, item: String, col: usize, value: String, over: bool, cx: &mut Context<Self>) {
        self.edit_table(table, "BOM cell override", cx, |t| {
            if let TableKind::Bom { overrides, .. } = &mut t.kind {
                overrides.retain(|(i, k, _)| !(*i == item && *k == col));
                if over {
                    overrides.push((item, col, value));
                }
            }
            t.col_widths.clear();
        });
    }

    /// Spreadsheet cells on the clipboard.
    pub fn clipboard_cells(&self, cx: &App) -> Option<Vec<Vec<String>>> {
        cx.read_from_clipboard().and_then(|c| c.text()).and_then(|t| parse_cells(&t))
    }

    /// Paste as table: the next click places the cells.
    pub fn start_paste_table(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(cells) = self.clipboard_cells(cx) else { return false };
        self.paste_table = Some(cells);
        self.status = Some("Click to place the table (top-left corner)".into());
        cx.notify();
        true
    }

    /// The click that places a pasted table. True when used.
    pub fn place_paste_table(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(cells) = self.paste_table.take() else { return false };
        let id = self.current_sheet().next_table_id();
        let mut t = Table::new(id, TableKind::General { cells, title: None, header: false }, self.cursor);
        t.corner = TableCorner::TopLeft;
        self.edit_sheet("Paste as table", cx, |s| s.tables.push(t));
        self.selection = vec![Sel::Table(id)];
        // The clipboard is used up after one paste.
        cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
        self.status = None;
        true
    }
}

