//! Custom tables (`annotation { "Table Type Name" : ... } export const t =
//! defineTable(function(context, definition) { ... })`): evaluated against
//! a Part Studio's final state, they give titled tables of columns and rows
//! (std table.fs) that the Part Studio's Custom tables panel and drawing
//! custom tables show.

use super::host::{self, FsContext};
use crate::reference::{FeatureId, Reference};
use crate::regen::context::Context;
use noshape_featurescript::interp::{Interp, Notice};
use noshape_featurescript::value::{Value, to_display};
use noshape_units::{Dim, Quantity, UnitPrefs};
use std::time::{Duration, Instant};

/// The Custom tables panel's text when a table throws.
pub const TABLE_ERRORS: &str = "FeatureScript execution produced table errors. See the FeatureScript notices pane for more information.";

const TIME_LIMIT: Duration = Duration::from_secs(60);
const STACK_SIZE: usize = 256 << 20;

/// One table of a custom table's result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomTable {
    pub title: String,
    /// Shown after the title, which truncates first.
    pub title_suffix: Option<String>,
    pub columns: Vec<TableColumn>,
    pub rows: Vec<TableRow>,
    /// Entities the whole table is about (cross-highlighting).
    pub entities: Vec<Reference>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableColumn {
    pub id: String,
    pub name: String,
    /// `TableTextAlignment` member (`LEFT`, `CENTER`, `RIGHT`), if given.
    pub alignment: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableRow {
    /// Cells by column id (missing ids are empty cells).
    pub cells: Vec<(String, TableCell)>,
    pub entities: Vec<Reference>,
}

impl TableRow {
    pub fn cell(&self, column: &str) -> Option<&TableCell> {
        self.cells.iter().find(|(c, _)| c == column).map(|(_, v)| v)
    }

    /// The cell's text, or [`NOT_SET`] for a column the row has no value for.
    pub fn text(&self, column: &str) -> &str {
        self.cell(column).map_or(NOT_SET, |c| c.text.as_str())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableCell {
    pub text: String,
    /// `TableCellError`: shown in error state with this tooltip.
    pub error: Option<String>,
    /// `TableCellWithInfo`: an info icon with this tooltip.
    pub info: Option<String>,
}

/// What a table run produced: the tables, or the error, plus `println`
/// output and notices. Live: a table that runs normally shows no output;
/// when it throws, the panel shows [`TABLE_ERRORS`] and the notices pane a
/// group "<table> (<Part Studio>)" with the error, its stack and the output.
#[derive(Clone, Debug)]
pub struct TableResult {
    pub tables: Result<Vec<CustomTable>, String>,
    pub output: String,
    pub notices: Vec<Notice>,
}

/// Run custom table `name` of the studio `studio_tab` on `ctx` (a Part
/// Studio's final context).
pub fn evaluate_table(provider: &dyn super::CustomFeatureProvider, studio_tab: &str, name: &str, ctx: &Context, units: &UnitPrefs) -> TableResult {
    let fail = |m: String| TableResult { tables: Err(m), output: String::new(), notices: Vec::new() };
    let Some(studio) = provider.studio(studio_tab) else { return fail("The Feature Studio defining this table no longer exists".into()) };
    if let Some(d) = studio.first_error() {
        return fail(format!("Feature Studio has errors: {}", d.message));
    }
    let Some(def) = studio.table(name).cloned() else { return fail(format!("The Feature Studio no longer defines \"{name}\"")) };
    noshape_featurescript::value::reset_shown_numbers();
    let cx = FsContext::new(ctx.clone(), FeatureId(0), def.display_name.clone(), *units, Default::default(), None);
    let ctx_value = cx.value();
    let counters = noshape_featurescript::value::shown_counters();
    let units = *units;
    let run = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(STACK_SIZE)
            .spawn_scoped(s, || {
                noshape_featurescript::value::set_shown_counters(counters);
                let mut i = Interp::new();
                i.set_deadline(Some(Instant::now() + TIME_LIMIT));
                let r = i.global(&def.cell).and_then(|f| i.call_value(&f, vec![ctx_value, Value::map(Default::default())]));
                let tables = match r {
                    Ok(v) => read_tables(&mut i, &cx, &v, &units),
                    Err(e) => Err(e.message()),
                };
                (tables, std::mem::take(&mut i.notices), std::mem::take(&mut i.output))
            })
            .map(|h| h.join())
    });
    match run {
        Ok(Ok((tables, notices, output))) => TableResult { tables, output, notices },
        _ => fail("Table evaluation crashed".into()),
    }
}

/// A `Table` or `TableArray` value.
fn read_tables(i: &mut Interp, cx: &FsContext, v: &Value, units: &UnitPrefs) -> Result<Vec<CustomTable>, String> {
    match v.as_array() {
        // Live: tables without rows are not shown at all.
        Some(arr) => Ok(arr.iter().map(|t| read_table(i, cx, t, units)).collect::<Result<Vec<_>, _>>()?.into_iter().filter(|t| !t.rows.is_empty()).collect()),
        None if v.get("columnDefinitions").as_array().is_some() => Ok(vec![read_table(i, cx, v, units)?]),
        None => Err(format!("A custom table must return a Table or TableArray, not {}", to_display(v))),
    }
}

fn read_table(i: &mut Interp, cx: &FsContext, t: &Value, units: &UnitPrefs) -> Result<CustomTable, String> {
    let entities = |i: &mut Interp, q: &Value| -> Result<Vec<Reference>, String> {
        if q.is_undefined() {
            return Ok(Vec::new());
        }
        host::query::evaluate_refs(i, cx, q).map_err(|e| e.message())
    };
    let columns = t
        .get("columnDefinitions")
        .as_array()
        .map(|cols| {
            cols.iter()
                .map(|c| TableColumn {
                    id: c.get("id").as_str().map(|s| s.to_string()).unwrap_or_default(),
                    name: cell_text(c.get("name"), units),
                    alignment: c.get("alignment").as_str().map(|s| s.to_string()),
                })
                .collect()
        })
        .unwrap_or_default();
    let mut rows = Vec::new();
    for r in t.get("rows").as_array().iter().flat_map(|a| a.iter()) {
        let mut cells = Vec::new();
        if let Some(m) = r.get("columnIdToCell").as_map() {
            for (k, v) in m.iter() {
                let key = k.as_str().map(|s| s.to_string()).unwrap_or_default();
                cells.push((key, table_cell(v, units)));
            }
        }
        rows.push(TableRow { cells, entities: entities(i, r.get("entities"))? });
    }
    let suffix = t.get("titleSuffix");
    Ok(CustomTable {
        title: cell_text(t.get("title"), units),
        title_suffix: (!suffix.is_undefined()).then(|| cell_text(suffix, units)),
        columns,
        rows,
        entities: entities(i, t.get("entities"))?,
    })
}

fn table_cell(v: &Value, units: &UnitPrefs) -> TableCell {
    let opt = |k: &str| {
        let x = v.get(k);
        (!x.is_undefined()).then(|| cell_text(x, units))
    };
    match (opt("error"), opt("info")) {
        (Some(e), _) => TableCell { text: cell_text(v.get("value"), units), error: Some(e), info: None },
        (None, Some(info)) => TableCell { text: cell_text(v.get("value"), units), error: None, info: Some(info) },
        _ => TableCell { text: cell_text(v, units), ..Default::default() },
    }
}

/// A table value as shown (live dom/fs-tables-*.txt): strings as they are,
/// numbers and quantities in the document units with its display decimals
/// ("0.333", "40 mm", "30 deg", "1290.32 mm²", "1500000000 mm³"),
/// `valueWithUnitsAndPrecision` with exactly that many decimals ("25.4000 mm"),
/// `templateString`s with their `#key`s filled in.
pub fn cell_text(v: &Value, units: &UnitPrefs) -> String {
    if let Some(s) = v.as_str() {
        return s.to_string();
    }
    if let Some(template) = v.get("template").as_str() {
        return fill_template(&template, v, units);
    }
    if !v.get("precision").is_undefined()
        && let (Some(q), Some(p)) = (quantity(v.get("value")), v.get("precision").as_number())
    {
        return format_cell(q, units, Some(p.clamp(0., 12.) as usize));
    }
    match quantity(v) {
        Some(q) => format_cell(q, units, None),
        None => to_display(v),
    }
}

/// The text a table panel shows for a column the row has no value for.
pub const NOT_SET: &str = "Not set";

fn format_cell(q: Quantity, units: &UnitPrefs, precision: Option<usize>) -> String {
    let v = q.value / units.si_factor(q.dim);
    let num = match precision {
        Some(p) => {
            let s = format!("{v:.p$}");
            if s.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') { s.trim_start_matches('-').to_string() } else { s }
        }
        None => noshape_units::format_number(v, if q.dim == Dim::ANGLE { units.angle_precision } else { units.length_precision }),
    };
    let sup = |e: i8| match e {
        1 => String::new(),
        2 => "²".into(),
        3 => "³".into(),
        e => format!("^{e}"),
    };
    let mut parts = Vec::new();
    if q.dim.length != 0 {
        parts.push(format!("{}{}", units.length.symbol(), sup(q.dim.length)));
    }
    if q.dim.angle != 0 {
        parts.push(format!("{}{}", units.angle.symbol(), sup(q.dim.angle)));
    }
    if parts.is_empty() { num } else { format!("{num} {}", parts.join("*")) }
}

fn quantity(v: &Value) -> Option<Quantity> {
    if let Some(n) = v.as_number() {
        return Some(Quantity::new(n, Dim::UNITLESS));
    }
    // ValueWithUnits: { value, unit : { meter : 2, radian : 1 } } (SI).
    let units = v.get("unit").as_map()?;
    let mut dim = Dim::UNITLESS;
    for (k, e) in units.iter() {
        let e = e.as_number()? as i8;
        match &*k.as_str()? {
            "meter" => dim.length = e,
            "radian" => dim.angle = e,
            _ => return host::quantity_of(v),
        }
    }
    Some(Quantity::new(v.get("value").as_number()?, dim))
}

/// `#key` -> the entry's text (longest key first, so `#ab` wins over `#a`).
fn fill_template(template: &str, v: &Value, units: &UnitPrefs) -> String {
    let mut keys: Vec<(String, String)> = v
        .as_map()
        .map(|m| m.iter().filter_map(|(k, x)| k.as_str().filter(|k| &**k != "template").map(|k| (k.to_string(), cell_text(x, units)))).collect())
        .unwrap_or_default();
    keys.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    let mut out = String::new();
    let mut rest = template;
    while let Some(at) = rest.find('#') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        match keys.iter().find(|(k, _)| after.starts_with(k.as_str())) {
            Some((k, text)) => {
                out.push_str(text);
                rest = &after[k.len()..];
            }
            None => {
                out.push('#');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}
