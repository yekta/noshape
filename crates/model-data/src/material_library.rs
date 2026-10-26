//! Material Library tab ("+" › Create Material Library): a table of
//! materials loaded from a CSV file (tab › Update…) and written back as CSV
//! (tab › Export…), in the format of Onshape's exported std.mat library:
//! a header row `Category, Name, Density [kg/m^3], <more properties>…`, then
//! one material per row. Part Studios offer the document's libraries in
//! the Material dialog ([`libraries`]).

use crate::document::{Document, TabContent, TabId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MaterialLibrary {
    /// Headers of the columns after Category, Name and Density, as in the file.
    #[serde(default)]
    pub extra_columns: Vec<String>,
    pub materials: Vec<LibraryMaterial>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryMaterial {
    pub category: String,
    pub name: String,
    /// kg/m³.
    pub density: f64,
    /// Values of `extra_columns`, as text.
    #[serde(default)]
    pub extra: Vec<String>,
}

/// Density units of the Density column (header `Density [unit]`) and the
/// table's unit select: label, kg/m³ per unit.
pub const DENSITY_UNITS: [(&str, f64); 4] = [("kg/m³", 1.0), ("g/cm³", 1000.0), ("lb/in³", 27_679.904_710_191), ("lb/ft³", 16.018_463_373_960_14)];

fn density_unit(header: &str) -> f64 {
    let unit = header.split_once('[').map(|(_, u)| u.trim_end_matches(']').trim().to_lowercase()).unwrap_or_default();
    let unit = unit.replace("^3", "³").replace(' ', "");
    DENSITY_UNITS.iter().find(|(u, _)| u.to_lowercase() == unit).map(|(_, k)| *k).unwrap_or(1.0)
}

/// Split one CSV line (RFC 4180 quoting; no line breaks inside fields).
pub fn split_csv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

impl MaterialLibrary {
    /// Read a library CSV (UTF-8, optional BOM). The header must name
    /// Category, Name and Density columns (in any order).
    pub fn from_csv(text: &str) -> Result<Self, String> {
        let mut lines = text.trim_start_matches('\u{feff}').lines().filter(|l| !l.trim().is_empty());
        let header = split_csv(lines.next().ok_or("The file is empty")?);
        let find = |name: &str| header.iter().position(|h| h.to_lowercase().starts_with(name));
        let (ci, ni, di) = match (find("category"), find("name"), find("density")) {
            (Some(c), Some(n), Some(d)) => (c, n, d),
            _ => return Err("The first row must name the Category, Name and Density columns".into()),
        };
        let per_unit = density_unit(&header[di]);
        let extra_ix: Vec<usize> = (0..header.len()).filter(|i| ![ci, ni, di].contains(i)).collect();
        let mut materials = Vec::new();
        for (row, line) in lines.enumerate() {
            let f = split_csv(line);
            let get = |i: usize| f.get(i).cloned().unwrap_or_default();
            let density: f64 = get(di).parse().map_err(|_| format!("Row {}: \"{}\" is not a density", row + 2, get(di)))?;
            materials.push(LibraryMaterial { category: get(ci), name: get(ni), density: density * per_unit, extra: extra_ix.iter().map(|i| get(*i)).collect() });
        }
        Ok(Self { extra_columns: extra_ix.iter().map(|i| header[*i].clone()).collect(), materials })
    }

    /// The library as CSV (densities in kg/m^3).
    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        let header: Vec<String> = ["Category", "Name", "Density [kg/m^3]"].iter().map(|s| s.to_string()).chain(self.extra_columns.iter().cloned()).collect();
        out.push_str(&header.iter().map(|h| csv_field(h)).collect::<Vec<_>>().join(","));
        out.push('\n');
        for m in &self.materials {
            let mut row = vec![csv_field(&m.category), csv_field(&m.name), format!("{}", m.density)];
            row.extend(m.extra.iter().map(|e| csv_field(e)));
            out.push_str(&row.join(","));
            out.push('\n');
        }
        out
    }

    /// The sample offered by "Download sample" (a few of std.mat's rows).
    pub fn sample() -> Self {
        let m = |c: &str, n: &str, d: f64| LibraryMaterial { category: c.into(), name: n.into(), density: d, extra: Vec::new() };
        Self { extra_columns: Vec::new(), materials: vec![m("Ceramic", "Alumina Oxide", 3848.), m("Ceramic", "Brick", 1765.), m("Metal", "Aluminum - 6061", 2700.), m("Metal", "Steel", 7850.), m("Plastic", "ABS", 1020.)] }
    }
}

/// The document's material libraries: (tab, library name = tab name, library).
pub fn libraries(doc: &Document) -> Vec<(TabId, String, &MaterialLibrary)> {
    doc.ordered_tabs()
        .into_iter()
        .filter_map(|t| match &t.content {
            TabContent::MaterialLibrary(l) => Some((t.id, t.name.clone(), l)),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_onshape_csv_and_round_trips() {
        let csv = "\u{feff}Category,Name,Density [g/cm^3],Poisson's Ratio\nMetal,\"Steel, mild\",7.85,0.29\nPlastic,ABS,1.02,0.35\n";
        let lib = MaterialLibrary::from_csv(csv).unwrap();
        assert_eq!(lib.materials.len(), 2);
        assert_eq!(lib.materials[0].name, "Steel, mild");
        assert!((lib.materials[0].density - 7850.).abs() < 1e-9);
        assert_eq!(lib.extra_columns, vec!["Poisson's Ratio".to_string()]);
        let again = MaterialLibrary::from_csv(&lib.to_csv()).unwrap();
        assert_eq!(again, lib);
        assert!(MaterialLibrary::from_csv("A,B\n1,2").is_err());
        assert!(MaterialLibrary::from_csv("Category,Name,Density\nMetal,X,heavy").is_err());
    }
}
