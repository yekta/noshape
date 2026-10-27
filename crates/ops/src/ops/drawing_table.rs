//! The drawing table menus: cell format, merge, insert / remove /
//! duplicate rows and columns, size, resize, split, hole table directions
//! and precision, BOM type and order, fixed corner and text height. Shared
//! code in `noshape_scene::drawing_tools::table_edit` and the model's
//! `Table` methods, with the editor's edit texts.

use super::drawing::{commit, drawing_field, reply, sheet_field};
use super::drawing_tools::{Target, target};
use super::doc_field;
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::Ctx;
use noshape_model::drawing::{CellAlign, Table, TableCorner, TableKind};
use noshape_scene::drawing_tools::table_edit::{self as te, FormatFields, FormatScope};
use serde_json::{Value, json};

const ACTIONS: [&str; 21] = [
    "format_cell",
    "merge_right",
    "merge_below",
    "merge",
    "unmerge",
    "insert",
    "remove",
    "duplicate",
    "columns_equally",
    "rows_equally",
    "resize",
    "split",
    "unsplit",
    "flip_x",
    "flip_y",
    "precision",
    "show_location",
    "order",
    "bom_type",
    "fixed_corner",
    "text_height",
];

const CORNERS: [(&str, TableCorner); 4] = [("top_right", TableCorner::TopRight), ("top_left", TableCorner::TopLeft), ("bottom_right", TableCorner::BottomRight), ("bottom_left", TableCorner::BottomLeft)];

pub fn ops() -> Vec<Op> {
    let int = || json!({ "type": "integer", "minimum": 0 });
    vec![Op {
        group: "drawing",
        verb: "table_edit",
        summary: "Edit a table from its menus: cells, merge, rows / columns, size, split, order, corner, text height",
        description: "The table context menus, one edit each. Cells are (`row`, `column`) counted from 0 below the title (the header is row 0); a block of cells runs to (`to_row`, `to_column`).\n- `format_cell`: Format cell… (`scope` cell | row | column | table; `align` left | center | right, `bold`, `height` mm or null for the table's) (\"Format cells\").\n- `merge_right` / `merge_below` (\"Merge cells\"), `unmerge` (\"Unmerge cells\").\n- General tables, a block selected: `merge` (`mode` all | horizontal | vertical), `insert` (`at` column_left | column_right | row_above | row_below), `remove` / `duplicate` (`what` columns | rows); edit texts the menu's (\"All\", \"Column Left\", \"Rows\"…).\n- `columns_equally` / `rows_equally`: Size ▸ — of the block's columns / rows when a block is given (two or more), else the whole table.\n- `resize`: Resize… column `column`'s `width` and the row `height` (mm) (\"Resize cell\").\n- BOM, cut list and custom tables: `split` below displayed row `row` (\"Split table\"; the reply's `table` is the new part), `unsplit` (\"Unsplit table\").\n- Hole tables: `flip_x`, `flip_y`, `precision` (`decimals` 0–4 or null for the drawing's).\n- `order` (`to` top_to_bottom | bottom_to_top; BOM, cut list and hole tables), `show_location` (`on`: a hole table's X LOC / Y LOC columns, Show location data), `bom_type` (`to` flattened | structured).\n- `fixed_corner` (`to` top_right | top_left | bottom_right | bottom_left; the table stays put), `text_height` (`to` 2.5 | 3 | 3.5 | 5 mm), `fit` (Fit to content).",
        fields: vec![
            doc_field(),
            drawing_field(),
            sheet_field(),
            req("table", int(), "Table id (drawing_get)."),
            req("action", json!({ "type": "string", "enum": ACTIONS.iter().chain(&["fit"]).collect::<Vec<_>>() }), "The menu action."),
            opt("row", int(), "The cell's row (0 = header / first row)."),
            opt("column", int(), "The cell's column."),
            opt("to_row", int(), "A block's last row (default `row`)."),
            opt("to_column", int(), "A block's last column (default `column`)."),
            opt("scope", json!({ "type": "string", "enum": ["cell", "row", "column", "table"], "default": "cell" }), "format_cell: what it applies to."),
            opt("align", json!({ "type": "string", "enum": ["left", "center", "right"] }), "format_cell: horizontal alignment (default: the cell's)."),
            opt("bold", json!({ "type": "boolean" }), "format_cell: bold (default: the cell's)."),
            opt("height", json!({ "type": ["number", "null"] }), "format_cell: text height mm (null: the table's); resize: row height mm."),
            opt("width", json!({ "type": "number" }), "resize: column width mm."),
            opt("mode", json!({ "type": "string", "enum": ["all", "horizontal", "vertical"], "default": "all" }), "merge."),
            opt("at", json!({ "type": "string", "enum": ["column_left", "column_right", "row_above", "row_below"] }), "insert."),
            opt("what", json!({ "type": "string", "enum": ["columns", "rows"] }), "remove, duplicate."),
            opt("decimals", json!({ "type": ["integer", "null"], "minimum": 0, "maximum": 4 }), "precision."),
            opt("on", json!({ "type": "boolean", "default": true }), "show_location."),
            opt("to", json!({ "type": ["string", "number"] }), "order, bom_type, fixed_corner, text_height: the choice."),
        ],
        mutates: true,
        destructive: false,
        run: table_edit,
    }]
}

fn cell(a: &Args, t: &Table, nrows: usize, ncols: usize) -> Result<(usize, usize), OpError> {
    let r = a.opt_u64("row")?.ok_or_else(|| OpError::param("row", "required for this action"))? as usize;
    let c = a.opt_u64("column")?.ok_or_else(|| OpError::param("column", "required for this action"))? as usize;
    if r >= nrows {
        return Err(OpError::param("row", &format!("rows are 0 to {}", nrows.saturating_sub(1))));
    }
    if c >= ncols {
        return Err(OpError::param("column", &format!("columns are 0 to {}", ncols.saturating_sub(1))));
    }
    Ok(t.anchor_cell(r, c))
}

/// The block from (`row`, `column`) to (`to_row`, `to_column`), ordered.
fn block(a: &Args, nrows: usize, ncols: usize) -> Result<(usize, usize, usize, usize), OpError> {
    let r = a.opt_u64("row")?.ok_or_else(|| OpError::param("row", "required for this action"))? as usize;
    let c = a.opt_u64("column")?.ok_or_else(|| OpError::param("column", "required for this action"))? as usize;
    let r2 = a.opt_u64("to_row")?.map_or(r, |x| x as usize);
    let c2 = a.opt_u64("to_column")?.map_or(c, |x| x as usize);
    for (k, v, n) in [("row", r, nrows), ("to_row", r2, nrows), ("column", c, ncols), ("to_column", c2, ncols)] {
        if v >= n {
            return Err(OpError::param(k, &format!("0 to {}", n.saturating_sub(1))));
        }
    }
    Ok((r.min(r2), r.max(r2), c.min(c2), c.max(c2)))
}

fn choice<T: Copy>(a: &Args, name: &str, options: &[(&str, T)]) -> Result<T, OpError> {
    let s = a.get(name).and_then(Value::as_str);
    s.and_then(|s| options.iter().find(|o| o.0 == s)).map(|o| o.1).ok_or_else(|| OpError::param(name, &format!("one of {}", options.iter().map(|o| o.0).collect::<Vec<_>>().join(", "))))
}

fn table_edit(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t: Target = target(od, a)?;
    let id = a.opt_u64("table")?.ok_or_else(|| OpError::param("table", "a table id (drawing_get)"))?;
    let sheet = &t.d.sheets[t.sheet];
    let table = sheet.table(id).cloned().ok_or_else(|| OpError::param("table", "no such table on the sheet"))?;
    let layout = t.m.built.as_ref().and_then(|b| b.table_layout(id)).cloned();
    let (nrows, ncols) = layout.as_ref().map(te::counts).unwrap_or((0, 0));
    let sheet_mm = sheet.size.size_mm();
    let general = matches!(table.kind, TableKind::General { .. });
    let data = matches!(table.kind, TableKind::Bom { .. } | TableKind::CutList { .. } | TableKind::Custom { .. });
    let action = a.str("action")?;
    let refuse = |why: &str| Err(OpError::param("action", &format!("{action}: {why}")));
    let need_layout = || layout.clone().ok_or_else(|| OpError::param("table", "the table has no layout (empty?)"));
    let mut new_table = None;
    let edit = |t: &mut Target, f: &dyn Fn(&mut Table)| {
        if let Some(x) = t.d.sheets[t.sheet].table_mut(id) {
            f(x);
        }
    };
    let desc: &str = match action {
        "format_cell" | "merge_right" | "merge_below" | "unmerge" if data => return refuse("not on BOM, cut list or custom tables"),
        "format_cell" => {
            let (r, c) = cell(a, &table, nrows, ncols)?;
            let scope = match a.opt_str("scope")? {
                None => FormatScope::Cell,
                Some(_) => choice(a, "scope", &[("cell", FormatScope::Cell), ("row", FormatScope::Row), ("column", FormatScope::Column), ("table", FormatScope::Table)])?,
            };
            let cur = table.shown_cell_format(r, c);
            let align = match a.opt_str("align")? {
                None => cur.align,
                Some(_) => choice(a, "align", &[("left", CellAlign::Left), ("center", CellAlign::Center), ("right", CellAlign::Right)])?,
            };
            let height = match a.value.get("height") {
                None => cur.height,
                Some(Value::Null) => None,
                Some(h) => Some(h.as_f64().filter(|h| *h > 0.5).ok_or_else(|| OpError::param("height", "a text height in mm (over 0.5), or null"))?),
            };
            let style = FormatFields { align, bold: a.bool_or("bold", cur.bold)?, height };
            let l = layout.clone();
            edit(&mut t, &move |x| te::format_cells(x, l.as_ref(), r, c, scope, style));
            "Format cells"
        }
        "merge_right" | "merge_below" => {
            let l = need_layout()?;
            let (r, c) = cell(a, &table, nrows, ncols)?;
            let mut x = table.clone();
            let ok = if action == "merge_right" { te::merge_right(&mut x, &l, r, c) } else { te::merge_below(&mut x, &l, r, c) };
            if !ok {
                return refuse("no cell there to merge with");
            }
            edit(&mut t, &move |y| *y = x.clone());
            "Merge cells"
        }
        "unmerge" => {
            let (r, c) = cell(a, &table, nrows, ncols)?;
            if table.merge_at(r, c).is_none() {
                return refuse("the cell is not merged");
            }
            edit(&mut t, &move |x| {
                x.unmerge(r, c);
            });
            if general { "Unmerge" } else { "Unmerge cells" }
        }
        "merge" | "insert" | "remove" | "duplicate" if !general => return refuse("general tables only"),
        "merge" => {
            let (r0, r1, c0, c1) = block(a, nrows, ncols)?;
            if r1 == r0 && c1 == c0 {
                return refuse("select two or more cells (`to_row` / `to_column`)");
            }
            let mode = match a.opt_str("mode")? {
                None => "all",
                Some(_) => choice(a, "mode", &[("all", "all"), ("horizontal", "horizontal"), ("vertical", "vertical")])?,
            };
            edit(&mut t, &move |x| match mode {
                "horizontal" => (r0..=r1).for_each(|r| {
                    x.merge_cells((r, c0), (r, c1));
                }),
                "vertical" => (c0..=c1).for_each(|c| {
                    x.merge_cells((r0, c), (r1, c));
                }),
                _ => {
                    x.merge_cells((r0, c0), (r1, c1));
                }
            });
            match mode {
                "horizontal" => "Horizontal",
                "vertical" => "Vertical",
                _ => "All",
            }
        }
        "insert" => {
            let (r0, r1, c0, c1) = block(a, nrows, ncols)?;
            let at = choice(a, "at", &[("column_left", 0), ("column_right", 1), ("row_above", 2), ("row_below", 3)])?;
            edit(&mut t, &move |x| match at {
                0 => x.insert_col(c0),
                1 => x.insert_col(c1 + 1),
                2 => x.insert_row(r0),
                _ => x.insert_row(r1 + 1),
            });
            ["Column Left", "Column Right", "Row Above", "Row Below"][at]
        }
        "remove" | "duplicate" => {
            let (r0, r1, c0, c1) = block(a, nrows, ncols)?;
            let cols = choice(a, "what", &[("columns", true), ("rows", false)])?;
            let remove = action == "remove";
            edit(&mut t, &move |x| match (remove, cols) {
                (true, true) => (c0..=c1).rev().for_each(|c| {
                    x.delete_col(c);
                }),
                (true, false) => (r0..=r1).rev().for_each(|r| {
                    x.delete_row(r);
                }),
                (false, true) => x.duplicate_cols(c0, c1),
                (false, false) => x.duplicate_rows(r0, r1),
            });
            if cols { "Columns" } else { "Rows" }
        }
        "columns_equally" | "rows_equally" => {
            let l = need_layout()?;
            let cols = action == "columns_equally";
            if a.get("row").is_some() || a.get("column").is_some() {
                let (r0, r1, c0, c1) = block(a, nrows, ncols)?;
                if (cols && c1 == c0) || (!cols && r1 == r0) {
                    return refuse("the block spans one column / row; give two or more, or no block for the whole table");
                }
                let (cs, rs): (Vec<usize>, Vec<usize>) = ((c0..=c1).collect(), (r0..=r1).collect());
                edit(&mut t, &move |x| if cols { te::columns_equally_of(x, &l, &cs) } else { te::rows_equally_of(x, &l, &rs) });
            } else if data {
                return refuse("BOM, cut list and custom tables size the selected cells' columns / rows: give a block");
            } else {
                edit(&mut t, &move |x| if cols { te::columns_equally(x, &l) } else { te::rows_equally(x, &l) });
            }
            if cols { "Size columns equally" } else { "Size rows equally" }
        }
        "resize" => {
            let c = a.opt_u64("column")?.ok_or_else(|| OpError::param("column", "the column to resize"))? as usize;
            if c >= ncols {
                return Err(OpError::param("column", &format!("columns are 0 to {}", ncols.saturating_sub(1))));
            }
            let w = a.opt_f64("width")?.map(|w| if w > 0.5 { Ok(w) } else { Err(OpError::param("width", "over 0.5 mm")) }).transpose()?;
            let h = a.opt_f64("height")?.map(|h| if h > 0.5 { Ok(h) } else { Err(OpError::param("height", "over 0.5 mm")) }).transpose()?;
            if w.is_none() && h.is_none() {
                return Err(OpError::param("width", "give `width` and / or `height`"));
            }
            let l = layout.clone();
            edit(&mut t, &move |x| te::resize_cell(x, l.as_ref(), sheet_mm, c, w, h));
            "Resize cell"
        }
        "split" | "unsplit" if !matches!(table.kind, TableKind::Bom { .. } | TableKind::CutList { .. } | TableKind::Custom { .. }) => return refuse("BOM, cut list and custom tables only"),
        "split" => {
            let l = need_layout()?;
            let r = a.opt_u64("row")?.ok_or_else(|| OpError::param("row", "the displayed row to split below"))? as usize;
            let split = te::split_at(&table, &l, r).ok_or_else(|| OpError::param("row", "no data row below it to split off"))?;
            new_table = te::split_table(&mut t.d.sheets[t.sheet], id, &l, split);
            "Split table"
        }
        "unsplit" => {
            let l = need_layout()?;
            if !te::unsplit_table(&mut t.d.sheets[t.sheet], id, &l) {
                return refuse("the first part of a list can't be unsplit; give a later part");
            }
            "Unsplit table"
        }
        "flip_x" | "flip_y" | "precision" | "show_location" if !matches!(table.kind, TableKind::Hole { .. }) => return refuse("hole tables only"),
        "show_location" => {
            let on = a.bool_or("on", true)?;
            edit(&mut t, &move |x| {
                if let TableKind::Hole { show_location, .. } = &mut x.kind {
                    *show_location = on;
                }
            });
            "Show location data"
        }
        "flip_x" | "flip_y" => {
            let fx = action == "flip_x";
            edit(&mut t, &move |x| {
                if let TableKind::Hole { flip_x, flip_y, .. } = &mut x.kind {
                    if fx { *flip_x = !*flip_x } else { *flip_y = !*flip_y }
                }
            });
            if fx { "Flip X direction" } else { "Flip Y direction" }
        }
        "precision" => {
            let p = match a.value.get("decimals") {
                None => return Err(OpError::param("decimals", "0 to 4, or null for the drawing's")),
                Some(Value::Null) => None,
                Some(v) => Some(v.as_u64().filter(|d| *d <= 4).ok_or_else(|| OpError::param("decimals", "0 to 4, or null for the drawing's"))? as u8),
            };
            edit(&mut t, &move |x| {
                if let TableKind::Hole { precision, .. } = &mut x.kind {
                    *precision = p;
                }
            });
            "Hole table precision"
        }
        "order" => {
            let up = choice(a, "to", &[("top_to_bottom", false), ("bottom_to_top", true)])?;
            let desc = match table.kind {
                TableKind::Bom { .. } => "BOM order",
                TableKind::CutList { .. } => "Cut list order",
                TableKind::Hole { .. } => "Hole table order",
                _ => return refuse("BOM, cut list and hole tables only"),
            };
            edit(&mut t, &move |x| match &mut x.kind {
                TableKind::Bom { bottom_to_top, .. } | TableKind::CutList { bottom_to_top, .. } | TableKind::Hole { bottom_to_top, .. } => *bottom_to_top = up,
                _ => {}
            });
            desc
        }
        "bom_type" => {
            if !matches!(table.kind, TableKind::Bom { .. }) {
                return refuse("BOM tables only");
            }
            let s = choice(a, "to", &[("flattened", false), ("structured", true)])?;
            edit(&mut t, &move |x| {
                if let TableKind::Bom { structured, .. } = &mut x.kind {
                    *structured = s;
                }
            });
            "BOM type"
        }
        "fixed_corner" | "text_height" | "fit" if data => return refuse("BOM, cut list and custom tables set these in their table properties"),
        "fixed_corner" => {
            let k = choice(a, "to", &CORNERS)?;
            let l = layout.clone();
            edit(&mut t, &move |x| te::set_fixed_corner(x, l.as_ref(), k));
            "Table fixed corner"
        }
        "text_height" => {
            let h = a.get("to").and_then(Value::as_f64).filter(|h| te::TEXT_HEIGHTS.iter().any(|x| (x - h).abs() < 1e-9)).ok_or_else(|| OpError::param("to", "2.5, 3, 3.5 or 5 (mm)"))?;
            edit(&mut t, &move |x| te::set_text_height(x, h));
            "Table text height"
        }
        "fit" => {
            edit(&mut t, &te::fit_to_content);
            "Fit table"
        }
        other => return Err(OpError::param("action", &format!("`{other}`: one of {}, fit", ACTIONS.join(", ")))),
    };
    commit(od, t.tab, t.sheet, t.d, desc)?;
    let mut out = reply(od, t.tab, t.sheet, false)?;
    out["table"] = json!(new_table.unwrap_or(id));
    Ok(out.into())
}
