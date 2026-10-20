//! Configured properties (Help PartStudio__configured_properties; live psb
//! 10-20 q, part-studio/600–656): per part a sparse table. Adding the first
//! property creates one row, the configuration active at that moment; input
//! columns come first (excluded inputs left out), then the properties in a
//! fixed order. A row applies to a configuration when every non-excluded
//! input has the row's value, so a row made before an input was added shows
//! that input's default but no longer applies.

use crate::configuration::{ConfigInputKind, ConfigValues, Configuration};
use crate::partstudio::{PartProperties, PartStudio};
use crate::reference::BodyId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Add property ▾, in the order the columns always take (live 601).
pub const PROPERTIES: [&str; 17] = [
    "Appearance",
    "Name",
    "Description",
    "Part number",
    "Revision",
    "Vendor",
    "Project",
    "Product line",
    "Material",
    "Title 1",
    "Title 2",
    "Title 3",
    "Not revision managed",
    "Exclude from all BOMs",
    "Unit of measure",
    "Mass",
    "Tessellation quality",
];

/// Unit of measure choices (live 636: "No value" first).
pub const UNITS_OF_MEASURE: [&str; 18] = [
    "No value", "Centimeter", "Foot", "Inch", "Meter", "Millimeter", "Yard", "Gram", "Kilogram", "Ounce", "Pound", "Liter", "Gallon", "Each", "Fluid ounce", "Milliliter", "Centiliter", "Package",
];

/// Tessellation quality choices (live 636).
pub const TESSELLATION: [&str; 5] = ["Auto", "Coarse", "Medium", "Fine", "Very fine"];

/// How a property's cell is edited (live 10-20 q table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    Text,
    Check,
    Select(&'static [&'static str]),
    Appearance,
    Material,
    /// A mass; empty reads "Computed".
    Mass,
}

pub fn cell_kind(prop: &str) -> CellKind {
    match prop {
        "Appearance" => CellKind::Appearance,
        "Material" => CellKind::Material,
        "Not revision managed" | "Exclude from all BOMs" => CellKind::Check,
        "Unit of measure" => CellKind::Select(&UNITS_OF_MEASURE),
        "Tessellation quality" => CellKind::Select(&TESSELLATION),
        "Mass" => CellKind::Mass,
        _ => CellKind::Text,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PropertyRow {
    /// The configuration the row is for: input id → value, non-excluded
    /// inputs as they were when the row was made.
    pub config: ConfigValues,
    /// Property → value as typed ("true"/"false" for checkboxes).
    pub values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropertyTable {
    pub part: BodyId,
    pub properties: Vec<String>,
    pub rows: Vec<PropertyRow>,
}

impl PropertyTable {
    /// The property columns in their fixed order.
    pub fn columns(&self) -> Vec<&'static str> {
        PROPERTIES.iter().copied().filter(|p| self.properties.iter().any(|q| q == p)).collect()
    }
}

/// The value an input has in a row: the row's, else the input's default.
pub fn row_input_value(cfg: &Configuration, row: &PropertyRow, input: &str) -> String {
    if let Some(v) = row.config.0.get(input) {
        return v.clone();
    }
    cfg.inputs.iter().find(|i| i.id == input).map(input_default).unwrap_or_default()
}

fn input_default(i: &crate::configuration::ConfigInput) -> String {
    match &i.kind {
        ConfigInputKind::List { default, .. } => default.clone(),
        ConfigInputKind::Checkbox { default } => default.to_string(),
        ConfigInputKind::Variable { default, .. } => default.text.clone(),
        ConfigInputKind::Text { default } => default.clone(),
    }
}

/// The inputs that are table columns (Exclude from properties leaves one
/// out), in input order.
pub fn input_columns(cfg: &Configuration) -> Vec<&crate::configuration::ConfigInput> {
    cfg.inputs.iter().filter(|i| !i.excluded).collect()
}

/// The part's own (non-configured) value of a property, as a cell shows it.
pub fn base_value(props: Option<&PartProperties>, prop: &str) -> String {
    let Some(p) = props else { return String::new() };
    let s = |o: &Option<String>| o.clone().unwrap_or_default();
    match prop {
        "Name" => s(&p.name),
        "Description" => s(&p.description),
        "Part number" => s(&p.part_number),
        "Revision" => s(&p.revision),
        "Vendor" => s(&p.meta.vendor),
        "Project" => s(&p.meta.project),
        "Product line" => s(&p.meta.product_line),
        "Material" => s(&p.material),
        _ => String::new(),
    }
}

pub fn table<'a>(ps: &'a PartStudio, part: &BodyId) -> Option<&'a PropertyTable> {
    ps.property_tables.iter().find(|t| &t.part == part)
}

fn table_mut<'a>(ps: &'a mut PartStudio, part: &BodyId) -> Option<&'a mut PropertyTable> {
    ps.property_tables.iter_mut().find(|t| &t.part == part)
}

/// Add property ▾: the first property creates the table with one row for
/// the active configuration; every property starts from the part's own
/// value.
pub fn add_property(ps: &mut PartStudio, part: &BodyId, prop: &str, active: &ConfigValues, part_name: &str) {
    let mut base = base_value(ps.part_properties.get(part), prop);
    if prop == "Name" && base.is_empty() {
        base = part_name.to_string();
    }
    if table(ps, part).is_none() {
        let (_, effective) = ps.configuration.visibility(active);
        let config = ConfigValues(input_columns(&ps.configuration).iter().map(|i| (i.id.clone(), effective.0.get(&i.id).cloned().unwrap_or_else(|| input_default(i)))).collect());
        ps.property_tables.push(PropertyTable { part: part.clone(), properties: Vec::new(), rows: vec![PropertyRow { config, values: BTreeMap::new() }] });
    }
    let Some(t) = table_mut(ps, part) else { return };
    if t.properties.iter().any(|p| p == prop) {
        return;
    }
    t.properties.push(prop.to_string());
    for r in &mut t.rows {
        r.values.insert(prop.to_string(), base.clone());
    }
}

/// Right-click › Delete column.
pub fn delete_column(ps: &mut PartStudio, part: &BodyId, prop: &str) {
    if let Some(t) = table_mut(ps, part) {
        t.properties.retain(|p| p != prop);
        for r in &mut t.rows {
            r.values.remove(prop);
        }
    }
}

/// ⋯ › Delete table for <part> (no confirmation).
pub fn delete_table(ps: &mut PartStudio, part: &BodyId) {
    ps.property_tables.retain(|t| &t.part != part);
}

pub fn set_cell(ps: &mut PartStudio, part: &BodyId, row: usize, prop: &str, value: &str) {
    if let Some(r) = table_mut(ps, part).and_then(|t| t.rows.get_mut(row)) {
        r.values.insert(prop.to_string(), value.to_string());
    }
}

/// Checkbox, variable and text input cells are editable in a row; list
/// inputs are not (live 10-20 q).
pub fn set_input_cell(ps: &mut PartStudio, part: &BodyId, row: usize, input: &str, value: &str) {
    if let Some(r) = table_mut(ps, part).and_then(|t| t.rows.get_mut(row)) {
        r.config.0.insert(input.to_string(), value.to_string());
    }
}

/// Set `prop` of `part` for configuration `config`, adding that row when
/// the table has none for it (live 626–627: editing the property in an
/// assembly BOM for an instance in that configuration adds the row). Does
/// nothing when the part has no table or the table lacks `prop`. Returns
/// whether it set the value.
pub fn set_for_configuration(ps: &mut PartStudio, part: &BodyId, prop: &str, config: &ConfigValues, value: &str) -> bool {
    let (_, effective) = ps.configuration.visibility(config);
    let cols: Vec<(String, String)> = input_columns(&ps.configuration).iter().map(|i| (i.id.clone(), effective.0.get(&i.id).cloned().unwrap_or_else(|| input_default(i)))).collect();
    let row = table(ps, part).and_then(|t| t.properties.iter().any(|p| p == prop).then(|| t.rows.iter().position(|r| cols.iter().all(|(id, v)| r.config.0.get(id) == Some(v)))));
    let Some(row) = row else { return false };
    let Some(t) = table_mut(ps, part) else { return false };
    let i = match row {
        Some(i) => i,
        None => {
            // New rows start from the first row's values (the part's own
            // values when the table was made).
            let values = t.rows.first().map(|r| r.values.clone()).unwrap_or_default();
            t.rows.push(PropertyRow { config: ConfigValues(cols.into_iter().collect()), values });
            t.rows.len() - 1
        }
    };
    t.rows[i].values.insert(prop.to_string(), value.to_string());
    true
}

/// The row that applies to configuration `active`.
pub fn applying_row<'a>(cfg: &Configuration, t: &'a PropertyTable, active: &ConfigValues) -> Option<&'a PropertyRow> {
    let (_, effective) = cfg.visibility(active);
    let cols = input_columns(cfg);
    t.rows.iter().find(|r| {
        cols.iter().all(|i| {
            let want = effective.0.get(&i.id).cloned().unwrap_or_else(|| input_default(i));
            r.config.0.get(&i.id) == Some(&want)
        })
    })
}

/// The configured value of `prop` for `part` in configuration `active`.
pub fn configured_value(ps: &PartStudio, part: &BodyId, prop: &str, active: &ConfigValues) -> Option<String> {
    let t = table(ps, part)?;
    if !t.properties.iter().any(|p| p == prop) {
        return None;
    }
    applying_row(&ps.configuration, t, active)?.values.get(prop).cloned()
}

/// ⋯ › Copy table: tab-separated, headers first (live: `Size\tShape\tPart
/// number\nSmall\tA\tPN-1`).
pub fn copy_table(ps: &PartStudio, part: &BodyId) -> String {
    let Some(t) = table(ps, part) else { return String::new() };
    let cfg = &ps.configuration;
    let inputs = input_columns(cfg);
    let cols = t.columns();
    let mut lines = vec![inputs.iter().map(|i| i.name.clone()).chain(cols.iter().map(|c| c.to_string())).collect::<Vec<_>>().join("\t")];
    for r in &t.rows {
        let cells = inputs.iter().map(|i| {
            let v = row_input_value(cfg, r, &i.id);
            // List cells show the option name.
            match &i.kind {
                ConfigInputKind::List { options, .. } => options.iter().find(|o| o.id == v).map(|o| o.name.clone()).unwrap_or(v),
                _ => v,
            }
        });
        lines.push(cells.chain(cols.iter().map(|c| r.values.get(*c).cloned().unwrap_or_default())).collect::<Vec<_>>().join("\t"));
    }
    lines.join("\n")
}

/// An Appearance cell's text: `#RRGGBB` (sRGB), with a 2-digit alpha when
/// not opaque.
pub fn color_text(rgb: [u8; 3], alpha: f32) -> String {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    if a == 255 { format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]) } else { format!("#{:02X}{:02X}{:02X}{a:02X}", rgb[0], rgb[1], rgb[2]) }
}

/// An Appearance cell's color as linear RGBA.
pub fn parse_color(text: &str) -> Option<[f32; 4]> {
    let h = text.trim().strip_prefix('#')?;
    if !(h.len() == 6 || h.len() == 8) || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let a = if h.len() == 8 { byte(6)? as f32 / 255.0 } else { 1.0 };
    Some([lin(byte(0)?), lin(byte(2)?), lin(byte(4)?), a])
}

/// Regeneration: a configured Name renames the part in the Parts list and
/// a configured Appearance colors it, for configurations that have a row
/// (live 644, 647).
pub fn apply_names(display: &mut crate::display::StudioDisplay, ps: &PartStudio, active: &ConfigValues) {
    for b in &mut display.bodies {
        if let Some(n) = configured_value(ps, &b.id, "Name", active).filter(|n| !n.trim().is_empty()) {
            b.name = n;
        }
        if let Some(c) = configured_value(ps, &b.id, "Appearance", active).and_then(|v| parse_color(&v)) {
            b.color = c;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration::{ConfigInput, ListOption};

    fn list(id: &str, opts: &[&str]) -> ConfigInput {
        ConfigInput {
            id: id.into(),
            name: id.into(),
            kind: ConfigInputKind::List { options: opts.iter().map(|o| ListOption { id: o.to_string(), name: o.to_string() }).collect(), default: opts[0].into() },
            visible_when: None,
            excluded: false,
        }
    }

    fn cv(pairs: &[(&str, &str)]) -> ConfigValues {
        ConfigValues(pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect())
    }

    #[test]
    fn sparse_rows_columns_and_names() {
        let mut ps = PartStudio::default();
        ps.configuration.inputs = vec![list("Size", &["Small", "Large"]), list("Shape", &["A", "B"])];
        let part = BodyId { feature: crate::reference::FeatureId(1), index: 0 };
        ps.part_properties.entry(part.clone()).or_default().part_number = Some("PN-L".into());
        // The first property: one row, the active configuration, prefilled.
        add_property(&mut ps, &part, "Part number", &cv(&[("Size", "Large")]), "Part 1");
        add_property(&mut ps, &part, "Description", &cv(&[("Size", "Large")]), "Part 1");
        let t = table(&ps, &part).unwrap();
        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.rows[0].config, cv(&[("Size", "Large"), ("Shape", "A")]));
        assert_eq!(t.rows[0].values["Part number"], "PN-L");
        // Columns in the fixed order, not the order added.
        assert_eq!(t.columns(), ["Description", "Part number"]);
        assert_eq!(copy_table(&ps, &part), "Size\tShape\tDescription\tPart number\nLarge\tA\t\tPN-L");
        // The row applies only to its configuration.
        set_cell(&mut ps, &part, 0, "Part number", "PN-LA");
        assert_eq!(configured_value(&ps, &part, "Part number", &cv(&[("Size", "Large")])).as_deref(), Some("PN-LA"));
        assert_eq!(configured_value(&ps, &part, "Part number", &cv(&[("Size", "Small")])), None);
        // A Name row renames the part in that configuration.
        add_property(&mut ps, &part, "Name", &cv(&[("Size", "Large")]), "Part 1");
        assert_eq!(table(&ps, &part).unwrap().rows[0].values["Name"], "Part 1");
        set_cell(&mut ps, &part, 0, "Name", "Block LA");
        assert_eq!(configured_value(&ps, &part, "Name", &cv(&[("Size", "Large")])).as_deref(), Some("Block LA"));
        // An input added later shows its default but the row stops applying.
        ps.configuration.inputs.push(list("Finish", &["Matte", "Gloss"]));
        assert_eq!(row_input_value(&ps.configuration, &table(&ps, &part).unwrap().rows[0], "Finish"), "Matte");
        assert_eq!(configured_value(&ps, &part, "Part number", &cv(&[("Size", "Large")])), None);
        // Excluding it from properties makes the row apply again.
        ps.configuration.inputs[2].excluded = true;
        assert_eq!(configured_value(&ps, &part, "Part number", &cv(&[("Size", "Large")])).as_deref(), Some("PN-LA"));
        // A BOM edit for another configuration adds its row.
        assert!(set_for_configuration(&mut ps, &part, "Part number", &cv(&[("Size", "Small"), ("Shape", "B")]), "PN-SB"));
        assert_eq!(table(&ps, &part).unwrap().rows.len(), 2);
        assert_eq!(configured_value(&ps, &part, "Part number", &cv(&[("Size", "Small"), ("Shape", "B")])).as_deref(), Some("PN-SB"));
        assert!(!set_for_configuration(&mut ps, &part, "Vendor", &cv(&[("Size", "Small")]), "x"));
        // Appearance cells hold "#RRGGBB[AA]".
        assert_eq!(color_text([157, 207, 237], 1.0), "#9DCFED");
        assert_eq!(color_text([255, 0, 0], 0.5), "#FF000080");
        let c = parse_color("#FF000080").unwrap();
        assert!((c[0] - 1.0).abs() < 1e-6 && c[1] == 0.0 && (c[3] - 128.0 / 255.0).abs() < 1e-6);
        assert!(parse_color("Default").is_none());
        delete_column(&mut ps, &part, "Part number");
        assert_eq!(table(&ps, &part).unwrap().columns(), ["Name", "Description"]);
        delete_table(&mut ps, &part);
        assert!(table(&ps, &part).is_none());
    }
}
