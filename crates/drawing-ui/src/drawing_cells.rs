//! Drawing editor: table cell selection, copy and paste. Click a cell to
//! select it, drag across cells to select a block; Ctrl+C copies the cells as tab-separated text (Excel /
//! Sheets / another drawing). Ctrl+V (or "Paste as table") with
//! spreadsheet text on the clipboard places a new editable table with its
//! top-left corner at the next click; the clipboard is used up.

use crate::drawing_view::*;
use gpui_kit::*;
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::{Table, TableKind, ViewId};

/// BOM / cut list tables showing the same rows (same kind and view).
use noshape_scene::drawing_tools::table_edit as te;

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

/// Spreadsheet cells from clipboard text (shared with the agent API).
pub use noshape_scene::drawing_tools::table_edit::parse_cells;

impl DrawingEditor {
    /// A click in Select mode on a table: select the cell (Shift: extend the
    /// block). True when handled. Clicking elsewhere clears the cells.
    pub fn table_cell_click(&mut self, sel: Option<Sel>, shift: bool, cx: &mut Context<Self>) -> bool {
        let Some(Sel::Table(id)) = sel else {
            self.cell_sel = None;
            return false;
        };
        // Help (bom_table, cut_list_table, drawing_hole_table): "Click the
        // edge of the table to select the entire table".
        let lists = matches!(self.current_sheet().table(id).map(|t| &t.kind), Some(TableKind::Bom { .. } | TableKind::CutList { .. } | TableKind::Hole { .. }));
        let tol = self.tol_mm(4.0);
        let on_edge = self.built.as_ref().and_then(|b| b.table_layout(id)).is_some_and(|l| {
            let (lo, hi, c) = (l.min(), l.max(), self.cursor);
            [c[0] - lo[0], c[0] - hi[0], c[1] - lo[1], c[1] - hi[1]].iter().any(|d| d.abs() < tol)
        });
        if lists && on_edge {
            self.selection = vec![Sel::Table(id)];
            self.cell_sel = None;
            self.status = None;
            self.update_preview(cx);
            self.redraw(cx);
            return true;
        }
        let Some(cell) = self.built.as_ref().and_then(|b| b.table_layout(id)).and_then(|l| l.cell_at(self.cursor)) else {
            self.cell_sel = None;
            return false;
        };
        // Live (behaviours "qa (2026-10-28) drawing.md TB7", request 1567):
        // Shift+click extends the cell selection to the block from its
        // first cell (qa DWG-TABLE-SHIFT-EXTEND).
        let anchor = self.cell_sel.as_ref().filter(|s| shift && s.table == id).map(|s| s.a);
        self.selection = vec![Sel::Table(id)];
        self.cell_sel = Some(CellSel { table: id, a: anchor.unwrap_or(cell), b: cell });
        self.status = None;
        self.update_preview(cx);
        self.redraw(cx);
        true
    }

    /// End of a box select (left-drag from empty sheet, live navigation/44):
    /// the views, annotations, tables, images and sketch entities wholly
    /// inside the box `a`–`b` are selected; Shift adds to the selection.
    pub fn select_in_box(&mut self, a: [f64; 2], b: [f64; 2], add: bool, cx: &mut Context<Self>) {
        let (lo, hi) = ([a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])]);
        let inside = |p: [f64; 2]| p[0] >= lo[0] && p[0] <= hi[0] && p[1] >= lo[1] && p[1] <= hi[1];
        let picked: Vec<Sel> = {
            let Some(built) = &self.built else { return };
            // Each item's drawn points: all inside, it is taken.
            let mut all_in: Vec<(Sel, bool)> = Vec::new();
            let mut note = |sel: Sel, ok: bool| match all_in.iter_mut().find(|x| x.0 == sel) {
                Some(x) => x.1 &= ok,
                None => all_in.push((sel, ok)),
            };
            for l in built.geometry.lines.iter().filter(|l| l.pick != 0) {
                if let Some(sel) = self.sel_of_pick(l.pick).filter(|s| !matches!(s, Sel::View(_))) {
                    note(sel, l.points.iter().all(|p| inside(*p)));
                }
            }
            for t in built.geometry.texts.iter().filter(|t| t.pick != 0) {
                if let Some(sel) = self.sel_of_pick(t.pick).filter(|s| !matches!(s, Sel::View(_))) {
                    note(sel, inside(t.pos));
                }
            }
            for pl in &built.placements {
                note(Sel::View(pl.id), inside(pl.min) && inside(pl.max));
            }
            all_in.into_iter().filter(|x| x.1).map(|x| x.0).collect()
        };
        if !add {
            self.selection.clear();
        }
        for s in picked {
            if !self.selection.contains(&s) {
                self.selection.push(s);
            }
        }
        self.cell_sel = None;
        self.redraw(cx);
    }

    /// The box of a box select being dragged: live's window box (#1651B0 at
    /// 20 %, its border at 60 %).
    pub fn render_select_box(&self) -> Option<AnyElement> {
        let Drag::SelectBox { start } = self.drag else { return None };
        let (a, b) = (self.sheet_to_local(start), self.sheet_to_local(self.cursor));
        let (x0, y0, x1, y1) = (a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1]));
        Some(
            div()
                .absolute()
                .left(px(x0 as f32))
                .top(px(y0 as f32))
                .w(px((x1 - x0) as f32))
                .h(px((y1 - y0) as f32))
                .bg(gpui_kit::rgba(0x1651b033))
                .border_1()
                .border_color(gpui_kit::rgba(0x1651b099))
                .into_any_element(),
        )
    }

    /// End of a cell drag-selection: the cells the rubber band `a`–`b`
    /// touches (below the title row) become the selected block.
    pub fn select_cells_in(&mut self, id: u64, a: [f64; 2], b: [f64; 2], cx: &mut Context<Self>) {
        let Some(l) = self.built.as_ref().and_then(|x| x.table_layout(id)).cloned() else { return };
        let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
        let (y0, y1) = (a[1].min(b[1]), a[1].max(b[1]));
        let cols: Vec<usize> = l.xs.windows(2).enumerate().filter(|(_, w)| w[1] >= x0 && w[0] <= x1).map(|(i, _)| i).collect();
        let t0 = l.title as usize;
        let rows: Vec<usize> = l.ys.windows(2).enumerate().skip(t0).filter(|(_, w)| w[0] >= y0 && w[1] <= y1).map(|(i, _)| i - t0).collect();
        if let (Some(&c0), Some(&c1), Some(&r0), Some(&r1)) = (cols.first(), cols.last(), rows.first(), rows.last()) {
            self.selection = vec![Sel::Table(id)];
            self.cell_sel = Some(CellSel { table: id, a: (r0, c0), b: (r1, c1) });
        }
        self.update_preview(cx);
        self.redraw(cx);
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
        if let Some(split) = cell.and_then(|(r, _)| te::split_at(t, &l, r)) {
            let l = l.clone();
            out.push(MenuEntry::item(te::split_label(&t.kind), move |t, _, cx| {
                t.edit_sheet("Split table", cx, |s| {
                    te::split_table(s, id, &l, split);
                });
            }));
        }
        if t.rows.is_some_and(|r| r[0] > 0) {
            out.push(MenuEntry::item(te::unsplit_label(&t.kind), move |t, _, cx| {
                t.edit_sheet("Unsplit table", cx, |s| {
                    te::unsplit_table(s, id, &l);
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

    /// Hovering a BOM or hole table row (help TB15): the picks of the
    /// callouts showing its item number and of its part's edges in the
    /// views of that source; a hole table row's hole circles.
    pub fn table_row_highlight(&self) -> Vec<u32> {
        let (Some(b), Some(h)) = (&self.built, self.hover) else { return Vec::new() };
        let Some(PickTarget::Table(id)) = b.picks.get(h as usize) else { return Vec::new() };
        let Some(t) = self.current_sheet().table(*id) else { return Vec::new() };
        let Some((r, _)) = b.table_layout(*id).and_then(|l| l.cell_at(self.cursor)) else { return Vec::new() };
        // Edge picks of `view` whose model edge passes `keep`.
        let edge_picks = |view: ViewId, keep: &dyn Fn(&noshape_model::drawing::views::ModelEdge) -> bool| -> Vec<u32> {
            let Some(g) = self.geoms.get(&view) else { return Vec::new() };
            b.picks
                .iter()
                .enumerate()
                .filter_map(|(i, p)| match p {
                    PickTarget::Edge { view: v, edge } if *v == view => g.edges.get(*edge as usize).and_then(|e| g.model_edges.get(e.model? as usize)).filter(|me| keep(me)).map(|_| i as u32),
                    _ => None,
                })
                .collect()
        };
        if let TableKind::Hole { view, origin, .. } = &t.kind {
            let tag = self.cell_text(*id, r, 0);
            let Some(g) = self.geoms.get(view) else { return Vec::new() };
            let Some(row) = noshape_model::drawing::tables::hole_rows(g, *origin).into_iter().find(|h| h.tag == tag) else { return Vec::new() };
            let c = row.center;
            return edge_picks(*view, &|me| matches!(me.geom, noshape_model::drawing::views::EdgeGeom::Circle { center, .. } if (center[0] - c[0]).hypot(center[1] - c[1]) < 1e-7));
        }
        let TableKind::Bom { view: bom_view, .. } = &t.kind else { return Vec::new() };
        let item = self.cell_text(*id, r, 0);
        if item.is_empty() || item == "Item" {
            return Vec::new();
        }
        // The row's part in every view of the table's source.
        let sheet = self.current_sheet();
        let key = noshape_model::drawing::views::view_source(sheet, *bom_view).map(|(k, _)| k);
        let bodies: Vec<_> = key
            .as_ref()
            .and_then(|k| self.sources.get(k))
            .and_then(|s| item.parse::<usize>().ok().and_then(|n| s.bom.iter().find(|e| e.item == n)))
            .map(|e| e.bodies.clone())
            .unwrap_or_default();
        let mut part_edges: Vec<u32> = Vec::new();
        if !bodies.is_empty() {
            for v in sheet.views.iter().filter(|v| noshape_model::drawing::views::view_source(sheet, v.id).map(|(k, _)| k) == key) {
                part_edges.extend(edge_picks(v.id, &|me| bodies.contains(&me.body)));
            }
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
        let mut out: Vec<u32> = b.picks.iter().enumerate().filter(|(_, t)| matches!(t, PickTarget::Annotation(a) if hits.contains(a))).map(|(i, _)| i as u32).collect();
        out.extend(part_edges);
        out
    }

    /// Double-click a BOM cell: its value with "Override" (grayed out when
    /// the row has no part number).
    pub fn open_bom_cell(&mut self, t: &Table, window: &mut Window, cx: &mut Context<Self>) {
        let Some((r, c)) = self.built.as_ref().and_then(|b| b.table_layout(t.id)).and_then(|l| l.cell_at(self.cursor)) else { return };
        let item = self.cell_text(t.id, r, 0);
        if item.is_empty() || item == "Item" {
            return;
        }
        let value = self.cell_text(t.id, r, c);
        let enabled = !self.cell_text(t.id, r, 2).trim().is_empty();
        let over = matches!(&t.kind, TableKind::Bom { overrides, .. } if overrides.iter().any(|(i, k, _)| *i == item && *k == c));
        let input = self.input(&value, window, cx);
        self.panel = Some(Panel::BomCell { table: t.id, item, col: c, input, over, enabled });
        cx.notify();
    }

    pub fn bom_cell_rows(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
                use gpui_kit::component::input::Input;
        use gpui_kit::component::Sizable as _;
        use gpui_kit::prelude::FluentBuilder as _;
        let Some(Panel::BomCell { input, over, enabled, .. }) = &self.panel else { return Vec::new() };
        let (input, over, enabled) = (input.clone(), *over, *enabled);
        vec![
            div().w(px(220.)).when(!over, |d| d.opacity(0.5)).child(Input::new(&input).small()).into_any_element(),
            div()
                .when(!enabled, |d| d.opacity(0.4))
                .child(crate::drawing_chrome::DrCheckbox::new("bom-override").label("Override").checked(over).when(enabled, |c| c.on_click(cx.listener(|this, v: &bool, _, cx| {
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
        let t = noshape_scene::drawing_tools::table_edit::pasted_table(id, cells, self.cursor);
        self.edit_sheet("Paste as table", cx, |s| s.tables.push(t));
        self.selection = vec![Sel::Table(id)];
        // The clipboard is used up after one paste.
        cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
        self.status = None;
        true
    }
}

