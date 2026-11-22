//! The Note panel's options (drawing_note) and a note's style in drawing_get.

use super::drawing_tools::enum_arg;
use crate::error::OpError;
use crate::registry::{Args, Field, opt};
use noshape_model::drawing::{CellAlign, CellFormat, CellVAlign};
use noshape_model::drawing::note::{FlagShape, NoteAlign, NoteList, NoteStyle, NoteVAlign};
use noshape_model::drawing::sheet::{DRAWING_DEFAULT_FONT, FONT_FAMILIES, TextFont, is_shx};
use serde_json::{Value, json};

const ALIGNS: [(&str, NoteAlign); 4] = [("left", NoteAlign::Left), ("center", NoteAlign::Center), ("right", NoteAlign::Right), ("justify", NoteAlign::Justified)];
const VALIGNS: [(&str, NoteVAlign); 3] = [("top", NoteVAlign::Top), ("middle", NoteVAlign::Middle), ("bottom", NoteVAlign::Bottom)];
const LISTS: [(&str, NoteList); 5] = [("none", NoteList::None), ("numbers", NoteList::Numbers), ("bullets", NoteList::Bullets), ("uppercase_letters", NoteList::UpperLetters), ("lowercase_letters", NoteList::LowerLetters)];
const SPACINGS: [f64; 5] = [1.0, 1.5, 2.0, 2.5, 3.0];

fn shape_key(k: FlagShape) -> String {
    k.name().to_lowercase().replace(' ', "_")
}

fn key_of<T: PartialEq + Copy>(list: &[(&'static str, T)], v: T) -> &'static str {
    list.iter().find(|x| x.1 == v).map(|x| x.0).unwrap_or("")
}

pub(super) fn fields() -> Vec<Field> {
    let shapes: Vec<String> = FlagShape::ALL.iter().map(|k| shape_key(*k)).collect();
    vec![
        opt("font", json!({ "type": "string", "enum": FONT_FAMILIES }), "Font family (the panel's Font list; Noto Sans is the note style's own)."),
        opt("bold", json!({ "type": "boolean" }), "Bold (not with an SHX font)."),
        opt("italic", json!({ "type": "boolean" }), "Italic (not with an SHX font)."),
        opt("underline", json!({ "type": "boolean" }), "Underline."),
        opt("strikethrough", json!({ "type": "boolean" }), "Strikethrough."),
        opt("color", json!({ "type": "string" }), "Color: hex RRGGBB (the Note color dialog; 000000 is the ink, no override)."),
        opt("line_spacing", json!({ "type": "number", "enum": SPACINGS }), "Line spacing."),
        opt("space_before", json!({ "type": "boolean" }), "Add (true) / Remove (false) space before paragraph."),
        opt("space_after", json!({ "type": "boolean" }), "Add (true) / Remove (false) space after paragraph."),
        opt("align", json!({ "type": "string", "enum": ALIGNS.map(|x| x.0) }), "Horizontal alignment (notes without leaders)."),
        opt("valign", json!({ "type": "string", "enum": VALIGNS.map(|x| x.0) }), "Vertical alignment (notes without leaders)."),
        opt("list", json!({ "type": "string", "enum": LISTS.map(|x| x.0) }), "List."),
        opt("order", json!({ "type": "string", "enum": ["top_to_bottom", "bottom_to_top"] }), "Note order."),
        opt("width", json!({ "type": ["number", "null"], "exclusiveMinimum": 0 }), "Bounding box width (mm; lines wrap at it); null: as wide as the text."),
        opt("angle", json!({ "type": "number" }), "Rotation (degrees, counter-clockwise, whole degrees as the rotate grip; notes without leaders)."),
        opt("flags", json!({ "type": "array", "items": { "type": "integer", "minimum": 0 } }), "Flag notes: the flagged list items (0 = first line); [] clears."),
        opt("flag_shape", json!({ "type": "string", "enum": shapes }), "Flag notes border shape (default: the drawing's)."),
        opt("flag_size", json!({ "type": "integer", "minimum": 0, "maximum": 5 }), "Flag notes border size in characters (0 = Tight fit)."),
    ]
}

/// The panel's options in `a` applied to `st`, as the panel's buttons and lists do; `text` is the note's text
/// (its lines are the list items) and `drawing_shape` the drawing's flag shape.
pub(super) fn apply(a: &Args, st: &mut NoteStyle, text: &str, drawing_shape: FlagShape) -> Result<(), OpError> {
    if let Some(f) = a.opt_str("font")? {
        let fam = FONT_FAMILIES.iter().find(|x| **x == f).ok_or_else(|| OpError::param("font", "not a font of the Font list"))?;
        let (b, i) = st.font.as_ref().map(|x| (x.bold, x.italic)).filter(|_| !is_shx(fam)).unwrap_or((false, false));
        st.font = if *fam == DRAWING_DEFAULT_FONT && !b && !i { None } else { Some(TextFont { family: (*fam).into(), bold: b, italic: i }) };
    }
    for (k, bold) in [("bold", true), ("italic", false)] {
        let Some(v) = a.get(k).and_then(Value::as_bool) else { continue };
        if st.font.as_ref().is_some_and(|f| is_shx(&f.family)) {
            return Err(OpError::param(k, "SHX fonts have no bold or italic"));
        }
        let f = st.font.get_or_insert_with(|| TextFont { family: DRAWING_DEFAULT_FONT.into(), bold: false, italic: false });
        if bold {
            f.bold = v;
        } else {
            f.italic = v;
        }
    }
    st.underline = a.bool_or("underline", st.underline)?;
    st.strikethrough = a.bool_or("strikethrough", st.strikethrough)?;
    if let Some(c) = a.opt_str("color")? {
        let hex = c.trim().trim_start_matches('#');
        let v = (hex.len() == 6).then(|| u32::from_str_radix(hex, 16).ok()).flatten().ok_or_else(|| OpError::param("color", "a hex color RRGGBB"))?;
        let rgb = [(v >> 16) as u8, (v >> 8) as u8, v as u8];
        st.color = (rgb != [0, 0, 0]).then_some(rgb);
    }
    if let Some(v) = a.opt_f64("line_spacing")? {
        st.line_spacing = *SPACINGS.iter().find(|x| (**x - v).abs() < 1e-9).ok_or_else(|| OpError::param("line_spacing", "one of 1.0, 1.5, 2.0, 2.5, 3.0"))?;
    }
    st.space_before = a.bool_or("space_before", st.space_before)?;
    st.space_after = a.bool_or("space_after", st.space_after)?;
    let free = st.leaders.is_empty();
    for k in ["align", "valign"] {
        if !free && a.get(k).is_some() {
            return Err(OpError::param(k, "a note with a leader has no alignment"));
        }
    }
    if let Some(deg) = a.opt_f64("angle")? {
        if !free {
            return Err(OpError::param("angle", "notes with leaders don't rotate"));
        }
        st.angle = deg.round().to_radians();
    }
    st.align = enum_arg(a, "align", &ALIGNS, st.align)?;
    st.valign = enum_arg(a, "valign", &VALIGNS, st.valign)?;
    st.list = enum_arg(a, "list", &LISTS, st.list)?;
    st.bottom_to_top = enum_arg(a, "order", &[("top_to_bottom", false), ("bottom_to_top", true)], st.bottom_to_top)?;
    match a.value.get("width") {
        None => {}
        Some(Value::Null) => st.width = None,
        Some(v) => st.width = Some(v.as_f64().filter(|w| *w > 0.0).ok_or_else(|| OpError::param("width", "a width > 0 (mm) or null"))?),
    }
    let shapes: Vec<(String, FlagShape)> = FlagShape::ALL.iter().map(|k| (shape_key(*k), *k)).collect();
    let shape = match a.opt_str("flag_shape")? {
        None => None,
        Some(s) => Some(shapes.iter().find(|x| x.0 == s).map(|x| x.1).ok_or_else(|| OpError::param("flag_shape", "not a border shape"))?),
    };
    if let Some(list) = a.opt_array("flags")? {
        let items = text.lines().count().max(1);
        let mut flags = Vec::new();
        for (i, v) in list.iter().enumerate() {
            let n = v.as_u64().map(|n| n as usize).ok_or_else(|| OpError::param(&format!("flags[{i}]"), "a list item (0 = first)"))?;
            if st.list == NoteList::None {
                return Err(OpError::param("flags", "only list items can be flagged: give a list"));
            }
            if n >= items {
                return Err(OpError::param(&format!("flags[{i}]"), &format!("the note has {items} list items")));
            }
            if !flags.contains(&n) {
                flags.push(n);
            }
        }
        if st.flags.is_empty() && !flags.is_empty() {
            st.flag_shape = drawing_shape;
        }
        st.flags = flags;
    }
    if let Some(k) = shape {
        st.flag_shape = k;
    }
    if let Some(n) = a.opt_u64("flag_size")? {
        st.flag_chars = u8::try_from(n).ok().filter(|n| *n <= 5).ok_or_else(|| OpError::param("flag_size", "0 (Tight fit) to 5 characters"))?;
    }
    Ok(())
}

/// A note's panel options for drawing_get: what differs from a new note's.
pub(super) fn style_json(st: &NoteStyle) -> Value {
    let d = NoteStyle::default();
    let mut j = json!({});
    if let Some(f) = &st.font {
        j["font"] = json!(f.family);
        if f.bold {
            j["bold"] = json!(true);
        }
        if f.italic {
            j["italic"] = json!(true);
        }
    }
    if st.underline {
        j["underline"] = json!(true);
    }
    if st.strikethrough {
        j["strikethrough"] = json!(true);
    }
    if let Some([r, g, b]) = st.color {
        j["color"] = json!(format!("{r:02X}{g:02X}{b:02X}"));
    }
    if (st.line_spacing - d.line_spacing).abs() > 1e-9 {
        j["line_spacing"] = json!(st.line_spacing);
    }
    if st.space_before {
        j["space_before"] = json!(true);
    }
    if st.space_after {
        j["space_after"] = json!(true);
    }
    if st.align != d.align {
        j["align"] = json!(key_of(&ALIGNS, st.align));
    }
    if st.valign != d.valign {
        j["valign"] = json!(key_of(&VALIGNS, st.valign));
    }
    if st.list != d.list {
        j["list"] = json!(key_of(&LISTS, st.list));
    }
    if st.bottom_to_top {
        j["order"] = json!("bottom_to_top");
    }
    if st.angle.abs() > 1e-12 {
        j["angle"] = json!(st.angle.to_degrees().round());
    }
    if let Some(w) = st.width {
        j["width"] = json!(w);
    }
    if !st.flags.is_empty() {
        j["flags"] = json!(st.flags);
        j["flag_shape"] = json!(shape_key(st.flag_shape));
        j["flag_size"] = json!(st.flag_chars);
    }
    j
}

/// The cell panel's options (drawing_table_cell).
pub(super) fn cell_fields() -> Vec<Field> {
    vec![
        opt("font", json!({ "type": "string", "enum": FONT_FAMILIES }), "Cell panel: Font."),
        opt("bold", json!({ "type": "boolean" }), "Cell panel: Bold (not with an SHX font)."),
        opt("italic", json!({ "type": "boolean" }), "Cell panel: Italic (not with an SHX font)."),
        opt("underline", json!({ "type": "boolean" }), "Cell panel: Underline."),
        opt("strikethrough", json!({ "type": "boolean" }), "Cell panel: Strikethrough."),
        opt("align", json!({ "type": "string", "enum": ["left", "center", "right"] }), "Cell panel: Horizontal alignment."),
        opt("valign", json!({ "type": "string", "enum": VALIGNS.map(|x| x.0) }), "Cell panel: Vertical alignment."),
        opt("height", json!({ "type": ["number", "null"] }), "Cell panel: Text height (mm); null: the table's."),
    ]
}

pub(super) const CELL_FIELDS: [&str; 8] = ["font", "bold", "italic", "underline", "strikethrough", "align", "valign", "height"];

const CELL_ALIGNS: [(&str, CellAlign); 3] = [("left", CellAlign::Left), ("center", CellAlign::Center), ("right", CellAlign::Right)];
const CELL_VALIGNS: [(&str, CellVAlign); 3] = [("top", CellVAlign::Top), ("middle", CellVAlign::Middle), ("bottom", CellVAlign::Bottom)];

/// The cell panel's options in `a` on a cell's shown format `f` (as the panel's apply, `table_height` the table's
/// text height).
pub(super) fn apply_cell(a: &Args, f: &mut CellFormat, table_height: f64) -> Result<(), OpError> {
    if let Some(v) = a.opt_str("font")? {
        let fam = FONT_FAMILIES.iter().find(|x| **x == v).ok_or_else(|| OpError::param("font", "not a font of the Font list"))?;
        if is_shx(fam) {
            (f.bold, f.italic) = (false, false);
        }
        f.font = (*fam != DRAWING_DEFAULT_FONT).then(|| (*fam).to_string());
    }
    for k in ["bold", "italic"] {
        let Some(v) = a.get(k).and_then(Value::as_bool) else { continue };
        if f.font.as_deref().is_some_and(is_shx) {
            return Err(OpError::param(k, "SHX fonts have no bold or italic"));
        }
        if k == "bold" {
            f.bold = v;
        } else {
            f.italic = v;
        }
    }
    f.underline = a.bool_or("underline", f.underline)?;
    f.strikethrough = a.bool_or("strikethrough", f.strikethrough)?;
    f.align = enum_arg(a, "align", &CELL_ALIGNS, f.align)?;
    f.valign = enum_arg(a, "valign", &CELL_VALIGNS, f.valign)?;
    match a.value.get("height") {
        None => {}
        Some(Value::Null) => f.height = None,
        Some(v) => {
            let h = v.as_f64().filter(|h| *h > 0.5).ok_or_else(|| OpError::param("height", "a text height in mm (over 0.5), or null"))?;
            f.height = ((h - table_height).abs() > 1e-6).then_some(h);
        }
    }
    Ok(())
}

pub(super) fn cell_format_json(row: usize, col: usize, f: &CellFormat) -> Value {
    let mut j = json!({ "row": row, "column": col, "align": key_of(&CELL_ALIGNS, f.align), "valign": key_of(&CELL_VALIGNS, f.valign) });
    for (k, on) in [("bold", f.bold), ("italic", f.italic), ("underline", f.underline), ("strikethrough", f.strikethrough)] {
        if on {
            j[k] = json!(true);
        }
    }
    if let Some(h) = f.height {
        j["height"] = json!(h);
    }
    if let Some(fam) = &f.font {
        j["font"] = json!(fam);
    }
    j
}
