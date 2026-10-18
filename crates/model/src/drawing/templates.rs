//! Drawing templates: standard ISO (A4..A0, millimeters, first angle) and
//! ANSI (A..E, inches, third angle) sheet setups with their title block
//! layouts, applied per sheet.

use super::{Drawing, Projection, SheetSize, TitleBlockStyle};

/// A sheet setup: size, title block and the drawing standard it implies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawingTemplate {
    pub name: &'static str,
    pub size: SheetSize,
    pub style: TitleBlockStyle,
    /// Dimensions in inches (ANSI) rather than millimeters.
    pub inches: bool,
    pub projection: Projection,
}

/// Onshape-like standard templates: "ISO A3", ..., "ANSI B", ...
pub fn standard_templates() -> Vec<DrawingTemplate> {
    let iso = [("ISO A4", SheetSize::A4), ("ISO A3", SheetSize::A3), ("ISO A2", SheetSize::A2), ("ISO A1", SheetSize::A1), ("ISO A0", SheetSize::A0)];
    let ansi = [("ANSI A", SheetSize::AnsiA), ("ANSI B", SheetSize::AnsiB), ("ANSI C", SheetSize::AnsiC), ("ANSI D", SheetSize::AnsiD), ("ANSI E", SheetSize::AnsiE)];
    iso.into_iter()
        .map(|(name, size)| DrawingTemplate { name, size, style: TitleBlockStyle::Iso, inches: false, projection: Projection::FirstAngle })
        .chain(ansi.into_iter().map(|(name, size)| DrawingTemplate { name, size, style: TitleBlockStyle::Ansi, inches: true, projection: Projection::ThirdAngle }))
        .collect()
}

pub fn template_by_name(name: &str) -> Option<DrawingTemplate> {
    standard_templates().into_iter().find(|t| t.name == name)
}

/// Tolerance block text of ANSI title blocks.
pub fn ansi_tolerances(inches: bool) -> &'static str {
    if inches {
        "UNLESS OTHERWISE SPECIFIED:\nDIMENSIONS ARE IN INCHES\nTOLERANCES:\nFRACTIONAL ±1/32\nANGULAR ±0.5°\nTWO PLACE DECIMAL ±.01\nTHREE PLACE DECIMAL ±.005"
    } else {
        "UNLESS OTHERWISE SPECIFIED:\nDIMENSIONS ARE IN MILLIMETERS\nTOLERANCES:\nLINEAR ±0.1\nANGULAR ±0.5°"
    }
}

/// Title block fields of a style with default values (property fields
/// where the model supplies them).
pub fn default_fields(style: TitleBlockStyle, inches: bool) -> Vec<(String, String)> {
    let f: Vec<(&str, &str)> = match style {
        TitleBlockStyle::Simple => vec![("Title", ""), ("Drawn by", ""), ("Date", ""), ("Part number", ""), ("Revision", ""), ("Material", ""), ("Scale", ""), ("Sheet", "")],
        TitleBlockStyle::Iso => vec![
            ("Legal owner", ""),
            ("Title", "{part.name}"),
            ("Supplementary title", "{model.name}"),
            ("Document type", "Part drawing"),
            ("Document status", "Released"),
            ("Date", "{date}"),
            ("Drawn by", ""),
            ("Approved by", ""),
            ("Material", "{part.material}"),
            ("Scale", ""),
            ("Part number", "{part.number}"),
            ("Revision", "A"),
            ("Sheet", "{sheet.number}/{sheet.count}"),
        ],
        TitleBlockStyle::Ansi => vec![
            ("Company", ""),
            ("Title", "{part.name}"),
            ("Size", ""),
            ("CAGE code", ""),
            ("Part number", "{part.number}"),
            ("Revision", "A"),
            ("Scale", ""),
            ("Weight", ""),
            ("Sheet", "{sheet.number} OF {sheet.count}"),
            ("Drawn by", ""),
            ("Drawn date", "{date}"),
            ("Checked by", ""),
            ("Checked date", ""),
            ("Eng approval", ""),
            ("Eng date", ""),
            ("Mfg approval", ""),
            ("Mfg date", ""),
            ("Tolerances", ansi_tolerances(inches)),
            ("Material", "{part.material}"),
            ("Finish", ""),
        ],
        TitleBlockStyle::None => vec![],
    };
    f.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// Switch a sheet's title block layout, keeping the values of fields that
/// exist in both layouts (non-empty old values win over defaults).
pub fn set_title_style(drawing: &mut Drawing, sheet: usize, style: TitleBlockStyle) {
    let inches = !drawing.units_mm;
    let Some(s) = drawing.sheets.get_mut(sheet) else { return };
    let old = std::mem::take(&mut s.title_block.fields);
    let mut fields = default_fields(style, inches);
    for (k, v) in &mut fields {
        if let Some((_, ov)) = old.iter().find(|(ok, ov)| ok == k && !ov.is_empty()) {
            *v = ov.clone();
        }
    }
    s.title_block.fields = fields;
    s.title_style = style;
}

/// Apply a template to sheet `sheet`: size and title block. The drawing's
/// units and projection angle follow the template's standard (they are
/// drawing-wide).
pub fn apply_template(drawing: &mut Drawing, sheet: usize, t: &DrawingTemplate) {
    if sheet >= drawing.sheets.len() {
        return;
    }
    drawing.units_mm = !t.inches;
    drawing.projection = t.projection;
    drawing.sheets[sheet].size = t.size;
    set_title_style(drawing, sheet, t.style);
    // The tolerance note follows the units.
    if let Some((_, v)) = drawing.sheets[sheet].title_block.fields.iter_mut().find(|(k, _)| k == "Tolerances")
        && (v == ansi_tolerances(true) || v == ansi_tolerances(false))
    {
        *v = ansi_tolerances(t.inches).to_string();
    }
}
