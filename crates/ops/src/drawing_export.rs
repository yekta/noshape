//! Drawing export (the Export Drawing modal's writers), GUI-free: the
//! formats, the modal's settings and the per-sheet rendering to PDF, DWG,
//! DXF, DWT and SVG. Raster formats are the SVG rendered by the app (the
//! modal keeps that step); CSV inspection lists come from the drawing's
//! inspection panel. From `crates/app/src/drawing_export.rs`.

use noshape_model::drawing::build::{SheetInfo, render_sheet_with};
use noshape_model::drawing::export::{self, ExportOptions};
use noshape_model::drawing::views::{Sources, ViewCache};
use noshape_model::drawing::{AnnotationKind, Drawing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawingFormat {
    Pdf,
    Dwg,
    Dxf,
    Dwt,
    Svg,
    Png,
    Jpeg,
    Tiff,
    InspectionList,
    DefinitionList,
}

impl DrawingFormat {
    pub const ALL: [DrawingFormat; 10] = [DrawingFormat::Pdf, DrawingFormat::Dwg, DrawingFormat::Dxf, DrawingFormat::Dwt, DrawingFormat::Svg, DrawingFormat::Png, DrawingFormat::Jpeg, DrawingFormat::Tiff, DrawingFormat::InspectionList, DrawingFormat::DefinitionList];
    pub fn label(self) -> &'static str {
        match self {
            DrawingFormat::Pdf => "PDF",
            DrawingFormat::Dwg => "DWG",
            DrawingFormat::Dxf => "DXF",
            DrawingFormat::Dwt => "DWT",
            DrawingFormat::Svg => "SVG",
            DrawingFormat::Png => "PNG",
            DrawingFormat::Jpeg => "JPEG",
            DrawingFormat::Tiff => "TIFF",
            DrawingFormat::InspectionList => "Inspection list",
            DrawingFormat::DefinitionList => "Drawing definition list",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            DrawingFormat::Pdf => "pdf",
            DrawingFormat::Dwg => "dwg",
            DrawingFormat::Dxf => "dxf",
            DrawingFormat::Dwt => "dwt",
            DrawingFormat::Svg => "svg",
            DrawingFormat::Png => "png",
            DrawingFormat::Jpeg => "jpg",
            DrawingFormat::Tiff => "tif",
            DrawingFormat::InspectionList | DrawingFormat::DefinitionList => "csv",
        }
    }
    pub fn raster(self) -> bool {
        matches!(self, DrawingFormat::Png | DrawingFormat::Jpeg | DrawingFormat::Tiff)
    }
    /// DWG, DXF and DWT share Text, Units and the CAD checkboxes.
    pub fn cad(self) -> bool {
        matches!(self, DrawingFormat::Dwg | DrawingFormat::Dxf | DrawingFormat::Dwt)
    }
    pub fn csv(self) -> bool {
        matches!(self, DrawingFormat::InspectionList | DrawingFormat::DefinitionList)
    }
}

pub const PDF_VERSIONS: [&str; 4] = ["1.5", "1.6", "1.7", "1.4"];
pub const SHEETS: [&str; 3] = ["All sheets", "Current sheet", "Custom sheets"];
pub const OVERRIDDEN: [&str; 2] = ["Show underlines", "Hide underlines"];
pub const PDF_TEXT: [&str; 2] = ["Normal", "Selectable"];
pub const DXF_TEXT: [&str; 3] = ["Export as text", "Explode into polylines (notes)", "Explode all into polylines"];
pub const COLOR: [&str; 3] = ["Color", "Black and white", "Grayscale"];
/// DXF Units: the drawing's, or another (INSUNITS code).
pub const UNITS: [(&str, u8); 6] = [("Drawing units", 0), ("Millimeters", 4), ("Centimeters", 5), ("Meters", 6), ("Inches", 1), ("Feet", 2)];
pub const OPTIONS: [&str; 3] = ["Download", "Download and store…", "Store file in a new tab"];

/// The modal's settings.
#[derive(Clone, Debug)]
pub struct ExportSettings {
    pub format: DrawingFormat,
    pub version: usize,
    pub sheets: usize,
    pub custom: String,
    pub overridden: usize,
    pub text: usize,
    pub color: usize,
    pub units: usize,
    pub splines: bool,
    pub z_zero: bool,
    pub hide_inspection: bool,
    pub exclude_off_sheet: bool,
    /// Options: 0 Download, 1 Download and store, 2 Store file in a new tab.
    pub options: usize,
    /// Document menu › Print…: the PDF opens in the system viewer (Onshape
    /// opens a print-friendly page with print / save controls).
    pub print: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self { format: DrawingFormat::Pdf, version: 0, sheets: 0, custom: String::new(), overridden: 0, text: 0, color: 0, units: 0, splines: false, z_zero: false, hide_inspection: false, exclude_off_sheet: false, options: 0, print: false }
    }
}

/// "1-3, 5" → sheet indices (0-based, in range, sorted, unique).
pub fn parse_sheet_range(s: &str, count: usize) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for part in s.split([',', ';', ' ']).filter(|p| !p.is_empty()) {
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim().parse::<usize>().ok(), b.trim().parse::<usize>().ok()),
            None => {
                let v = part.trim().parse::<usize>().ok();
                (v, v)
            }
        };
        if let (Some(a), Some(b)) = (a, b) {
            out.extend((a.min(b)..=a.max(b)).filter(|&i| i >= 1 && i <= count).map(|i| i - 1));
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The sheets an export covers (the modal's Sheets choice; `current` is
/// the sheet shown).
pub fn export_sheets_of(s: &ExportSettings, count: usize, current: usize) -> Vec<usize> {
    match s.sheets {
        _ if s.format.csv() => vec![],
        // DWT: sheets 1 and 2 (the second is the continuation template).
        _ if s.format == DrawingFormat::Dwt => (0..count.min(2)).collect(),
        1 => vec![current],
        2 => parse_sheet_range(&s.custom, count),
        _ => (0..count).collect(),
    }
}

/// Several files go into a zip (one file per sheet), PDF / DWT hold all sheets.
pub fn zipped(format: DrawingFormat, sheets: usize) -> bool {
    !matches!(format, DrawingFormat::Pdf | DrawingFormat::Dwt) && sheets > 1
}

/// AutoCAD 2013 (DWT exports always use it).
pub fn acadrust_2013() -> noshape_model::drawing::cad_export::CadVersion {
    noshape_model::drawing::cad_export::VERSIONS[1].1
}

/// The vector files of an export: one (sheet name, bytes) per sheet, or a
/// single unnamed file for PDF / DWT (all sheets). Raster formats return
/// each sheet's SVG for the caller to rasterize.
pub fn export_files(drawing: &Drawing, sheets: &[usize], s: &ExportSettings, sources: &Sources, cache: &ViewCache, info: &SheetInfo) -> Result<Vec<(String, Vec<u8>)>, String> {
    let f = s.format;
    let mut drawing = drawing.clone();
    // Help H:drawing_properties_-_annotations: "all exported drawings
    // will, by default, no longer contain Drawing and Sheet Property
    // placeholders (----)".
    drawing.props.hide_placeholder = true;
    if s.hide_inspection {
        drawing.props.hide_inspection = true;
    }
    if f == DrawingFormat::Dwt {
        // A template keeps the formats only: no views, no Drawing layer
        // (annotations and tables).
        for sh in drawing.sheets.iter_mut() {
            sh.views.clear();
            sh.annotations.clear();
            sh.tables.clear();
        }
    }
    // DWG / DXF versions (DXF's last entry is our R12 writer); DWT is 2013.
    let versions = noshape_model::drawing::cad_export::VERSIONS;
    let cad_version = match f {
        DrawingFormat::Dwt => Some(acadrust_2013()),
        DrawingFormat::Dwg | DrawingFormat::Dxf => versions.get(s.version).map(|v| v.1),
        _ => None,
    };
    let insunits = match UNITS[s.units].1 {
        0 if drawing.units_mm => 4,
        0 => 1,
        u => u,
    };
    let mut pages = Vec::new();
    for &i in sheets {
        let built = render_sheet_with(&drawing, i, sources, cache, true, info).ok_or("nothing to export")?;
        // Explode notes: all notes' texts (Text › Explode into
        // polylines (notes)), else the notes marked Explode.
        let all_notes = f.cad() && s.text == 1;
        let sheet = &drawing.sheets[i];
        let explode: Vec<usize> = built
            .geometry
            .texts
            .iter()
            .enumerate()
            .filter(|(_, t)| matches!(built.picks.get(t.pick as usize), Some(noshape_model::drawing::build::PickTarget::Annotation(id)) if sheet.annotations.iter().any(|a| a.id == *id && matches!(&a.kind, AnnotationKind::Note { style, .. } if all_notes || style.explode))))
            .map(|(k, _)| k)
            .collect();
        let opts = ExportOptions {
            explode_all: (f == DrawingFormat::Pdf && s.text == 0) || (f.cad() && s.text == 2) || f.raster(),
            strokes: f.cad(),
            explode,
            underline_overrides: s.overridden == 0,
            exclude_off_sheet: s.exclude_off_sheet,
            color_mode: if f == DrawingFormat::Pdf || f.raster() { s.color as u8 } else { 0 },
        };
        let mut geo = export::prepare(&built.geometry, &opts);
        // PDF embeds decoded image pixels.
        if f == DrawingFormat::Pdf {
            for im in &mut geo.images {
                im.rgb = noshape_scene::png::decode_rgb(&im.data_base64);
            }
        }
        pages.push((drawing.sheets[i].name.clone(), geo));
    }
    let files: Vec<(String, Vec<u8>)> = match f {
        DrawingFormat::Pdf => vec![(String::new(), export::to_pdf_pages_version(&pages.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), PDF_VERSIONS[s.version]))],
        DrawingFormat::Dxf => match cad_version {
            Some(v) => pages.iter().map(|(n, g)| noshape_model::drawing::cad_export::to_dxf_version(g, insunits, v).map(|b| (n.clone(), b))).collect::<Result<Vec<_>, String>>()?,
            None => pages.iter().map(|(n, g)| (n.clone(), export::to_dxf_units(g, insunits).into_bytes())).collect(),
        },
        DrawingFormat::Dwg => pages.iter().map(|(n, g)| noshape_model::drawing::cad_export::to_dwg(g, insunits, cad_version.unwrap_or(versions[0].1)).map(|b| (n.clone(), b))).collect::<Result<Vec<_>, String>>()?,
        // With the sheet formats, so Select a DWT gets them back exactly.
        DrawingFormat::Dwt => {
            let templates: Vec<noshape_model::drawing::dwt::SheetTemplate> = sheets.iter().filter_map(|i| drawing.sheets.get(*i)).map(noshape_model::drawing::dwt::SheetTemplate::of).collect();
            vec![(String::new(), noshape_model::drawing::dwt::to_dwt(&pages.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), &templates, insunits, acadrust_2013())?)]
        }
        _ => pages.iter().map(|(n, g)| (n.clone(), export::to_svg(g).into_bytes())).collect(),
    };
    Ok(files)
}

/// One file of an export: the only file as it is, several in a zip named
/// "<name> - <sheet>.<ext>".
pub fn bundle(files: Vec<(String, Vec<u8>)>, name: &str, format: DrawingFormat) -> Vec<u8> {
    if files.len() == 1 {
        files.into_iter().next().map(|f| f.1).unwrap_or_default()
    } else {
        let named: Vec<(String, Vec<u8>)> = files.into_iter().map(|(sheet, b)| (format!("{name} - {sheet}.{}", format.ext()), b)).collect();
        export::zip_store(&named)
    }
}

#[cfg(test)]
mod tests {
    use super::parse_sheet_range;

    #[test]
    fn custom_sheet_ranges() {
        assert_eq!(parse_sheet_range("1-3, 5", 6), vec![0, 1, 2, 4]);
        assert_eq!(parse_sheet_range("3-1 9", 4), vec![0, 1, 2]);
        assert!(parse_sheet_range("x", 3).is_empty());
    }
}
