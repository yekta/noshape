//! Paste as table from a spreadsheet's HTML (Google Sheets, Excel,
//! LibreOffice copy a `<table>` alongside the text). Live (drawing.md TB12):
//! pasted tables keep merged cells, text size, bold / italic / underline /
//! strikethrough, and cell width and height; background, text color and
//! links are dropped; partly-formatted cells lose their formatting, and all
//! cells are center-aligned.

use noshape_model::drawing::{CellFormat, CellMerge, CellStyle};

/// A table read from clipboard HTML: its cells (a merged range's text in its
/// top-left cell), merges, cell formats and sizes (mm; empty = automatic).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PastedTable {
    pub cells: Vec<Vec<String>>,
    pub merges: Vec<CellMerge>,
    pub formats: Vec<CellStyle>,
    pub col_widths: Vec<f64>,
    pub row_heights: Vec<f64>,
}

impl PastedTable {
    /// Plain cells (tab-separated text): no formatting, all centred.
    pub fn plain(cells: Vec<Vec<String>>) -> Self {
        Self { cells, ..Default::default() }
    }
}

/// The text formatting a run of a cell is drawn with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    /// Font size (mm).
    size: Option<f64>,
}

/// Live (request 1727): a pasted sheet's px and pt are taken as mm, 1:1
/// (140 px → a 140 mm column, 10 pt → 10 mm text).
const PT: f64 = 1.0;
const PX: f64 = 1.0;

/// A CSS / attribute length in mm (px without a unit).
fn length(v: &str) -> Option<f64> {
    let v = v.trim().to_ascii_lowercase();
    let (num, unit) = match v.find(|c: char| !(c.is_ascii_digit() || c == '.')) {
        Some(i) => (&v[..i], v[i..].trim()),
        None => (v.as_str(), ""),
    };
    let n: f64 = num.parse().ok()?;
    let mm = match unit {
        "" | "px" => n * PX,
        "pt" => n * PT,
        "mm" => n,
        "cm" => n * 10.0,
        "in" => n * 25.4,
        "em" | "rem" => n * 16.0 * PX,
        // Physical units as given (no live data).
        _ => return None,
    };
    (mm > 0.0).then_some(mm)
}

/// `name="value"`, `name='value'` and `name=value` attributes of a tag.
fn attrs(tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = tag.as_bytes();
    // Skip the tag name.
    let mut i = tag.find(|c: char| c.is_whitespace()).unwrap_or(tag.len());
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'/' {
            i += 1;
        }
        let name = tag[start..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                let s = i + 1;
                i = s;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                value = tag[s..i.min(b.len())].to_string();
                i += 1;
            } else {
                let s = i;
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                value = tag[s..i].to_string();
            }
        }
        if !name.is_empty() {
            out.push((name, decode(&value)));
        }
    }
    out
}

fn attr<'a>(a: &'a [(String, String)], name: &str) -> Option<&'a str> {
    a.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
}

/// The `style` declarations of a tag ("font-weight" → "bold").
fn css(a: &[(String, String)]) -> Vec<(String, String)> {
    attr(a, "style")
        .unwrap_or("")
        .split(';')
        .filter_map(|d| {
            let (k, v) = d.split_once(':')?;
            Some((k.trim().to_ascii_lowercase(), v.trim().to_ascii_lowercase()))
        })
        .collect()
}

/// `style` applied over `s` (font weight / style / decoration / size).
fn apply_css(mut s: Style, decls: &[(String, String)]) -> Style {
    for (k, v) in decls {
        match k.as_str() {
            "font-weight" => s.bold = v == "bold" || v == "bolder" || v.parse::<u32>().is_ok_and(|w| w >= 600),
            "font-style" => s.italic = v.contains("italic") || v.contains("oblique"),
            "text-decoration" | "text-decoration-line" => {
                if v.contains("none") {
                    (s.underline, s.strike) = (false, false);
                }
                s.underline |= v.contains("underline");
                s.strike |= v.contains("line-through");
            }
            "font-size" => s.size = length(v).or(s.size),
            _ => {}
        }
    }
    s
}

/// HTML entities in text (`&amp;`, `&nbsp;`, `&#39;`, `&#x2014;` …).
fn decode(t: &str) -> String {
    if !t.contains('&') {
        return t.to_string();
    }
    let mut out = String::with_capacity(t.len());
    let mut rest = t;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let ch = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ if name.starts_with("#x") || name.starts_with("#X") => u32::from_str_radix(&name[2..], 16).ok().and_then(char::from_u32),
            _ if name.starts_with('#') => name[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A cell being read: its text runs with their styles.
struct Cell {
    row: usize,
    col: usize,
    text: String,
    runs: Vec<Style>,
}

/// Paste as table from clipboard HTML: the first `<table>`, else None.
pub fn parse_html_table(html: &str) -> Option<PastedTable> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<table")?;
    let end = lower[start..].find("</table").map_or(html.len(), |e| start + e);
    let src = &html[start..end];

    let mut t = PastedTable::default();
    // Cells covered by a rowspan from a row above: (row, col).
    let mut taken: std::collections::HashSet<(usize, usize)> = Default::default();
    let mut row: Option<usize> = None;
    let mut col = 0;
    let mut cell: Option<Cell> = None;
    // Inline style stack inside a cell: (tag, style before it).
    let mut stack: Vec<(String, Style)> = Vec::new();
    let mut style = Style::default();
    // The <table>'s own style is every cell's start (live 1727: Sheets'
    // table font-size:10pt gives 10 mm text in every cell).
    let mut base = Style::default();
    let mut col_defs: Vec<Option<f64>> = Vec::new();
    let mut skip = false;

    let finish = |t: &mut PastedTable, c: Cell| {
        let text = c.text.split('\n').map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n").trim().to_string();
        // One style for every run, else none (partly-formatted cells
        // lose their formatting).
        let uniform = c.runs.first().filter(|f| c.runs.iter().all(|r| r == *f)).copied();
        let s = uniform.unwrap_or_default();
        let format = CellFormat { bold: s.bold, italic: s.italic, underline: s.underline, strikethrough: s.strike, height: s.size, ..Default::default() };
        while t.cells.len() <= c.row {
            t.cells.push(Vec::new());
        }
        let r = &mut t.cells[c.row];
        if r.len() <= c.col {
            r.resize(c.col + 1, String::new());
        }
        r[c.col] = text;
        t.formats.push(CellStyle { row: c.row, col: c.col, format });
    };

    let mut rest = src;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            if let Some(c) = &mut cell
                && !skip
            {
                push_text(c, rest, style);
            }
            break;
        };
        if lt > 0 {
            if let Some(c) = &mut cell
                && !skip
            {
                push_text(c, &rest[..lt], style);
            }
            rest = &rest[lt..];
        }
        // Comments (Excel's conditional comments).
        if rest.starts_with("<!--") {
            rest = rest.find("-->").map_or("", |e| &rest[e + 3..]);
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        let tag = &rest[1..gt];
        rest = &rest[gt + 1..];
        let closing = tag.starts_with('/');
        let name: String = tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
        match (name.as_str(), closing) {
            ("style" | "script", false) => skip = true,
            ("table", false) => base = apply_css(Style::default(), &css(&attrs(tag))),
            ("style" | "script", true) => skip = false,
            ("col", false) => {
                let a = attrs(tag);
                let w = css(&a).iter().find(|(k, _)| k == "width").and_then(|(_, v)| length(v)).or_else(|| attr(&a, "width").and_then(length));
                let span = attr(&a, "span").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1).clamp(1, 1000);
                col_defs.extend(std::iter::repeat_n(w, span));
            }
            ("tr", false) => {
                if let Some(c) = cell.take() {
                    finish(&mut t, c);
                }
                let r = row.map_or(0, |r| r + 1);
                row = Some(r);
                col = 0;
                let a = attrs(tag);
                let h = css(&a).iter().find(|(k, _)| k == "height").and_then(|(_, v)| length(v)).or_else(|| attr(&a, "height").and_then(length));
                if t.row_heights.len() <= r {
                    t.row_heights.resize(r + 1, 0.0);
                }
                t.row_heights[r] = h.unwrap_or(0.0);
            }
            ("td" | "th", false) => {
                if let Some(c) = cell.take() {
                    finish(&mut t, c);
                }
                let r = match row {
                    Some(r) => r,
                    None => {
                        row = Some(0);
                        t.row_heights.push(0.0);
                        0
                    }
                };
                while taken.contains(&(r, col)) {
                    col += 1;
                }
                let a = attrs(tag);
                let decls = css(&a);
                let span = |n: &str| attr(&a, n).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1).clamp(1, 1000);
                let (rows, cols) = (span("rowspan"), span("colspan"));
                if rows > 1 || cols > 1 {
                    t.merges.push(CellMerge { row: r, col, rows, cols });
                }
                for dr in 0..rows {
                    for dc in 0..cols {
                        if dr > 0 || dc > 0 {
                            taken.insert((r + dr, col + dc));
                        }
                    }
                }
                // A cell's own width / height (a single-column cell sets its
                // column; Excel and LibreOffice put sizes on cells).
                if cols == 1
                    && let Some(w) = decls.iter().find(|(k, _)| k == "width").and_then(|(_, v)| length(v)).or_else(|| attr(&a, "width").and_then(length))
                {
                    if col_defs.len() <= col {
                        col_defs.resize(col + 1, None);
                    }
                    col_defs[col].get_or_insert(w);
                }
                if rows == 1
                    && let Some(h) = decls.iter().find(|(k, _)| k == "height").and_then(|(_, v)| length(v)).or_else(|| attr(&a, "height").and_then(length))
                    && t.row_heights[r] == 0.0
                {
                    t.row_heights[r] = h;
                }
                style = apply_css(Style { bold: name == "th" || base.bold, ..base }, &decls);
                stack.clear();
                cell = Some(Cell { row: r, col, text: String::new(), runs: Vec::new() });
                col += cols;
            }
            ("td" | "th", true) => {
                if let Some(c) = cell.take() {
                    finish(&mut t, c);
                }
            }
            ("br", _) => {
                if let Some(c) = &mut cell {
                    c.text.push('\n');
                }
            }
            (n @ ("p" | "div"), true) => {
                if let Some(c) = &mut cell
                    && !c.text.trim_end_matches(' ').ends_with('\n')
                    && !c.text.trim().is_empty()
                {
                    c.text.push('\n');
                }
                if let Some(i) = stack.iter().rposition(|(t, _)| t == n) {
                    style = stack[i].1;
                    stack.truncate(i);
                }
            }
            (n @ ("b" | "strong" | "i" | "em" | "u" | "ins" | "s" | "strike" | "del" | "span" | "font" | "a" | "p" | "div"), false) if cell.is_some() && !tag.ends_with('/') => {
                stack.push((n.to_string(), style));
                let a = attrs(tag);
                match n {
                    "b" | "strong" => style.bold = true,
                    "i" | "em" => style.italic = true,
                    "u" | "ins" => style.underline = true,
                    "s" | "strike" | "del" => style.strike = true,
                    // <font size=1..7> (LibreOffice).
                    "font" => {
                        if let Some(sz) = attr(&a, "size").and_then(|s| s.trim().parse::<usize>().ok()) {
                            style.size = [8.0, 10.0, 12.0, 14.0, 18.0, 24.0, 36.0].get(sz.clamp(1, 7) - 1).map(|pt| pt * PT);
                        }
                    }
                    _ => {}
                }
                style = apply_css(style, &css(&a));
            }
            (n @ ("b" | "strong" | "i" | "em" | "u" | "ins" | "s" | "strike" | "del" | "span" | "font" | "a"), true) => {
                if let Some(i) = stack.iter().rposition(|(t, _)| t == n) {
                    style = stack[i].1;
                    stack.truncate(i);
                }
            }
            _ => {}
        }
    }
    if let Some(c) = cell.take() {
        finish(&mut t, c);
    }
    let ncols = t.cells.iter().map(|r| r.len()).max().unwrap_or(0).max(t.merges.iter().map(|m| m.col + m.cols).max().unwrap_or(0));
    let nrows = t.cells.len().max(t.merges.iter().map(|m| m.row + m.rows).max().unwrap_or(0));
    if ncols == 0 || nrows == 0 || (ncols == 1 && nrows == 1 && t.merges.is_empty()) {
        return None;
    }
    t.cells.resize(nrows, Vec::new());
    for r in &mut t.cells {
        r.resize(ncols, String::new());
    }
    // Sizes: all known, else automatic.
    t.col_widths = if col_defs.len() >= ncols && col_defs[..ncols].iter().all(Option::is_some) { col_defs[..ncols].iter().flatten().copied().collect() } else { Vec::new() };
    t.row_heights.resize(nrows, 0.0);
    if t.row_heights.iter().any(|h| *h <= 0.0) {
        t.row_heights.clear();
    }
    t.formats.sort_by_key(|s| (s.row, s.col));
    Some(t)
}

/// Text inside a cell: a styled run when it has any visible characters.
fn push_text(c: &mut Cell, raw: &str, style: Style) {
    let text = decode(raw);
    if !text.trim().is_empty() {
        c.runs.push(style);
    }
    // Source line breaks are whitespace in HTML.
    c.text.push_str(&text.replace(['\n', '\r', '\t'], " "));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Google Sheets' clipboard HTML: sizes from <col> / <tr>, styles inline.
    #[test]
    fn sheets_html_keeps_merges_styles_and_sizes() {
        let html = r#"<meta charset='utf-8'><google-sheets-html-origin><style type="text/css"><!--td {border: 1px solid #cccccc;}--></style><table xmlns="http://www.w3.org/1999/xhtml" cellspacing="0" cellpadding="0" dir="ltr" border="1" style="table-layout:fixed;font-size:10pt;font-family:Arial;width:0px;border-collapse:collapse;border:none"><colgroup><col width="100"/><col width="120"/><col width="80"/></colgroup><tbody><tr style="height:21px;"><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;font-weight:bold;" rowspan="1" colspan="2">Title &amp; more</td><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;font-size:14pt;">Big</td></tr><tr style="height:21px;"><td style="font-style:italic;text-decoration:underline;">a</td><td style="text-decoration:line-through;background-color:#ff0000;color:#00ff00;"><a href="http://x">link</a></td><td>plain <b>bold</b></td></tr></tbody></table>"#;
        let t = parse_html_table(html).unwrap();
        assert_eq!(t.cells, vec![vec!["Title & more", "", "Big"], vec!["a", "link", "plain bold"]]);
        assert_eq!(t.merges, vec![CellMerge { row: 0, col: 0, rows: 1, cols: 2 }]);
        let f = |r, c| t.formats.iter().find(|s| s.row == r && s.col == c).unwrap().format.clone();
        assert!(f(0, 0).bold && !f(0, 0).italic);
        // The table's font-size:10pt reaches every cell (live 1727).
        assert_eq!(f(0, 0).height, Some(10.0 * PT));
        assert!((f(0, 2).height.unwrap() - 14.0 * PT).abs() < 1e-9);
        assert!(f(1, 0).italic && f(1, 0).underline && !f(1, 0).bold);
        assert!(f(1, 1).strikethrough, "links and colours dropped, strikethrough kept");
        // Partly bold: no formatting.
        assert_eq!(f(1, 2), CellFormat::default());
        assert_eq!(f(1, 0).height, Some(10.0 * PT));
        // Every cell centred.
        assert!(t.formats.iter().all(|s| s.format.align == noshape_model::drawing::CellAlign::Center));
        assert_eq!(t.col_widths.iter().map(|w| (w / PX).round()).collect::<Vec<_>>(), vec![100.0, 120.0, 80.0]);
        assert_eq!(t.row_heights.iter().map(|h| (h / PX).round()).collect::<Vec<_>>(), vec![21.0, 21.0]);
    }

    /// Excel / LibreOffice: rowspans push later cells right; <font size>,
    /// sizes in pt on cells.
    #[test]
    fn rowspans_take_their_cells_in_later_rows() {
        let html = "<html><body><table><tr height=20 style='height:15.0pt'><td rowspan=2 width=64 style='height:30.0pt;width:48pt'>A</td><td width=64 style='width:48pt'>B</td></tr>\n<tr height=20 style='height:15.0pt'><td style='height:15.0pt'><font size=4>C</font></td></tr></table></body></html>";
        let t = parse_html_table(html).unwrap();
        assert_eq!(t.cells, vec![vec!["A", "B"], vec!["", "C"]]);
        assert_eq!(t.merges, vec![CellMerge { row: 0, col: 0, rows: 2, cols: 1 }]);
        assert_eq!(t.formats.iter().find(|s| s.row == 1 && s.col == 1).unwrap().format.height, Some(14.0 * PT));
        assert_eq!(t.col_widths, vec![48.0 * PT, 48.0 * PT]);
        assert_eq!(t.row_heights, vec![15.0 * PT, 15.0 * PT]);
    }

    #[test]
    fn not_a_table() {
        assert_eq!(parse_html_table("<p>hello</p>"), None);
        assert_eq!(parse_html_table("<table><tr><td>one</td></tr></table>"), None);
    }
}
