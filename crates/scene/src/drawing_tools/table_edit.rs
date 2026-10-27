//! The drawing table menus' edits (cells, size, split, fixed corner, text
//! height), shared by the drawing editor and the agent API. Rows count below
//! the title (the header is row 0), like `TableLayout::cell_at`.

use noshape_model::drawing::tables::TableLayout;
use noshape_model::drawing::{CellAlign, CellFormat, CellMerge, Sheet, Table, TableCorner, TableKind};

/// Rows (below the title) and columns of a laid-out table.
pub fn counts(l: &TableLayout) -> (usize, usize) {
    (l.ys.len().saturating_sub(1 + l.title as usize), l.xs.len().saturating_sub(1))
}

fn merge_of(t: &Table, r: usize, c: usize) -> CellMerge {
    let (r, c) = t.anchor_cell(r, c);
    t.merge_at(r, c).unwrap_or(CellMerge { row: r, col: c, rows: 1, cols: 1 })
}

/// Merge with cell right (false at the last column).
pub fn merge_right(t: &mut Table, l: &TableLayout, r: usize, c: usize) -> bool {
    let m = merge_of(t, r, c);
    m.col + m.cols < counts(l).1 && t.merge_cells((m.row, m.col), (m.row + m.rows - 1, m.col + m.cols))
}

/// Merge with cell below (false at the last row).
pub fn merge_below(t: &mut Table, l: &TableLayout, r: usize, c: usize) -> bool {
    let m = merge_of(t, r, c);
    m.row + m.rows < counts(l).0 && t.merge_cells((m.row, m.col), (m.row + m.rows, m.col + m.cols - 1))
}

/// Size ▸ Columns equally (the whole table: every column the average width).
pub fn columns_equally(t: &mut Table, l: &TableLayout) {
    let n = counts(l).1;
    let avg = (l.xs.last().copied().unwrap_or(0.0) - l.xs.first().copied().unwrap_or(0.0)) / n.max(1) as f64;
    t.col_widths = vec![avg; n];
}

/// Size ▸ Rows equally (the whole table: the tallest row's height).
pub fn rows_equally(t: &mut Table, l: &TableLayout) {
    t.row_height = l.ys.windows(2).skip(l.title as usize).map(|w| w[0] - w[1]).fold(0.0, f64::max);
}

/// Size ▸ Columns Equally with cells selected: those columns share their
/// average width.
pub fn columns_equally_of(t: &mut Table, l: &TableLayout, cols: &[usize]) {
    let mut w: Vec<f64> = l.xs.windows(2).map(|w| w[1] - w[0]).collect();
    let cols: Vec<usize> = cols.iter().copied().filter(|c| *c < w.len()).collect();
    let avg = cols.iter().map(|c| w[*c]).sum::<f64>() / cols.len().max(1) as f64;
    cols.iter().for_each(|c| w[*c] = avg);
    t.col_widths = w;
}

/// Size ▸ Rows Equally with cells selected: those rows share their average
/// height.
pub fn rows_equally_of(t: &mut Table, l: &TableLayout, rows: &[usize]) {
    let mut h: Vec<f64> = l.ys.windows(2).skip(l.title as usize).map(|w| w[0] - w[1]).collect();
    let rows: Vec<usize> = rows.iter().copied().filter(|r| *r < h.len()).collect();
    let avg = rows.iter().map(|r| h[*r]).sum::<f64>() / rows.len().max(1) as f64;
    rows.iter().for_each(|r| h[*r] = avg);
    t.row_heights = h;
}

/// Format cell…: which cells the format applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatScope {
    Cell,
    Row,
    Column,
    Table,
}

/// Format cell…'s fields: alignment, bold and text height (None: the
/// table's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormatFields {
    pub align: CellAlign,
    pub bold: bool,
    pub height: Option<f64>,
}

/// Format cell… ✓ on the cell, its row, its column or the whole table.
pub fn format_cells(t: &mut Table, l: Option<&TableLayout>, row: usize, col: usize, scope: FormatScope, style: FormatFields) {
    let (nrows, ncols) = l.map(counts).unwrap_or((row + 1, col + 1));
    let cells: Vec<(usize, usize)> = match scope {
        FormatScope::Row => (0..ncols).map(|c| (row, c)).collect(),
        FormatScope::Column => (0..nrows).map(|r| (r, col)).collect(),
        FormatScope::Table => (0..nrows).flat_map(|r| (0..ncols).map(move |c| (r, c))).collect(),
        FormatScope::Cell => vec![(row, col)],
    };
    for (r, c) in cells {
        let f = CellFormat { align: style.align, height: style.height, bold: style.bold, ..t.shown_cell_format(r, c) };
        t.set_cell_format(r, c, f);
    }
    t.col_widths.clear();
}

/// Resize… ✓: column `col`'s width and the row height (mm; each capped at
/// the sheet's size).
pub fn resize_cell(t: &mut Table, l: Option<&TableLayout>, sheet_mm: (f64, f64), col: usize, width: Option<f64>, height: Option<f64>) {
    let widths: Vec<f64> = l.map(|l| l.xs.windows(2).map(|x| x[1] - x[0]).collect()).unwrap_or_default();
    if let Some(w) = width.map(|w| w.min(sheet_mm.0)) {
        if t.col_widths.len() != widths.len() {
            t.col_widths = widths;
        }
        if let Some(x) = t.col_widths.get_mut(col) {
            *x = w;
        }
    }
    if let Some(h) = height.map(|h| h.min(sheet_mm.1)) {
        t.row_height = h;
    }
}

/// Fixed corner ▸: the table stays where it is, anchored at corner `k`.
pub fn set_fixed_corner(t: &mut Table, l: Option<&TableLayout>, k: TableCorner) {
    if let Some(l) = l {
        let (lo, hi) = (l.min(), l.max());
        t.position = match k {
            TableCorner::TopLeft => [lo[0], hi[1]],
            TableCorner::TopRight => [hi[0], hi[1]],
            TableCorner::BottomLeft => [lo[0], lo[1]],
            TableCorner::BottomRight => [hi[0], lo[1]],
        };
    }
    t.corner = k;
}

/// The Text height ▸ choices (mm).
pub const TEXT_HEIGHTS: [f64; 4] = [2.5, 3.0, 3.5, 5.0];

/// Text height ▸ (the columns refit).
pub fn set_text_height(t: &mut Table, h: f64) {
    t.text_height = h;
    t.own_text_height = true;
    t.col_widths.clear();
    t.row_height = (h * 2.2).max(7.0);
}

/// Fit to content.
pub fn fit_to_content(t: &mut Table) {
    t.col_widths.clear();
    t.row_height = (t.text_height * 2.2).max(7.0);
}

/// Two parts of one split list (the same BOM / cut list view, or the same
/// custom table).
pub fn same_list(a: &TableKind, b: &TableKind) -> bool {
    match (a, b) {
        (TableKind::Bom { view: x, .. }, TableKind::Bom { view: y, .. }) | (TableKind::CutList { view: x, .. }, TableKind::CutList { view: y, .. }) => x == y,
        (TableKind::Custom { .. }, TableKind::Custom { .. }) => noshape_model::drawing::tables::CustomTableKey::of(a) == noshape_model::drawing::tables::CustomTableKey::of(b),
        _ => false,
    }
}

/// The split menu item's label of a table kind.
pub fn split_label(k: &TableKind) -> &'static str {
    // Live custom-tables-14: "Split custom table below".
    match k {
        TableKind::Custom { .. } => "Split custom table below",
        TableKind::Bom { .. } => "Split BOM table below",
        // Help (cut_list_table): "Split cut list table below".
        TableKind::CutList { .. } => "Split cut list table below",
        _ => "Split table below",
    }
}

/// The second part's "Unsplit …" (help cut_list_table: "Unsplit cut list table").
pub fn unsplit_label(k: &TableKind) -> &'static str {
    match k {
        TableKind::CutList { .. } => "Unsplit cut list table",
        _ => "Unsplit table",
    }
}

/// The data rows a part of a list shows: [first, end).
fn shown_rows(t: &Table, l: &TableLayout) -> [usize; 2] {
    let shown = l.ys.len().saturating_sub(1 + l.title as usize + 1);
    t.rows.unwrap_or([0, shown])
}

/// Where Split … below at displayed row `r` cuts the list (the data row the
/// second part starts at), if it can.
pub fn split_at(t: &Table, l: &TableLayout, r: usize) -> Option<usize> {
    let shown = l.ys.len().saturating_sub(1 + l.title as usize + 1);
    let header_bottom = matches!(t.kind, TableKind::Bom { bottom_to_top: true, .. } | TableKind::CutList { bottom_to_top: true, .. } | TableKind::Custom { bottom_to_top: true, .. });
    let data_r = if header_bottom { r } else { r.wrapping_sub(1) };
    (data_r < shown && data_r + 1 < shown).then(|| shown_rows(t, l)[0] + data_r + 1)
}

/// Split … below: the rows after `split` go to a new part beside this one
/// (to the right, else to the left). Returns its id.
pub fn split_table(s: &mut Sheet, id: u64, l: &TableLayout, split: usize) -> Option<u64> {
    let orig = s.table(id)?.clone();
    let [a, b] = shown_rows(&orig, l);
    let nid = s.next_table_id();
    let width = l.xs.last().copied().unwrap_or(0.0) - l.xs.first().copied().unwrap_or(0.0);
    let mut second = orig;
    second.id = nid;
    second.rows = Some([split, b]);
    let (sw, _) = s.size.size_mm();
    let right_edge = l.xs.last().copied().unwrap_or(0.0) + width + 6.0;
    second.position[0] += if right_edge <= sw { width + 6.0 } else { -(width + 6.0) };
    if let Some(x) = s.table_mut(id) {
        x.rows = Some([a, split]);
    }
    s.tables.push(second);
    Some(nid)
}

/// Unsplit table: this part's rows go back to the part before it (a later
/// part's rows stay split off). False for a first part.
pub fn unsplit_table(s: &mut Sheet, id: u64, l: &TableLayout) -> bool {
    let Some(t) = s.table(id).cloned() else { return false };
    let [a, b] = shown_rows(&t, l);
    if a == 0 {
        return false;
    }
    // A later part continues after this one?
    let more = s.tables.iter().any(|x| x.id != id && same_list(&x.kind, &t.kind) && x.rows.is_some_and(|r| r[0] == b));
    if let Some(prev) = s.tables.iter_mut().find(|x| x.id != id && same_list(&x.kind, &t.kind) && x.rows.is_some_and(|r| r[1] == a)) {
        let start = prev.rows.map(|r| r[0]).unwrap_or(0);
        prev.rows = if start == 0 && !more { None } else { Some([start, b]) };
    }
    s.tables.retain(|x| x.id != id);
    true
}

/// Spreadsheet cells from clipboard text (tab-separated rows), if it looks
/// like a table (a tab, or several lines): Paste as table.
pub fn parse_cells(text: &str) -> Option<Vec<Vec<String>>> {
    let text = text.trim_end_matches(['\n', '\r']);
    if text.is_empty() || (!text.contains('\t') && !text.contains('\n')) || text.trim_start().starts_with('{') {
        return None;
    }
    let rows: Vec<Vec<String>> = text.lines().map(|l| l.split('\t').map(|c| c.trim_matches('"').to_string()).collect()).collect();
    let n = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    Some(
        rows.into_iter()
            .map(|mut r| {
                r.resize(n, String::new());
                r
            })
            .collect(),
    )
}

/// Paste as table: a general table of `cells` with its top-left corner at
/// `at` (sheet mm), one edit "Paste as table".
pub fn pasted_table(id: u64, cells: Vec<Vec<String>>, at: [f64; 2]) -> Table {
    let mut t = Table::new(id, TableKind::General { cells, title: None, header: false }, at);
    t.corner = TableCorner::TopLeft;
    t
}
