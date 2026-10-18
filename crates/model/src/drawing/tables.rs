//! Drawing tables: general tables, bills of materials (from an assembly
//! view) and hole tables (from a view). Content is resolved when the sheet
//! is built; layout and rendering produce plain sheet geometry so tables
//! appear in every export.

use super::font;
use super::format::{default_digits, format_number, length_units};
use super::sheet::{Anchor, LineStyle, Polyline2, SheetGeometry, Text2};
use super::views::{SourceModel, ViewGeometry};
use super::{CellAlign, DimFormat, Drawing, Table, TableCorner, TableKind};

/// Resolved table content.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableData {
    pub title: Option<String>,
    pub header: Option<Vec<String>>,
    pub rows: Vec<Vec<String>>,
}

impl TableData {
    pub fn columns(&self) -> usize {
        self.header.iter().chain(self.rows.iter()).map(|r| r.len()).max().unwrap_or(0).max(1)
    }
    /// Every row in display order (header first, no title).
    pub fn all_rows(&self) -> Vec<&Vec<String>> {
        self.header.iter().chain(self.rows.iter()).collect()
    }
}

/// One hole of a hole table.
#[derive(Clone, Debug, PartialEq)]
pub struct HoleRow {
    pub tag: String,
    /// View coordinates of the center.
    pub center: [f64; 2],
    /// Location relative to the origin (model units).
    pub x: f64,
    pub y: f64,
    pub diameter: f64,
}

/// Holes of a view grouped by size: tags A1, A2... for the largest size,
/// B1... for the next; sorted top-to-bottom, left-to-right within a size.
pub fn hole_rows(g: &ViewGeometry, origin: [f64; 2]) -> Vec<HoleRow> {
    let mut bores = g.bores();
    // Concentric bores of different size (e.g. counterbores): keep the
    // smallest per center as the hole size.
    bores.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut holes: Vec<([f64; 2], f64)> = Vec::new();
    for (c, r, _) in bores {
        if !holes.iter().any(|(h, _)| (h[0] - c[0]).abs() < 1e-9 && (h[1] - c[1]).abs() < 1e-9) {
            holes.push((c, r));
        }
    }
    let mut sizes: Vec<f64> = Vec::new();
    for (_, r) in &holes {
        if !sizes.iter().any(|s| (s - r).abs() < 1e-9) {
            sizes.push(*r);
        }
    }
    sizes.sort_by(|a, b| b.total_cmp(a));
    let mut out = Vec::new();
    for (si, size) in sizes.iter().enumerate() {
        let mut group: Vec<&([f64; 2], f64)> = holes.iter().filter(|(_, r)| (r - size).abs() < 1e-9).collect();
        group.sort_by(|a, b| b.0[1].total_cmp(&a.0[1]).then(a.0[0].total_cmp(&b.0[0])));
        let letter = tag_letters(si);
        for (k, (c, r)) in group.into_iter().enumerate() {
            out.push(HoleRow { tag: format!("{letter}{}", k + 1), center: *c, x: c[0] - origin[0], y: c[1] - origin[1], diameter: r * 2.0 });
        }
    }
    out
}

/// A, B, ... Z, AA, AB...
fn tag_letters(i: usize) -> String {
    let mut n = i;
    let mut s = String::new();
    loop {
        s.insert(0, (b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    s
}

/// Resolve a table's content. `view_geo`: geometry of the table's view;
/// `source`: the view's source; `origin`: resolved hole table origin.
pub fn table_data(drawing: &Drawing, table: &Table, view_geo: Option<&ViewGeometry>, source: Option<&SourceModel>, origin: Option<[f64; 2]>, expand: &dyn Fn(&str) -> String) -> TableData {
    match &table.kind {
        TableKind::General { cells, title, header } => {
            let mut rows: Vec<Vec<String>> = cells.iter().map(|r| r.iter().map(|c| expand(c)).collect()).collect();
            let header = if *header && !rows.is_empty() { Some(rows.remove(0)) } else { None };
            TableData { title: title.as_ref().map(|t| expand(t)), header, rows }
        }
        TableKind::Bom { .. } => {
            let header = Some(vec!["ITEM".into(), "QTY".into(), "PART NAME".into(), "PART NUMBER".into(), "MATERIAL".into()]);
            let rows = match source {
                Some(s) if s.assembly => s.bom.iter().map(|r| vec![r.item.to_string(), r.quantity.to_string(), r.name.clone(), r.number.clone(), r.material.clone()]).collect(),
                Some(s) => {
                    // A Part Studio view: one row per part.
                    s.parts.iter().enumerate().map(|(i, p)| vec![(i + 1).to_string(), "1".into(), p.name.clone(), p.number.clone(), p.material.clone()]).collect()
                }
                None => vec![],
            };
            TableData { title: Some("BILL OF MATERIALS".into()), header, rows }
        }
        TableKind::Hole { precision, origin: stored, .. } => {
            let header = Some(vec!["TAG".into(), "X LOC".into(), "Y LOC".into(), "SIZE".into()]);
            let f = DimFormat { precision: *precision, ..Default::default() };
            let u = length_units(drawing, &f);
            let num = |m: f64| format_number(m / u.meters(), f.precision, default_digits(u));
            let rows = view_geo
                .map(|g| hole_rows(g, origin.unwrap_or(*stored)))
                .unwrap_or_default()
                .into_iter()
                .map(|h| vec![h.tag, num(h.x), num(h.y), format!("Ø{}", num(h.diameter))])
                .collect();
            TableData { title: Some("HOLE TABLE".into()), header, rows }
        }
    }
}

/// Table cell grid on the sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct TableLayout {
    /// Column boundaries (left to right), `columns + 1` values.
    pub xs: Vec<f64>,
    /// Row boundaries (top to bottom), `rows + 1` values (title row first).
    pub ys: Vec<f64>,
    pub title: bool,
}

impl TableLayout {
    pub fn min(&self) -> [f64; 2] {
        [self.xs[0], *self.ys.last().unwrap_or(&self.ys[0])]
    }
    pub fn max(&self) -> [f64; 2] {
        [*self.xs.last().unwrap_or(&self.xs[0]), self.ys[0]]
    }
    pub fn contains(&self, p: [f64; 2], margin: f64) -> bool {
        let (lo, hi) = (self.min(), self.max());
        p[0] >= lo[0] - margin && p[0] <= hi[0] + margin && p[1] >= lo[1] - margin && p[1] <= hi[1] + margin
    }
    /// The corner opposite to the fixed one (the resize grip).
    pub fn free_corner(&self, corner: TableCorner) -> [f64; 2] {
        let (lo, hi) = (self.min(), self.max());
        match corner {
            TableCorner::TopLeft => [hi[0], lo[1]],
            TableCorner::TopRight => [lo[0], lo[1]],
            TableCorner::BottomLeft => [hi[0], hi[1]],
            TableCorner::BottomRight => [lo[0], hi[1]],
        }
    }
    /// (row, column) of the cell under `p` (row 0 = first row after the title).
    pub fn cell_at(&self, p: [f64; 2]) -> Option<(usize, usize)> {
        let c = self.xs.windows(2).position(|w| p[0] >= w[0] && p[0] <= w[1])?;
        let r = self.ys.windows(2).position(|w| p[1] <= w[0] && p[1] >= w[1])?;
        if self.title {
            (r > 0).then(|| (r - 1, c))
        } else {
            Some((r, c))
        }
    }
}

/// Text height of a cell.
fn cell_height(table: &Table, r: usize, c: usize) -> f64 {
    table.cell_format(r, c).height.filter(|h| *h > 0.0).unwrap_or(table.text_height)
}

/// Natural column widths (mm) for content. Cells merged across columns do
/// not widen single columns.
pub fn auto_widths(table: &Table, data: &TableData) -> Vec<f64> {
    let n = data.columns();
    let pad = table.text_height * 2.2;
    let mut w = vec![table.text_height * 3.0; n];
    for (ri, row) in data.all_rows().iter().enumerate() {
        for (i, c) in row.iter().enumerate() {
            if table.merge_at(ri, i).is_some_and(|m| m.cols > 1 || (m.row, m.col) != (ri, i)) {
                continue;
            }
            let h = cell_height(table, ri, i);
            let tw = c.lines().map(|l| font::text_width(l, h)).fold(0.0, f64::max) * if table.cell_format(ri, i).bold { 1.05 } else { 1.0 };
            w[i] = w[i].max(tw + pad);
        }
    }
    if let Some(t) = &data.title {
        let total: f64 = w.iter().sum();
        let need = font::text_width(t, table.text_height * 1.2) + pad;
        if need > total {
            let k = need / total;
            w.iter_mut().for_each(|x| *x *= k);
        }
    }
    w
}

/// Height (mm) a row's content needs.
fn row_need(table: &Table, ri: usize, row: &[String]) -> f64 {
    let mut need = table.row_height;
    for (ci, c) in row.iter().enumerate() {
        if table.merge_at(ri, ci).is_some_and(|m| m.rows > 1 || (m.row, m.col) != (ri, ci)) {
            continue;
        }
        let h = cell_height(table, ri, ci);
        let lines = c.lines().count().max(1) as f64;
        need = need.max(table.row_height - table.text_height + h + (lines - 1.0) * h * 1.5).max(h * 2.0);
    }
    need
}

pub fn layout(table: &Table, data: &TableData) -> TableLayout {
    let widths = if table.col_widths.len() == data.columns() { table.col_widths.clone() } else { auto_widths(table, data) };
    let mut heights: Vec<f64> = Vec::new();
    if data.title.is_some() {
        heights.push(table.row_height.max(table.text_height * 2.0));
    }
    for (ri, r) in data.all_rows().iter().enumerate() {
        heights.push(row_need(table, ri, r));
    }
    if heights.is_empty() {
        heights.push(table.row_height);
    }
    let (w, h): (f64, f64) = (widths.iter().sum(), heights.iter().sum());
    let p = table.position;
    let (x0, y1) = match table.corner {
        TableCorner::TopLeft => (p[0], p[1]),
        TableCorner::TopRight => (p[0] - w, p[1]),
        TableCorner::BottomLeft => (p[0], p[1] + h),
        TableCorner::BottomRight => (p[0] - w, p[1] + h),
    };
    let mut xs = vec![x0];
    for wi in &widths {
        xs.push(xs.last().unwrap() + wi);
    }
    let mut ys = vec![y1];
    for hi in &heights {
        ys.push(ys.last().unwrap() - hi);
    }
    TableLayout { xs, ys, title: data.title.is_some() }
}

/// Push runs of consecutive `keep` cells as polylines along a grid line.
fn grid_runs(g: &mut SheetGeometry, n: usize, keep: impl Fn(usize) -> bool, seg: impl Fn(usize) -> ([f64; 2], [f64; 2]), style: LineStyle, pick: u32) {
    let mut i = 0;
    while i < n {
        if !keep(i) {
            i += 1;
            continue;
        }
        let start = seg(i).0;
        let mut j = i;
        while j + 1 < n && keep(j + 1) {
            j += 1;
        }
        g.lines.push(Polyline2 { points: vec![start, seg(j).1], style, closed: false, pick });
        i = j + 1;
    }
}

/// Draw a table (grid and cell texts) with pick id `pick`. Merged ranges
/// show one cell; cell formats set alignment, text height and weight.
pub fn render(g: &mut SheetGeometry, table: &Table, data: &TableData, l: &TableLayout, pick: u32) {
    let (lo, hi) = (l.min(), l.max());
    g.lines.push(Polyline2 { points: vec![[lo[0], lo[1]], [hi[0], lo[1]], [hi[0], hi[1]], [lo[0], hi[1]]], style: LineStyle::Visible, closed: true, pick });
    let th = table.text_height;
    let mut row0 = 0usize;
    if let Some(t) = &data.title {
        let (ya, yb) = (l.ys[0], l.ys[1]);
        g.lines.push(Polyline2 { points: vec![[lo[0], yb], [hi[0], yb]], style: LineStyle::Thin, closed: false, pick });
        g.texts.push(Text2 { pos: [(lo[0] + hi[0]) / 2.0, (ya + yb) / 2.0], height: th * 1.2, text: t.clone(), angle: 0.0, anchor: Anchor::Center, pick });
        row0 = 1;
    }
    let rows = data.all_rows();
    let nrows = rows.len();
    let ncols = l.xs.len().saturating_sub(1);
    let header_rows = data.header.is_some() as usize;
    let merged_across = |r0: usize, c0: usize, r1: usize, c1: usize| table.merges.iter().any(|m| m.contains(r0, c0) && m.contains(r1, c1));
    // Row dividers (interrupted by merged ranges).
    for ri in 0..nrows.saturating_sub(1) {
        let y = l.ys[row0 + ri + 1];
        let style = if ri + 1 == header_rows { LineStyle::Visible } else { LineStyle::Thin };
        grid_runs(g, ncols, |c| !merged_across(ri, c, ri + 1, c), |c| ([l.xs[c], y], [l.xs[c + 1], y]), style, pick);
    }
    // Column dividers below the title.
    for c in 1..ncols {
        let x = l.xs[c];
        let n = nrows.max(1);
        grid_runs(g, n, |r| !merged_across(r, c - 1, r, c), |r| ([x, l.ys[row0 + r]], [x, l.ys[(row0 + r + 1).min(l.ys.len() - 1)]]), LineStyle::Thin, pick);
    }
    for (ri, r) in rows.iter().enumerate() {
        for (ci, c) in r.iter().enumerate() {
            if ci >= ncols {
                break;
            }
            let (rows_n, cols_n) = match table.merge_at(ri, ci) {
                Some(m) if (m.row, m.col) == (ri, ci) => (m.rows, m.cols),
                Some(_) => continue,
                None => (1, 1),
            };
            let (xa, xb) = (l.xs[ci], l.xs[(ci + cols_n).min(ncols)]);
            let (ya, yb) = (l.ys[row0 + ri], l.ys[(row0 + ri + rows_n).min(l.ys.len() - 1)]);
            let fmt = table.cell_format(ri, ci);
            let h = cell_height(table, ri, ci);
            let lines: Vec<&str> = c.lines().collect();
            let n = lines.len().max(1) as f64;
            let line_h = h * 1.5;
            let pad = th * 0.8;
            for (k, text) in lines.iter().enumerate() {
                let y = (ya + yb) / 2.0 + (n - 1.0) * line_h / 2.0 - k as f64 * line_h;
                let tw = font::text_width(text, h);
                let (pos, anchor) = match fmt.align {
                    CellAlign::Center => ([(xa + xb) / 2.0, y], Anchor::Center),
                    CellAlign::Left => ([xa + pad, y - h / 2.0], Anchor::BottomLeft),
                    CellAlign::Right => ([xb - pad - tw, y - h / 2.0], Anchor::BottomLeft),
                };
                g.texts.push(Text2 { pos, height: h, text: text.to_string(), angle: 0.0, anchor, pick });
                if fmt.bold {
                    // Bold: the stroke text drawn again, slightly offset.
                    for dx in [0.045, 0.09] {
                        g.texts.push(Text2 { pos: [pos[0] + h * dx, pos[1]], height: h, text: text.to_string(), angle: 0.0, anchor, pick });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_table_layout() {
        let t = Table::new(1, TableKind::General { cells: vec![vec!["A".into(), "Long header".into()], vec!["1".into(), "x\ny".into()]], title: Some("T".into()), header: true }, [400.0, 280.0]);
        let d = table_data(&Drawing::default(), &t, None, None, None, &|s| s.to_string());
        assert_eq!(d.header.as_ref().unwrap().len(), 2);
        assert_eq!(d.rows.len(), 1);
        let l = layout(&t, &d);
        // Fixed top-right corner.
        assert!((l.max()[0] - 400.0).abs() < 1e-9 && (l.max()[1] - 280.0).abs() < 1e-9);
        assert_eq!(l.ys.len(), 4);
        // The two-line cell makes its row taller.
        assert!(l.ys[2] - l.ys[3] > l.ys[1] - l.ys[2]);
        assert_eq!(l.cell_at([l.xs[0] + 0.5, l.ys[3] + 0.5]), Some((1, 0)));
        let mut g = SheetGeometry::new(420.0, 297.0);
        render(&mut g, &t, &d, &l, 7);
        assert!(g.texts.iter().any(|x| x.text == "Long header"));
        assert!(g.lines.iter().all(|x| x.pick == 7));
    }

    #[test]
    fn tags() {
        assert_eq!(tag_letters(0), "A");
        assert_eq!(tag_letters(25), "Z");
        assert_eq!(tag_letters(26), "AA");
    }
}
