//! Drawing tables: general tables, bills of materials (from an assembly
//! view), hole tables (from a view) and revision tables (from the release
//! log). Content is resolved when the sheet
//! is built; layout and rendering produce plain sheet geometry so tables
//! appear in every export.

use super::font;
use super::format::{default_digits, format_number, length_units};
use super::sheet::{Anchor, GeomMark, LineStyle, Polyline2, SheetGeometry, Text2};
use super::cad_objects::{CadMember, CadObject, table_row_styles};
use super::views::{SourceKey, SourceModel, Sources, ViewGeometry};
use crate::custom::table::{CustomTable, TableResult, evaluate_table};
use crate::custom::{CustomFeatureProvider, DocumentScripts};
use crate::document::{Document, TabContent, TabId};
use crate::regen::context::Context;
use std::collections::HashMap;
use std::sync::Arc;
use super::{CellAlign, CellVAlign, DimFormat, Drawing, Table, TableCorner, TableKind};

/// Resolved table content.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableData {
    pub title: Option<String>,
    pub header: Option<Vec<String>>,
    pub rows: Vec<Vec<String>>,
    /// Pending rows (indices into `rows`), highlighted blue.
    pub pending: Vec<usize>,
    /// Cell padding (mm, each side): horizontal, vertical (Drawing
    /// properties › Tables).
    pub padding: [f64; 2],
    /// The header row is the last row (BOM Order "Bottom to top").
    pub header_bottom: bool,
    /// Overridden cells (row in `rows`, column): a black triangle at the
    /// cell's bottom left.
    pub overridden: Vec<(usize, usize)>,
    /// Alignment of the body cells per column, where the table's own cell
    /// formats don't say (custom tables: their columns' alignment).
    pub col_align: Vec<CellAlign>,
    /// The header cells are bold (custom tables).
    pub header_bold: bool,
    /// Drawing properties › Tables for the table (None: tables drawn on
    /// their own, as in unit tests).
    pub style: Option<AppliedStyle>,
}

/// A table's Drawing properties › Tables look, resolved: row text heights
/// (mm), the header and content rows' Bold / Italic / colour, the font,
/// the line pen (mm).
#[derive(Clone, Debug, PartialEq)]
pub struct AppliedStyle {
    pub title: f64,
    pub header: f64,
    pub content: f64,
    pub title_text: super::RowText,
    pub header_text: super::RowText,
    pub content_text: super::RowText,
    pub font: Option<super::sheet::TextFont>,
    pub line: f64,
}

impl TableData {
    pub fn columns(&self) -> usize {
        self.header.iter().chain(self.rows.iter()).map(|r| r.len()).max().unwrap_or(0).max(1)
    }
    /// Whether display row `row` (of `rows`) is the header.
    pub fn is_header(&self, row: usize, rows: usize) -> bool {
        self.header.is_some() && if self.header_bottom { row + 1 == rows } else { row == 0 }
    }
    /// Format of a cell the table's cell formats don't cover: Drawing
    /// properties › Tables' row Bold / Italic and font.
    pub fn default_format(&self, row: usize, col: usize, rows: usize) -> super::CellFormat {
        let header = self.is_header(row, rows);
        let font = self.style.as_ref().and_then(|s| s.font.as_ref()).map(|f| f.family.clone());
        let (bold, italic) = match (&self.style, header) {
            (Some(s), true) => (s.header_text.bold, s.header_text.italic),
            (Some(s), false) => (s.content_text.bold, s.content_text.italic),
            (None, true) => (self.header_bold, false),
            (None, false) => (false, false),
        };
        let align = if header { Default::default() } else { self.col_align.get(col).copied().unwrap_or_default() };
        super::CellFormat { align, bold, italic, font, ..Default::default() }
    }
    /// Display row `row`'s text height (mm).
    pub fn row_text_height(&self, table: &Table, row: usize) -> f64 {
        match &self.style {
            Some(s) if self.is_header(row, self.all_rows().len()) => s.header,
            Some(s) => s.content,
            None => table.text_height,
        }
    }
    /// The content rows' and the title row's text heights (mm).
    pub fn content_height(&self, table: &Table) -> f64 {
        self.style.as_ref().map_or(table.text_height, |s| s.content)
    }
    pub fn title_height(&self, table: &Table) -> f64 {
        self.style.as_ref().map_or(table.text_height * 1.2, |s| s.title)
    }
    /// Every row in display order (header first, no title).
    pub fn all_rows(&self) -> Vec<&Vec<String>> {
        if self.header_bottom {
            self.rows.iter().chain(self.header.iter()).collect()
        } else {
            self.header.iter().chain(self.rows.iter()).collect()
        }
    }
}

/// One revision of a drawing (a revision table row).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RevisionRow {
    pub revision: String,
    pub description: String,
    /// YYYY-MM-DD (UTC).
    pub date: String,
    pub approved: String,
    /// In a release candidate, not yet released ("B*", highlighted).
    pub pending: bool,
    /// Obsoleted.
    pub obsolete: bool,
    /// The released item's part number and name (template columns).
    pub part_number: String,
    pub name: String,
}

/// The revisions of drawing tab `tab` in the release log, oldest first:
/// one row per release, then a pending row per saved candidate (draft)
/// that contains the drawing. Descriptions are the package notes.
pub fn revision_rows(log: &crate::release::ReleaseLog, tab: crate::document::TabId) -> Vec<RevisionRow> {
    use crate::release::{ItemKind, PackageKind};
    let date = |ms: i64| {
        let (y, m, d) = super::format::civil_from_days(ms.div_euclid(86_400_000));
        format!("{y:04}-{m:02}-{d:02}")
    };
    let mut out: Vec<RevisionRow> = Vec::new();
    for p in &log.packages {
        for it in p.items.iter().filter(|it| it.kind == ItemKind::Drawing && it.tab == tab) {
            match p.kind {
                PackageKind::Release => out.push(RevisionRow { revision: it.revision.clone(), description: p.notes.clone(), date: date(p.created), approved: String::new(), pending: false, obsolete: false, part_number: it.part_number.clone(), name: it.name.clone() }),
                PackageKind::Obsoletion => {
                    if let Some(r) = out.iter_mut().rev().find(|r| r.revision == it.revision) {
                        r.obsolete = true;
                    }
                }
            }
        }
    }
    for p in log.drafts.iter().filter(|p| p.kind == PackageKind::Release) {
        for it in p.items.iter().filter(|it| it.kind == ItemKind::Drawing && it.tab == tab) {
            out.push(RevisionRow { revision: it.revision.clone(), description: p.notes.clone(), date: String::new(), approved: String::new(), pending: true, obsolete: false, part_number: it.part_number.clone(), name: it.name.clone() });
        }
    }
    out
}

/// Highlight of pending revision rows.
pub const PENDING_BLUE: [u8; 3] = [0xd2, 0xe8, 0xfc];

/// Revision table column headers (defaults).
pub const REVISION_HEADERS: [&str; 4] = ["REV", "DESCRIPTION", "DATE", "APPROVED"];

/// Company settings › Release management › Revision table template (live
/// bundle `osx-properties-table-template-*`, type REVISION_TABLE): the
/// columns drawings' revision tables show, by property name, and "All
/// uppercase in Drawings".
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RevisionTemplate {
    pub name: String,
    pub columns: Vec<String>,
    #[serde(default)]
    pub all_caps: bool,
}

/// The Onshape default template's columns (help image relmgmt-workflows-14).
pub const DEFAULT_REVISION_COLUMNS: [&str; 4] = ["Revision", "Revision description", "Date approved", "Approver"];

impl Default for RevisionTemplate {
    fn default() -> Self {
        Self { name: "Onshape Default revision table".into(), columns: DEFAULT_REVISION_COLUMNS.map(String::from).to_vec(), all_caps: false }
    }
}

impl RevisionTemplate {
    /// Unchanged from Onshape's default (its table keeps the REV /
    /// DESCRIPTION / DATE / APPROVED headers).
    pub fn is_default(&self) -> bool {
        self.columns == DEFAULT_REVISION_COLUMNS && !self.all_caps
    }

    /// A row's value in column `column`: the revision ("B*" while pending),
    /// its description, date, approver, the item's part number or name;
    /// other properties are empty.
    fn cell(column: &str, row: &RevisionRow, desc: &str) -> String {
        match column {
            "Revision" if row.pending => format!("{}*", row.revision),
            "Revision" => row.revision.clone(),
            "Revision description" => desc.to_string(),
            "Date approved" => row.date.clone(),
            "Approver" => row.approved.clone(),
            "Part number" => row.part_number.clone(),
            "Name" => row.name.clone(),
            _ => String::new(),
        }
    }
}

static REVISION_TEMPLATE: std::sync::RwLock<Option<RevisionTemplate>> = std::sync::RwLock::new(None);

/// The company's revision table template (the app sets it from its release
/// settings); None: Onshape's default.
pub fn set_revision_template(t: Option<RevisionTemplate>) {
    *REVISION_TEMPLATE.write().unwrap_or_else(|e| e.into_inner()) = t.filter(|t| !t.is_default());
}

/// Content of a revision table: the last `visible_rows` revisions in the
/// table's order, pending revisions marked "*".
fn revision_data(kind: &TableKind, revisions: &[RevisionRow]) -> TableData {
    let template = REVISION_TEMPLATE.read().unwrap_or_else(|e| e.into_inner()).clone();
    revision_data_with(kind, revisions, template.as_ref())
}

/// [`revision_data`] with the company's template (None: Onshape's default).
fn revision_data_with(kind: &TableKind, revisions: &[RevisionRow], template: Option<&RevisionTemplate>) -> TableData {
    let TableKind::Revision { visible_rows, newest_top, title, headers, descriptions, .. } = kind else { return TableData::default() };
    let n = (*visible_rows as usize).max(1);
    let mut shown: Vec<&RevisionRow> = revisions.iter().skip(revisions.len().saturating_sub(n)).collect();
    if *newest_top {
        shown.reverse();
    }
    let desc_of = |r: &RevisionRow| descriptions.iter().find(|(rev, _)| *rev == r.revision).map(|(_, d)| d.clone()).unwrap_or_else(|| r.description.clone());
    let (header, rows): (Vec<String>, Vec<Vec<String>>) = match template {
        // A company template: its columns, in capitals with "All uppercase in Drawings".
        Some(t) => {
            let caps = |s: String| if t.all_caps { s.to_uppercase() } else { s };
            let header = t.columns.iter().enumerate().map(|(i, c)| headers.get(i).cloned().flatten().unwrap_or_else(|| caps(c.clone()))).collect();
            let rows = shown.iter().map(|r| t.columns.iter().map(|c| caps(RevisionTemplate::cell(c, r, &desc_of(r)))).collect()).collect();
            (header, rows)
        }
        None => {
            let header = REVISION_HEADERS.iter().enumerate().map(|(i, d)| headers.get(i).cloned().flatten().unwrap_or_else(|| d.to_string())).collect();
            let rows = shown
                .iter()
                .map(|r| {
                    let rev = if r.pending { format!("{}*", r.revision) } else { r.revision.clone() };
                    vec![rev, desc_of(r), r.date.clone(), r.approved.clone()]
                })
                .collect();
            (header, rows)
        }
    };
    let pending = shown.iter().enumerate().filter(|(_, r)| r.pending).map(|(i, _)| i).collect();
    TableData { title: Some(title.clone().unwrap_or_else(|| "REVISION HISTORY".into())), header: Some(header), rows, pending, padding: [0.0; 2], header_bottom: false, overridden: Vec::new(), ..Default::default() }
}

/// Label of a revision callout: the revision, "*" while pending; None
/// when the drawing has no revision table, the row's callouts are hidden,
/// or "Hide revision callouts no longer shown in the table" applies.
pub fn revision_callout_label(drawing: &Drawing, revisions: &[RevisionRow], rev: &str) -> Option<String> {
    let t = drawing.sheets.iter().flat_map(|s| &s.tables).find(|t| matches!(t.kind, TableKind::Revision { .. }))?;
    let TableKind::Revision { visible_rows, hide_callouts, hidden, .. } = &t.kind else { return None };
    if hidden.iter().any(|h| h == rev) {
        return None;
    }
    let shown = &revisions[revisions.len().saturating_sub((*visible_rows as usize).max(1))..];
    if *hide_callouts && !shown.iter().any(|r| r.revision == rev) {
        return None;
    }
    let pending = revisions.iter().rev().find(|r| r.revision == rev).is_some_and(|r| r.pending);
    Some(if pending { format!("{rev}*") } else { rev.to_string() })
}

/// The pointer table "See sheet 1" shows on the other sheets.
pub fn see_sheet_data(drawing: &Drawing, home: usize) -> TableData {
    TableData { title: Some("REVISION HISTORY".into()), header: None, rows: vec![vec![format!("SEE SHEET {}", home + 1)]], pending: Vec::new(), padding: padding(drawing), header_bottom: false, overridden: Vec::new(), ..Default::default() }
}

fn padding(drawing: &Drawing) -> [f64; 2] {
    [drawing.props.table_h_padding, drawing.props.table_v_padding]
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
/// `source`: the view's source; `origin`: resolved hole table origin;
/// `revisions`: the drawing's revisions (revision tables).
/// The BOM table's header row.
pub const BOM_HEADER: [&str; 4] = ["Item", "Quantity", "Part number", "Description"];

pub fn table_data(drawing: &Drawing, table: &Table, view_geo: Option<&ViewGeometry>, source: Option<&SourceModel>, origin: Option<[f64; 2]>, revisions: &[RevisionRow], expand: &dyn Fn(&str) -> String) -> TableData {
    let data = match &table.kind {
        TableKind::General { cells, title, header } => {
            let mut rows: Vec<Vec<String>> = cells.iter().map(|r| r.iter().map(|c| expand(c)).collect()).collect();
            let header = if *header && !rows.is_empty() { Some(rows.remove(0)) } else { None };
            // Live 744–750: header cells bold and centred, body cells left-aligned.
            let col_align = vec![CellAlign::Left; cells.iter().map(|r| r.len()).max().unwrap_or(0)];
            TableData { title: title.as_ref().map(|t| expand(t)), header, rows, pending: Vec::new(), padding: [0.0; 2], header_bottom: false, overridden: Vec::new(), col_align, header_bold: true, style: None }
        }
        TableKind::Bom { structured, bottom_to_top, overrides, .. } => {
            // Onshape's BOM table columns (live drawing/189): Item, Quantity,
            // Part number, Description; no title row.
            let header = Some(BOM_HEADER.map(String::from).to_vec());
            let description = |s: &SourceModel, body: Option<&crate::reference::BodyId>| body.and_then(|b| s.part(Some(b))).map(|p| p.description.clone()).unwrap_or_default();
            let mut rows: Vec<Vec<String>> = match source {
                // Structured: nested rows ("3.1").
                Some(s) if s.assembly && *structured => s
                    .bom_structured
                    .iter()
                    .map(|r| {
                        let body = s.bom.iter().find(|e| e.name == r.name && e.number == r.number).and_then(|e| e.bodies.first());
                        vec![r.item.clone(), r.quantity.to_string(), r.number.clone(), description(s, body)]
                    })
                    .collect(),
                Some(s) if s.assembly => s.bom.iter().map(|r| vec![r.item.to_string(), r.quantity.to_string(), r.number.clone(), description(s, r.bodies.first())]).collect(),
                Some(s) => {
                    // A Part Studio view: one row per part.
                    s.parts.iter().enumerate().map(|(i, p)| vec![(i + 1).to_string(), "1".into(), p.number.clone(), p.description.clone()]).collect()
                }
                None => vec![],
            };
            if *bottom_to_top {
                rows.reverse();
            }
            // Overrides replace the model's value in their cells.
            let mut overridden = Vec::new();
            for (ri, row) in rows.iter_mut().enumerate() {
                for (item, col, value) in overrides {
                    if row.first() == Some(item)
                        && let Some(c) = row.get_mut(*col)
                    {
                        *c = value.clone();
                        overridden.push((ri, *col));
                    }
                }
            }
            TableData { title: None, header, rows, pending: Vec::new(), padding: [0.0; 2], header_bottom: *bottom_to_top, overridden, ..Default::default() }
        }
        TableKind::Hole { precision, origin: stored, flip_x, flip_y, .. } => {
            let (sx, sy) = (if *flip_x { -1.0 } else { 1.0 }, if *flip_y { -1.0 } else { 1.0 });
            let header = Some(vec!["TAG".into(), "X LOC".into(), "Y LOC".into(), "SIZE".into()]);
            let f = DimFormat { precision: *precision, ..Default::default() };
            let u = length_units(drawing, &f);
            let num = |m: f64| format_number(m / u.meters(), f.precision, default_digits(u));
            let rows = view_geo
                .map(|g| hole_rows(g, origin.unwrap_or(*stored)))
                .unwrap_or_default()
                .into_iter()
                .map(|h| vec![h.tag, num(h.x * sx), num(h.y * sy), format!("Ø{}", num(h.diameter))])
                .collect();
            TableData { title: Some("HOLE TABLE".into()), header, rows, pending: Vec::new(), padding: [0.0; 2], header_bottom: false, overridden: Vec::new(), ..Default::default() }
        }
        TableKind::Revision { .. } => revision_data(&table.kind, revisions),
        // Resolved by [`table_data_with`] from the evaluated tables.
        TableKind::Custom { .. } => TableData::default(),
        TableKind::CutList { bottom_to_top, .. } => {
            // Onshape's cut list table (help cutlist-table-indrawing): Item,
            // Qty, Description, Length ("675.444 mm"), Angle 1 / 2 ("45 deg").
            let header = Some(["Item", "Qty", "Description", "Length", "Angle 1", "Angle 2"].map(String::from).to_vec());
            let f = DimFormat::default();
            let u = length_units(drawing, &f);
            let len = |m: Option<f64>| m.map(|m| format!("{} {}", format_number(m / u.meters(), None, 3), u.symbol())).unwrap_or_default();
            let ang = |a: Option<f64>| a.map(|a| format!("{} deg", format_number(a.to_degrees(), None, 3))).unwrap_or_default();
            let mut rows: Vec<Vec<String>> = source
                .map(|s| crate::regen::features::frames::cut_list(&s.context, None))
                .unwrap_or_default()
                .into_iter()
                .map(|r| vec![r.item.to_string(), r.qty.to_string(), if r.standard.is_empty() { r.description } else { format!("{} {}", r.standard, r.description) }, len(r.length), ang(r.angle1), ang(r.angle2)])
                .collect();
            if *bottom_to_top {
                rows.reverse();
            }
            TableData { title: None, header, rows, pending: Vec::new(), padding: [0.0; 2], header_bottom: *bottom_to_top, overridden: Vec::new(), ..Default::default() }
        }
    };
    finish(drawing, table, data)
}

/// [`table_data`] with the document-level data of `info` (custom tables).
pub fn table_data_with(drawing: &Drawing, table: &Table, view_geo: Option<&ViewGeometry>, source: Option<&SourceModel>, origin: Option<[f64; 2]>, info: &super::build::SheetInfo, expand: &dyn Fn(&str) -> String) -> TableData {
    match CustomTableKey::of(&table.kind) {
        Some(key) => finish(drawing, table, custom_data(&table.kind, info.custom_tables.get(&key).map(|r| r.as_ref()))),
        None => table_data(drawing, table, view_geo, source, origin, &info.revisions, expand),
    }
}

/// A split table's part of the rows, the drawing's cell padding and its
/// Tables style.
fn finish(drawing: &Drawing, table: &Table, mut data: TableData) -> TableData {
    let st = drawing.props.table_styles.of(&table.kind);
    let content = drawing.table_text_height(table);
    let header = if table.own_text_height { content } else { st.header.height.unwrap_or(drawing.props.ann_text_height) };
    // Live: Title row text 0.1560 in = 1.3 x the rows' 0.1200.
    let title = if table.own_text_height { content * 1.2 } else { st.title.unwrap_or(content * 1.3) };
    data.style = Some(AppliedStyle { title, header, content, title_text: st.title_text.clone(), header_text: st.header.clone(), content_text: st.content.clone(), font: st.font.clone(), line: st.line });
    // A split table shows its part of the rows.
    if let Some([a, b]) = table.rows {
        let n = data.rows.len();
        data.rows = data.rows[a.min(n)..b.min(n)].to_vec();
        data.overridden = data.overridden.iter().filter(|(r, _)| *r >= a && *r < b).map(|(r, c)| (r - a, *c)).collect();
    }
    data.padding = padding(drawing);
    data
}

// ------------------------------------------------------------ custom tables

/// What a custom table shows: table `name` of Feature Studio `fs` (at
/// `fs_version`) run on the Part Studio `source`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CustomTableKey {
    pub source: SourceKey,
    pub fs: TabId,
    pub fs_version: Option<String>,
    pub name: String,
}

impl CustomTableKey {
    pub fn of(kind: &TableKind) -> Option<Self> {
        let TableKind::Custom { studio, version, fs, name, fs_version, .. } = kind else { return None };
        Some(Self { source: SourceKey { version: version.clone(), ..SourceKey::studio(*studio) }, fs: *fs, fs_version: fs_version.clone(), name: name.clone() })
    }
}

/// Evaluated custom tables (the tables of a `TableArray`, or the error).
pub type CustomTables = HashMap<CustomTableKey, Arc<Result<Vec<CustomTable>, String>>>;

/// Every custom table a drawing shows.
pub fn custom_table_keys(drawing: &Drawing) -> Vec<CustomTableKey> {
    let mut out: Vec<CustomTableKey> = Vec::new();
    for k in drawing.sheets.iter().flat_map(|s| &s.tables).filter_map(|t| CustomTableKey::of(&t.kind)) {
        if !out.contains(&k) {
            out.push(k);
        }
    }
    out
}

/// Run custom table `name` of Feature Studio `fs` on a Part Studio's
/// regenerated `context`. `doc`: the document the Feature Studio comes from
/// (the workspace, or the document at a version when `from_version`).
pub fn evaluate_custom(doc: &Document, fs: TabId, name: &str, from_version: bool, context: &Context) -> TableResult {
    let units = doc.settings.units;
    let tab = fs.to_string();
    if !from_version {
        // The workspace's own tables: compiled with the document's other studios.
        return evaluate_table(&*DocumentScripts::from_document(doc), &tab, name, context, &units);
    }
    let studio = match doc.tab(fs).map(|t| &t.content) {
        Some(TabContent::FeatureStudio(f)) => Some((tab.clone(), crate::custom::compile(fs, &f.source))),
        _ => None,
    };
    evaluate_table(&OneStudio(studio), &tab, name, context, &units)
}

/// A Feature Studio from a version, on its own.
#[derive(Debug)]
struct OneStudio(Option<(String, Arc<crate::custom::CompiledStudio>)>);

impl CustomFeatureProvider for OneStudio {
    fn studio(&self, tab: &str) -> Option<Arc<crate::custom::CompiledStudio>> {
        self.0.as_ref().filter(|(t, _)| t == tab).map(|(_, c)| c.clone())
    }
}

/// Evaluate every custom table of a drawing whose source is in `sources`
/// (headless helper; the app evaluates in the background). `versions`
/// gives the document at a version id.
pub fn evaluate_custom_tables(doc: &Document, drawing: &Drawing, sources: &Sources, versions: &dyn Fn(&str) -> Option<Document>) -> CustomTables {
    let mut out = CustomTables::new();
    for key in custom_table_keys(drawing) {
        let Some(src) = sources.get(&key.source) else { continue };
        let at = key.fs_version.as_deref().map(versions);
        let d = match &at {
            Some(Some(d)) => d,
            Some(None) => continue,
            None => doc,
        };
        let r = evaluate_custom(d, key.fs, &key.name, key.fs_version.is_some(), &src.context);
        out.insert(key, Arc::new(r.tables));
    }
    out
}

/// Content of a custom table (live drawing/267–268): the column names as
/// the bold, centred header, a row per table row (missing values "Not set",
/// error / info cells their plain text), left-aligned whatever the column's
/// alignment; the first non-empty table of a `TableArray`, no title row.
fn custom_data(kind: &TableKind, result: Option<&Result<Vec<CustomTable>, String>>) -> TableData {
    let TableKind::Custom { bottom_to_top, .. } = kind else { return TableData::default() };
    let table = match result {
        Some(Ok(tables)) => tables.first(),
        Some(Err(e)) => return TableData { rows: vec![vec![e.clone()]], ..Default::default() },
        None => None,
    };
    let Some(t) = table else { return TableData { header: Some(vec![String::new()]), header_bold: true, ..Default::default() } };
    let header = Some(t.columns.iter().map(|c| c.name.clone()).collect());
    let col_align = vec![CellAlign::Left; t.columns.len()];
    let mut rows: Vec<Vec<String>> = t.rows.iter().map(|r| t.columns.iter().map(|c| r.text(&c.id).to_string()).collect()).collect();
    if *bottom_to_top {
        rows.reverse();
    }
    TableData { header, rows, header_bottom: *bottom_to_top, col_align, header_bold: true, ..Default::default() }
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
fn cell_height(table: &Table, data: &TableData, r: usize, c: usize) -> f64 {
    table.cell_format(r, c).height.filter(|h| *h > 0.0).unwrap_or_else(|| data.row_text_height(table, r))
}

/// Natural column widths (mm) for content. Cells merged across columns do
/// not widen single columns.
pub fn auto_widths(table: &Table, data: &TableData) -> Vec<f64> {
    let n = data.columns();
    let pad = data.padding[0] * 2.0;
    let mut w = vec![data.content_height(table) * 3.0; n];
    for (ri, row) in data.all_rows().iter().enumerate() {
        for (i, c) in row.iter().enumerate() {
            if table.merge_at(ri, i).is_some_and(|m| m.cols > 1 || (m.row, m.col) != (ri, i)) {
                continue;
            }
            let h = cell_height(table, data, ri, i);
            let tw = c.lines().map(|l| font::text_width(l, h)).fold(0.0, f64::max) * if table.shown_cell_format(ri, i).bold { 1.05 } else { 1.0 };
            w[i] = w[i].max(tw + pad);
        }
    }
    if let Some(t) = &data.title {
        let total: f64 = w.iter().sum();
        let need = font::text_width(t, data.title_height(table)) + pad;
        if need > total {
            let k = need / total;
            w.iter_mut().for_each(|x| *x *= k);
        }
    }
    w
}

/// Height (mm) a row's content needs (`vpad`: vertical padding).
pub fn row_need(table: &Table, data: &TableData, ri: usize, row: &[String], vpad: f64) -> f64 {
    let th = data.row_text_height(table, ri);
    let mut need = table.row_height.max(th + 2.0 * vpad);
    for (ci, c) in row.iter().enumerate() {
        if table.merge_at(ri, ci).is_some_and(|m| m.rows > 1 || (m.row, m.col) != (ri, ci)) {
            continue;
        }
        let h = cell_height(table, data, ri, ci);
        let lines = c.split('\n').count().max(1) as f64;
        need = need.max(table.row_height - th + h + (lines - 1.0) * h * 1.5).max(h + 2.0 * vpad + (lines - 1.0) * h * 1.5);
    }
    need
}

pub fn layout(table: &Table, data: &TableData) -> TableLayout {
    let widths = if table.col_widths.len() == data.columns() { table.col_widths.clone() } else { auto_widths(table, data) };
    let mut heights: Vec<f64> = Vec::new();
    if data.title.is_some() {
        heights.push(table.row_height.max(data.title_height(table) + 2.0 * data.padding[1]));
    }
    let all = data.all_rows();
    let set = table.row_heights.len() == all.len();
    for (ri, r) in all.iter().enumerate() {
        let need = row_need(table, data, ri, r, data.padding[1]);
        // A dragged height holds unless the text needs more.
        heights.push(if set { table.row_heights[ri].max(data.row_text_height(table, ri) * 1.4) } else { need });
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
    let line0 = g.lines.len();
    g.lines.push(Polyline2 { points: vec![[lo[0], lo[1]], [hi[0], lo[1]], [hi[0], hi[1]], [lo[0], hi[1]]], style: LineStyle::Visible, closed: true, pick });
    let th = data.content_height(table);
    let mut row0 = 0usize;
    if let Some(t) = &data.title {
        let (ya, yb) = (l.ys[0], l.ys[1]);
        g.lines.push(Polyline2 { points: vec![[lo[0], yb], [hi[0], yb]], style: LineStyle::Thin, closed: false, pick });
        g.texts.push(Text2 { pos: [(lo[0] + hi[0]) / 2.0, (ya + yb) / 2.0], height: data.title_height(table), text: t.clone(), angle: 0.0, anchor: Anchor::Center, pick });
        // Drawing properties › Tables › Title row text: Bold / Italic, colour.
        if let Some(s) = &data.style {
            let i = g.texts.len() - 1;
            let tt = &s.title_text;
            if tt.bold || tt.italic || s.font.is_some() {
                let family = s.font.as_ref().map_or_else(|| font::DEFAULT_FAMILY.into(), |f| f.family.clone());
                g.fonts.push(super::sheet::FontSpan { texts: i..i + 1, font: super::sheet::TextFont { family, bold: tt.bold, italic: tt.italic } });
            }
            if let Some(rgb) = tt.color {
                g.colors.push(super::sheet::ColorSpan { lines: 0..0, texts: i..i + 1, fills: 0..0, rgb });
            }
        }
        row0 = 1;
    }
    let rows = data.all_rows();
    let nrows = rows.len();
    let header_n = data.header.is_some() as usize;
    for &(ri, ci) in &data.overridden {
        // Overridden BOM cell: a small black triangle at the bottom left.
        let row = ri + if data.header_bottom { 0 } else { header_n };
        let (Some(&x), Some(&y)) = (l.xs.get(ci), l.ys.get(row0 + row + 1)) else { continue };
        let s = th * 0.6;
        g.fills.push(super::sheet::Fill2 { points: vec![[x + 0.2, y + 0.2], [x + 0.2 + s, y + 0.2], [x + 0.2, y + 0.2 + s]] });
    }
    for &pi in &data.pending {
        // Pending revisions: a blue row and an unfilled triangle by the label.
        let (Some(&ya), Some(&yb)) = (l.ys.get(row0 + header_n + pi), l.ys.get(row0 + header_n + pi + 1)) else { continue };
        let (xa, xb) = (lo[0] + 0.15, hi[0] - 0.15);
        g.backdrops.push((vec![[xa, yb + 0.15], [xb, yb + 0.15], [xb, ya - 0.15], [xa, ya - 0.15]], PENDING_BLUE));
        let s = th * 0.7;
        let (x, y) = (l.xs[0] + th * 0.5, (ya + yb) / 2.0 - s / 2.0);
        g.lines.push(Polyline2 { points: vec![[x, y], [x + s, y], [x + s / 2.0, y + s * 0.87]], style: LineStyle::Thin, closed: true, pick });
    }
    let ncols = l.xs.len().saturating_sub(1);
    let header_rows = data.header.is_some() as usize;
    // The heavier rule sits below the header (above it when at the bottom).
    let rule_after = if data.header_bottom { nrows.saturating_sub(1) } else { header_rows };
    let merged_across = |r0: usize, c0: usize, r1: usize, c1: usize| table.merges.iter().any(|m| m.contains(r0, c0) && m.contains(r1, c1));
    // Row dividers (interrupted by merged ranges).
    for ri in 0..nrows.saturating_sub(1) {
        let y = l.ys[row0 + ri + 1];
        let style = if header_rows > 0 && ri + 1 == rule_after { LineStyle::Visible } else { LineStyle::Thin };
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
            let fmt = table.cell_formats.iter().find(|s| s.row == ri && s.col == ci).map(|s| s.format.clone()).unwrap_or_else(|| data.default_format(ri, ci, nrows));
            let h = cell_height(table, data, ri, ci);
            let row_color = data.style.as_ref().and_then(|s| if data.is_header(ri, nrows) { s.header_text.color } else { s.content_text.color });
            let lines: Vec<&str> = c.split('\n').collect();
            let n = lines.len().max(1) as f64;
            let line_h = h * 1.5;
            let pad = data.padding[0];
            // Centre of the first line's cap height.
            let y0 = match fmt.valign {
                CellVAlign::Top => ya - h,
                CellVAlign::Middle => (ya + yb) / 2.0 + (n - 1.0) * line_h / 2.0,
                CellVAlign::Bottom => yb + h + (n - 1.0) * line_h,
            };
            for (k, text) in lines.iter().enumerate() {
                // An empty line (Alt+Enter at the end) still takes its height.
                if text.is_empty() {
                    continue;
                }
                let y = y0 - k as f64 * line_h;
                let tw = font::text_width(text, h);
                let (pos, anchor) = match fmt.align {
                    CellAlign::Center => ([(xa + xb) / 2.0, y], Anchor::Center),
                    CellAlign::Left => ([xa + pad, y - h / 2.0], Anchor::BottomLeft),
                    CellAlign::Right => ([xb - pad - tw, y - h / 2.0], Anchor::BottomLeft),
                };
                g.texts.push(Text2 { pos, height: h, text: text.to_string(), angle: 0.0, anchor, pick });
                if let Some(rgb) = row_color {
                    let i = g.texts.len() - 1;
                    g.colors.push(super::sheet::ColorSpan { lines: 0..0, texts: i..i + 1, fills: 0..0, rgb });
                }
                if fmt.bold || fmt.italic || fmt.font.is_some() {
                    let i = g.texts.len() - 1;
                    let family = fmt.font.clone().unwrap_or_else(|| font::DEFAULT_FAMILY.into());
                    g.fonts.push(super::sheet::FontSpan { texts: i..i + 1, font: super::sheet::TextFont { family, bold: fmt.bold, italic: fmt.italic } });
                }
                // Underline / strikethrough as on notes.
                let x0 = if anchor == Anchor::Center { pos[0] - tw / 2.0 } else { pos[0] };
                let base = if anchor == Anchor::Center { pos[1] - h / 2.0 } else { pos[1] };
                for (on, dy) in [(fmt.underline, -0.2), (fmt.strikethrough, 0.5)] {
                    if on && !text.is_empty() {
                        g.lines.push(Polyline2 { points: vec![[x0, base + h * dy], [x0 + tw, base + h * dy]], style: LineStyle::Thin, closed: false, pick });
                    }
                }
            }
        }
    }
    // Drawing properties › Tables › Line thickness: every line of the table.
    if let Some(s) = &data.style {
        g.pens.push(super::sheet::PenSpan { centered: false, lines: line0..g.lines.len(), width_mm: s.line, dashes: Vec::new() });
    }
}

/// A table drawn into the CAD export layers (DWG/DXF objects); [`SheetGeometry`] is drawing data, the table
/// layout comes from here.
pub trait CadTables {
    /// Everything since `m` is this table.
    fn add_cad_table(&mut self, m: GeomMark, layout: &TableLayout, data: &TableData, kind: &TableKind);
}

impl CadTables for SheetGeometry {
    /// Everything since `m` is this table.
    fn add_cad_table(&mut self, m: GeomMark, layout: &TableLayout, data: &TableData, kind: &TableKind) {
        if layout.xs.len() < 2 || layout.ys.len() < 2 {
            return;
        }
        let mut cells: Vec<Vec<String>> = Vec::new();
        if layout.title {
            cells.push(vec![data.title.clone().unwrap_or_default()]);
        }
        cells.extend(data.all_rows().into_iter().cloned());
        let widths = layout.xs.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        let heights: Vec<f64> = layout.ys.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        // Row styles (live drw2-k-dxf2018 / drw2-en-ansi-a STYLE table).
        let [title, header, row] = table_row_styles(kind);
        let n = heights.len();
        let body = layout.title as usize;
        let header_row = data.header.is_some().then(|| if data.header_bottom { n - 1 } else { body });
        let rows = (0..n).map(|r| if layout.title && r == 0 { title } else if Some(r) == header_row { header } else { row }).collect();
        let aligned = matches!(kind, TableKind::General { .. });
        let obj = self.cad_objects.len();
        self.cad_objects.push(CadObject::Table { corner: [layout.xs[0], layout.ys[0]], widths, heights, cells, rows, aligned });
        self.cad_members.push(CadMember { lines: m.0..self.lines.len(), texts: m.1..self.texts.len(), fills: m.2..self.fills.len(), obj });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_table_layout() {
        let t = Table::new(1, TableKind::General { cells: vec![vec!["A".into(), "Long header".into()], vec!["1".into(), "x\ny".into()]], title: Some("T".into()), header: true }, [400.0, 280.0]);
        let d = table_data(&Drawing::default(), &t, None, None, None, &[], &|s| s.to_string());
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

    #[test]
    fn revision_table_rows() {
        use crate::release::{ItemKind, Package, PackageKind, ReleaseItem, ReleaseLog};
        let tab = crate::document::TabId(7);
        let item = |rev: &str| ReleaseItem { tab, part: None, name: "Drawing 1".into(), kind: ItemKind::Drawing, part_number: "DRW-1".into(), revision: rev.into(), not_revision_managed: false, reverted: false, config: Default::default(), document: None };
        let pkg = |id: u64, kind: PackageKind, rev: &str, notes: &str, day: i64| Package { id, kind, name: format!("R{id}"), notes: notes.into(), created: day * 86_400_000, version: None, items: vec![item(rev)], re_releasable: false };
        let mut log = ReleaseLog::default();
        log.packages.push(pkg(1, PackageKind::Release, "A", "Initial release", 20744));
        log.packages.push(pkg(2, PackageKind::Release, "B", "Hole moved", 20745));
        log.drafts.push(pkg(3, PackageKind::Release, "C", "Pending", 0));
        let rows = revision_rows(&log, tab);
        assert_eq!(rows.iter().map(|r| (r.revision.as_str(), r.pending)).collect::<Vec<_>>(), vec![("A", false), ("B", false), ("C", true)]);
        assert_eq!(rows[0].date, "2026-10-18");
        assert!(revision_rows(&log, crate::document::TabId(8)).is_empty());
        let mut t = Table::new(1, TableKind::revision(), [400.0, 280.0]);
        let d = table_data(&Drawing::default(), &t, None, None, None, &rows, &|s| s.to_string());
        assert_eq!(d.title.as_deref(), Some("REVISION HISTORY"));
        assert_eq!(d.header.as_ref().unwrap()[0], "REV");
        assert_eq!(d.rows.iter().map(|r| r[0].as_str()).collect::<Vec<_>>(), vec!["A", "B", "C*"]);
        assert_eq!((d.rows[1][1].as_str(), d.pending.as_slice()), ("Hole moved", &[2][..]));
        // Two visible rows, newest at the top.
        if let TableKind::Revision { visible_rows, newest_top, .. } = &mut t.kind {
            (*visible_rows, *newest_top) = (2, true);
        }
        let d = table_data(&Drawing::default(), &t, None, None, None, &rows, &|s| s.to_string());
        assert_eq!(d.rows.iter().map(|r| r[0].as_str()).collect::<Vec<_>>(), vec!["C*", "B"]);
        assert_eq!(d.pending, vec![0]);
        // A company template: its columns, in capitals with "All uppercase".
        let tpl = RevisionTemplate { name: "Ours".into(), columns: vec!["Revision".into(), "Part number".into(), "Revision description".into()], all_caps: true };
        let d = revision_data_with(&t.kind, &rows, Some(&tpl));
        assert_eq!(d.header.as_deref(), Some(&["REVISION".to_string(), "PART NUMBER".into(), "REVISION DESCRIPTION".into()][..]));
        assert_eq!(d.rows[0], vec!["C*".to_string(), "DRW-1".into(), "PENDING".into()]);
        assert!(RevisionTemplate::default().is_default());
        // The pending row gets a blue backdrop.
        let mut g = SheetGeometry::new(420.0, 297.0);
        render(&mut g, &t, &d, &layout(&t, &d), 1);
        assert_eq!(g.backdrops.len(), 1);
        assert_eq!(g.backdrops[0].1, PENDING_BLUE);
    }
}
