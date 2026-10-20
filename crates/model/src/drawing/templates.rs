//! Drawing templates: standard ISO (A4..A0, millimeters, first angle) and
//! ANSI (A..E, inches, third angle) sheet setups with their title block
//! layouts, applied per sheet.

use super::{DimUnits, Drawing, DrawingProps, Projection, SheetFormat, SheetSize, TitleBlockStyle};

/// A sheet setup: size, title block and the drawing standard it implies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawingTemplate {
    pub name: &'static str,
    pub size: SheetSize,
    pub style: TitleBlockStyle,
    /// Dimensions in inches (ANSI) rather than millimeters.
    pub inches: bool,
    pub projection: Projection,
    /// Custom template › Units when not the standard's millimeters /
    /// inches (centimeters, meters, feet and inches, foot, yards).
    pub units: Option<DimUnits>,
    /// Custom template › Decimal separator Comma.
    pub decimal_comma: bool,
    /// Custom template › Border, zones and start zones.
    pub format: SheetFormat,
}

impl DrawingTemplate {
    /// An Onshape template of a standard size: ISO in millimeters, first
    /// angle; ANSI in inches, third angle.
    pub fn standard(name: &'static str, size: SheetSize, style: TitleBlockStyle, inches: bool, projection: Projection) -> Self {
        Self { name, size, style, inches, projection, units: None, decimal_comma: false, format: SheetFormat::default() }
    }

    /// The drawing's units.
    pub fn dim_units(&self) -> DimUnits {
        self.units.unwrap_or(if self.inches { DimUnits::Inch } else { DimUnits::Millimeter })
    }
}

/// Onshape-like standard templates: "ISO A3", ..., "ANSI B", ...
pub fn standard_templates() -> Vec<DrawingTemplate> {
    let iso = [("ISO A4", SheetSize::A4), ("ISO A3", SheetSize::A3), ("ISO A2", SheetSize::A2), ("ISO A1", SheetSize::A1), ("ISO A0", SheetSize::A0)];
    let ansi = [("ANSI A", SheetSize::AnsiA), ("ANSI B", SheetSize::AnsiB), ("ANSI C", SheetSize::AnsiC), ("ANSI D", SheetSize::AnsiD), ("ANSI E", SheetSize::AnsiE)];
    iso.into_iter()
        .map(|(name, size)| DrawingTemplate::standard(name, size, TitleBlockStyle::Iso, false, Projection::FirstAngle))
        .chain(ansi.into_iter().map(|(name, size)| DrawingTemplate::standard(name, size, TitleBlockStyle::Ansi, true, Projection::ThirdAngle)))
        .collect()
}

pub fn template_by_name(name: &str) -> Option<DrawingTemplate> {
    standard_templates().into_iter().find(|t| t.name == name)
}

/// The title block's units note ("DIMENSIONS ARE IN …").
pub fn units_note(inches: bool) -> &'static str {
    if inches { "INCHES" } else { "MILLIMETERS" }
}

/// The units note of any drawing unit.
pub fn units_note_of(u: DimUnits) -> &'static str {
    match u {
        DimUnits::Millimeter => "MILLIMETERS",
        DimUnits::Centimeter => "CENTIMETERS",
        DimUnits::Meter => "METERS",
        DimUnits::Inch | DimUnits::InchFraction => "INCHES",
        DimUnits::Foot => "FEET",
        DimUnits::Yard => "YARDS",
        DimUnits::FeetInches => "FEET AND INCHES",
    }
}

/// Title block fields of a style with default values (property fields
/// where the model supplies them).
pub fn default_fields(style: TitleBlockStyle, inches: bool) -> Vec<(String, String)> {
    let f: Vec<(&str, &str)> = match style {
        TitleBlockStyle::Simple => vec![("Title", ""), ("Drawn by", ""), ("Date", ""), ("Part number", ""), ("Revision", ""), ("Material", ""), ("Scale", ""), ("Sheet", "")],
        // Onshape's ANSI and ISO templates share one block (sheet.rs
        // `onshape_title_block`); the tolerance notes are fixed text.
        TitleBlockStyle::Iso | TitleBlockStyle::Ansi => vec![
            ("Drawn by", ""),
            ("Drawn date", "{date}"),
            ("Checked by", ""),
            ("Checked date", ""),
            ("Approved by", ""),
            ("Approved date", ""),
            ("Material", "{part.material}"),
            ("Finish", ""),
            ("Title", "{part.name}"),
            ("Size", ""),
            ("Part number", "{part.number}"),
            ("Revision", "-"),
            ("Scale", ""),
            ("Weight", ""),
            ("Sheet", "{sheet.number} of {sheet.count}"),
            ("Units", units_note(inches)),
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
/// Drawing properties › "Update properties from a template…": the
/// template's properties (its units and standard; everything else at the
/// defaults), remembering the template file.
pub fn template_props(t: &DrawingTemplate, file: &str) -> DrawingProps {
    DrawingProps { units: Some(t.dim_units()), decimal_comma: t.decimal_comma, template: Some(file.to_string()), ..DrawingProps::default() }
}

pub fn apply_template(drawing: &mut Drawing, sheet: usize, t: &DrawingTemplate) {
    if sheet >= drawing.sheets.len() {
        return;
    }
    let units = t.dim_units();
    drawing.units_mm = !units.imperial();
    if t.units.is_some() {
        drawing.props.units = Some(units);
    }
    if t.decimal_comma {
        drawing.props.decimal_comma = true;
    }
    drawing.projection = t.projection;
    drawing.sheets[sheet].size = t.size;
    drawing.sheets[sheet].format = t.format;
    set_title_style(drawing, sheet, t.style);
    // The units note follows the template.
    if let Some((_, v)) = drawing.sheets[sheet].title_block.fields.iter_mut().find(|(k, _)| k == "Units") {
        *v = units_note_of(units).to_string();
    }
}
