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
        // (A revision table selects one cell at a time: Shift / Ctrl+click replace it, live2 drw-revision-*.)
        let single = matches!(self.current_sheet().table(id).map(|t| &t.kind), Some(TableKind::Revision { .. }));
        let anchor = self.cell_sel.as_ref().filter(|s| shift && !single && s.table == id).map(|s| s.a);
        self.selection = vec![Sel::Table(id)];
        self.cell_sel = Some(CellSel { table: id, a: anchor.unwrap_or(cell), b: cell });
        self.status = None;
        self.update_preview(cx);
        self.redraw(cx);
        true
    }

    /// End of a box select (left-drag from empty sheet, live navigation/44) from `a` to `b`: left to right (window)
    /// the annotations, tables, images and sketch entities wholly inside the box are selected, right to left
    /// (crossing) those it touches; `add` keeps what was selected (live2 F3: a box adds). Live2 box-select-rule
    /// (11-16): a window takes a view only with a whole model edge of it inside, and a dimension with its whole text,
    /// an arrowhead or an attachment point inside (a crossing box with that point too); a crossing box takes a view when it
    /// meets its frame.
    pub fn select_in_box(&mut self, a: [f64; 2], b: [f64; 2], add: bool, cx: &mut Context<Self>) {
        use noshape_model::drawing::font::text_box;
        let (lo, hi) = ([a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])]);
        let crossing = b[0] < a[0];
        let inside = |p: [f64; 2]| p[0] >= lo[0] && p[0] <= hi[0] && p[1] >= lo[1] && p[1] <= hi[1];
        // A drawn polyline taken: wholly inside (window), or any of its segments in the box (crossing).
        let takes = |pts: &[[f64; 2]]| match crossing {
            false => pts.iter().all(|p| inside(*p)),
            true => pts.iter().any(|p| inside(*p)) || pts.windows(2).any(|w| segment_meets_box(w[0], w[1], lo, hi)),
        };
        let frame = self.tol_mm(BOX_VIEW_FRAME_PX);
        let picked: Vec<Sel> = {
            let Some(built) = &self.built else { return };
            let pk = self.pk();
            let sheet = self.current_sheet();
            let dimension = |sel: &Sel| match sel {
                Sel::Annotation(id) => sheet.annotations.iter().find(|x| x.id == *id).filter(|x| x.kind.is_dimension()).map(|x| x.id),
                _ => None,
            };
            // Each item's drawn parts: window, all of them taken; crossing, any. A dimension in a window: any of its
            // texts, arrowheads or attachment points.
            let mut all_in: Vec<(Sel, bool)> = Vec::new();
            let mut note = |sel: Sel, ok: bool, any: bool| match all_in.iter_mut().find(|x| x.0 == sel) {
                Some(x) if crossing || any => x.1 |= ok,
                Some(x) => x.1 &= ok,
                None => all_in.push((sel, ok)),
            };
            for l in built.geometry.lines.iter().filter(|l| l.pick != 0) {
                if let Some(sel) = self.sel_of_pick(l.pick).filter(|s| !matches!(s, Sel::View(_) | Sel::Edge(..))) {
                    let mut pts = l.points.clone();
                    if l.closed && let Some(first) = pts.first().copied() {
                        pts.push(first);
                    }
                    let dim = !crossing && dimension(&sel).is_some();
                    note(sel, !dim && takes(&pts), dim);
                }
            }
            for t in built.geometry.texts.iter().filter(|t| t.pick != 0) {
                if let Some(sel) = self.sel_of_pick(t.pick).filter(|s| !matches!(s, Sel::View(_) | Sel::Edge(..))) {
                    let mut quad = text_box(t).to_vec();
                    quad.push(quad[0]);
                    let ok = takes(&quad) || (crossing && point_in_polygon(lo, &quad));
                    note(sel, ok, !crossing && dimension(&sel).is_some());
                }
            }
            if !crossing {
                for (id, fills) in &built.geometry.annotation_fills {
                    let sel = Sel::Annotation(*id);
                    if dimension(&sel).is_some() {
                        let ok = built.geometry.fills.get(fills.clone()).is_some_and(|f| f.iter().any(|f| f.points.iter().all(|p| inside(*p))));
                        note(sel, ok, true);
                    }
                }
            }
            // (Either way: live2's case (b) held only the attachment point and took the dimension both ways.)
            for x in sheet.annotations.iter().filter(|x| x.kind.is_dimension()) {
                let ok = pk.resolved_annotation(x.id).is_some_and(|r| pk.attach_points(&r).iter().any(|(_, p)| inside(*p)));
                note(Sel::Annotation(x.id), ok, true);
            }
            for pl in &built.placements {
                let ok = match crossing {
                    true => pl.min[0] - frame <= hi[0] && pl.max[0] + frame >= lo[0] && pl.min[1] - frame <= hi[1] && pl.max[1] + frame >= lo[1],
                    false => self.geoms.get(&pl.id).is_some_and(|g| whole_edge_inside(g, pl, &inside)),
                };
                note(Sel::View(pl.id), ok, false);
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

    /// The box of a box select being dragged: Onshape's window box left to right (#1651B0 at 20 %, its border at
    /// 60 %), its crossing box right to left (#FFC20E, dashed 9 on 8 off).
    pub fn render_select_box(&self) -> Option<AnyElement> {
        let Drag::SelectBox { start } = self.drag else { return None };
        let (a, b) = (self.sheet_to_local(start), self.sheet_to_local(self.cursor));
        Some(crate::viewport::box_overlay(glam::DVec2::from(a), glam::DVec2::from(b), self.select_box_style()?).into_any_element())
    }

    /// The look of the box select being dragged: the window box left to right, the crossing box right to left.
    pub fn select_box_style(&self) -> Option<crate::viewport::BoxStyle> {
        let Drag::SelectBox { start } = self.drag else { return None };
        Some(if self.cursor[0] < start[0] { crate::viewport::BoxStyle::Crossing } else { crate::viewport::BoxStyle::Window })
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

    /// The selected cells (rows r0..=r1, columns c0..=c1) grown to the merged cells they touch (live
    /// drw-hole-table-cell-panel 13: a size's SIZE cell over A1–A3).
    fn cell_sel_span(&self) -> Option<(CellSel, (usize, usize, usize, usize))> {
        let cs = self.cell_sel?;
        let l = self.built.as_ref()?.table_layout(cs.table)?;
        let sel = (*cs.rows().start(), *cs.rows().end(), *cs.cols().start(), *cs.cols().end());
        let touches = |m: &&noshape_model::drawing::CellMerge| m.row <= sel.1 && m.row + m.rows > sel.0 && m.col <= sel.3 && m.col + m.cols > sel.2;
        Some((cs, l.merges.iter().filter(touches).fold(sel, |(r0, r1, c0, c1), m| (r0.min(m.row), r1.max(m.row + m.rows - 1), c0.min(m.col), c1.max(m.col + m.cols - 1)))))
    }

    /// The selected cells' outline (sheet mm) for the overlay.
    pub fn cell_sel_rect(&self) -> Option<([f64; 2], [f64; 2])> {
        let (cs, (r0, r1, c0, c1)) = self.cell_sel_span()?;
        let l = self.built.as_ref()?.table_layout(cs.table)?;
        let t0 = l.title as usize;
        let (x0, x1) = (*l.xs.get(c0)?, *l.xs.get(c1 + 1)?);
        let (y0, y1) = (*l.ys.get(t0 + r0)?, *l.ys.get(t0 + r1 + 1)?);
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
            // A hole table's part split off is left selected (live2 drw-hole-table-split s14: its frame and grips lit).
            let hole = matches!(t.kind, TableKind::Hole { .. });
            out.push(MenuEntry::item(te::split_label(&t.kind), move |t, _, cx| {
                let mut part = None;
                let kept = t.selection.clone();
                t.edit_sheet("Split table", cx, |s| {
                    part = te::split_table(s, id, &l, split);
                });
                // The others keep what was selected (live drw-custom-table-split s13: the original, lit).
                t.selection = match part {
                    Some(p) if hole => {
                        t.cell_sel = None;
                        vec![Sel::Table(p)]
                    }
                    _ => kept,
                };
                t.redraw(cx);
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

    /// A hole table's selected cells: their holes' circles and automatic centre marks (picks) and their tags (text
    /// indices), lit blue (live drw-hole-table-cell-panel 13: the SIZE cell of A1–A3 lights A1, A2, A3).
    pub(crate) fn hole_cells_lit(&self) -> (Vec<u32>, Vec<usize>) {
        use noshape_model::drawing::build::{centermark_key, same_centre};
        use noshape_model::drawing::views::EdgeGeom;
        let none = || (Vec::new(), Vec::new());
        let (Some(b), Some((cs, (r0, r1, _, _)))) = (&self.built, self.cell_sel_span()) else { return none() };
        let Some(t) = self.current_sheet().table(cs.table) else { return none() };
        let TableKind::Hole { view, origin, exclude, x_dir, .. } = &t.kind else { return none() };
        let Some(g) = self.geoms.get(view) else { return none() };
        let tags: Vec<String> = (r0..=r1).map(|r| self.cell_text(cs.table, r, 0)).collect();
        let ctx = noshape_model::drawing::views::table_source(self.current_sheet(), t).and_then(|k| self.sources.get(&k)).map(|s| &*s.context);
        let centres: Vec<[f64; 2]> = noshape_model::drawing::tables::hole_rows(g, ctx, *origin, exclude, *x_dir).into_iter().filter(|h| tags.contains(&h.tag)).map(|h| h.center).collect();
        if centres.is_empty() {
            return none();
        }
        let at_centre = |c: [f64; 2]| centres.iter().any(|o| (o[0] - c[0]).hypot(o[1] - c[1]) < 1e-7);
        let picks = b
            .picks
            .iter()
            .enumerate()
            .filter(|(_, p)| match p {
                PickTarget::Edge { view: v, edge } if v == view => g.edges.get(*edge as usize).and_then(|e| g.model_edges.get(e.model? as usize)).is_some_and(|me| matches!(me.geom, EdgeGeom::Circle { center, .. } if at_centre(center))),
                PickTarget::AutoCentermark { view: v, center } if v == view => centres.iter().any(|c| same_centre(centermark_key(*c), *center)),
                _ => false,
            })
            .map(|(i, _)| i as u32)
            .collect();
        let tag_pick = b.picks.iter().position(|p| *p == PickTarget::HoleAxis(cs.table)).map(|i| i as u32);
        let texts = b.geometry.texts.iter().enumerate().filter(|(_, x)| Some(x.pick) == tag_pick && tags.contains(&x.text)).map(|(i, _)| i).collect();
        (picks, texts)
    }

    /// Hovering a BOM or hole table row (help TB15): the picks of the
    /// callouts showing its item number and of its part's edges in the
    /// views of that source; a hole table row's hole circles.
    /// (callout picks, geometry picks): live drawing/421 draws the callouts
    /// blue and the part's edges tan.
    pub fn table_row_highlight(&self) -> (Vec<u32>, Vec<u32>) {
        let none = || (Vec::new(), Vec::new());
        let (Some(b), Some(h)) = (&self.built, self.hover) else { return none() };
        let Some(PickTarget::Table(id)) = b.picks.get(h as usize) else { return none() };
        let Some(t) = self.current_sheet().table(*id) else { return none() };
        let Some((r, _)) = b.table_layout(*id).and_then(|l| l.cell_at(self.cursor)) else { return none() };
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
        if let TableKind::Hole { view, origin, exclude, x_dir, .. } = &t.kind {
            let tag = self.cell_text(*id, r, 0);
            let Some(g) = self.geoms.get(view) else { return none() };
            let ctx = noshape_model::drawing::views::table_source(self.current_sheet(), t).and_then(|k| self.sources.get(&k)).map(|s| &*s.context);
            let Some(row) = noshape_model::drawing::tables::hole_rows(g, ctx, *origin, exclude, *x_dir).into_iter().find(|h| h.tag == tag) else { return none() };
            let c = row.center;
            return (Vec::new(), edge_picks(*view, &|me| matches!(me.geom, noshape_model::drawing::views::EdgeGeom::Circle { center, .. } if (center[0] - c[0]).hypot(center[1] - c[1]) < 1e-7)));
        }
        // A cut list row: its frame members' edges in every view of the
        // table's source (as a BOM row's part; help TB15).
        if let TableKind::CutList { view, .. } = &t.kind {
            let Ok(n) = self.cell_text(*id, r, 0).parse::<usize>() else { return none() };
            let sheet = self.current_sheet();
            let key = noshape_model::drawing::views::view_source(sheet, *view).map(|(k, _)| k);
            // The rows as the table lists them (its own configuration).
            let table_key = noshape_model::drawing::views::table_source(sheet, t);
            let rows = noshape_model::drawing::tables::cut_list_rows(self.geoms.get(view).map(|g| g.as_ref()), table_key.as_ref().and_then(|k| self.sources.get(k)));
            let bodies = rows.into_iter().find(|row| row.item == n).map(|row| row.bodies).unwrap_or_default();
            let mut out = Vec::new();
            for v in sheet.views.iter().filter(|v| noshape_model::drawing::views::view_source(sheet, v.id).map(|(k, _)| k) == key) {
                out.extend(edge_picks(v.id, &|me| bodies.contains(&me.body)));
            }
            return (Vec::new(), out);
        }
        let TableKind::Bom { view: bom_view, .. } = &t.kind else { return none() };
        let item = self.cell_text(*id, r, 0);
        if item.is_empty() || item == "Item" {
            return none();
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
        let callouts: Vec<u32> = b.picks.iter().enumerate().filter(|(_, t)| matches!(t, PickTarget::Annotation(a) if hits.contains(a))).map(|(i, _)| i as u32).collect();
        (callouts, part_edges)
    }

    /// Spreadsheet cells on the clipboard: its HTML table (merges, formats,
    /// sizes; drawing.md TB12), else its tab-separated text.
    pub fn clipboard_cells(&self, cx: &App) -> Option<noshape_scene::drawing_tools::paste_html::PastedTable> {
        use noshape_scene::drawing_tools::paste_html::{PastedTable, parse_html_table};
        let item = cx.read_from_clipboard()?;
        item.html().and_then(parse_html_table).or_else(|| item.text().and_then(|t| parse_cells(&t)).map(PastedTable::plain))
    }

    /// Menu › Paste as table: the cells, else live's toast "Clipboard is
    /// empty." (request 1727).
    pub fn paste_as_table(&mut self, cx: &mut Context<Self>) {
        if !self.start_paste_table(cx) {
            self.show_message("Clipboard is empty.", crate::os_ui::BubbleKind::Info, Some(std::time::Duration::from_millis(2500)), cx);
        }
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
        // The placeholder grid goes with the click.
        self.update_preview(cx);
        // The clipboard is used up after one paste.
        cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
        self.status = None;
        true
    }
}

/// How far outside its geometry a view's frame lies (screen px): a right to left box meeting it takes the view (live2
/// box-select-rule: boxes 4 and 5 px off the geometry took it, the frame drawn ~7 px out).
const BOX_VIEW_FRAME_PX: f64 = 7.0;

/// Whether a whole model edge of view `g` (all its drawn pieces) lies inside, on the sheet.
fn whole_edge_inside(g: &noshape_model::drawing::views::ViewGeometry, pl: &noshape_model::drawing::build::ViewPlacement, inside: &impl Fn([f64; 2]) -> bool) -> bool {
    let all_in = |e: &noshape_model::drawing::views::ViewEdge| e.points.iter().all(|p| inside(pl.to_sheet(*p)));
    g.edges.iter().enumerate().any(|(i, e)| match e.model {
        Some(m) => g.edges.iter().position(|o| o.model == Some(m)) == Some(i) && g.edges.iter().filter(|o| o.model == Some(m)).all(all_in),
        None => all_in(e),
    })
}

/// Whether `p` lies inside closed polygon `poly` (its first point repeated last).
fn point_in_polygon(p: [f64; 2], poly: &[[f64; 2]]) -> bool {
    poly.windows(2).fold(false, |odd, w| {
        let (a, b) = (w[0], w[1]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) { !odd } else { odd }
    })
}

/// Whether segment `a`–`b` meets the box `lo`–`hi` (Liang–Barsky).
fn segment_meets_box(a: [f64; 2], b: [f64; 2], lo: [f64; 2], hi: [f64; 2]) -> bool {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for k in 0..2 {
        let d = b[k] - a[k];
        for (p, q) in [(-d, a[k] - lo[k]), (d, hi[k] - a[k])] {
            if p == 0.0 {
                if q < 0.0 {
                    return false;
                }
                continue;
            }
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}
