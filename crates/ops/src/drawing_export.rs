//! Drawing export (the Export Drawing modal), GUI-free and shared by the
//! app's modal (`crates/app/src/drawing_export.rs`) and the agent API: the
//! formats, versions and settings as the modal offers them (live
//! dom/dialog-export-drawing-all-formats) and the per-sheet writers for PDF,
//! DWG, DXF, DWT and SVG. Raster formats are each sheet's SVG, rasterized by
//! the caller; what only the app has (shaded views' pictures, the release
//! watermark) comes in [`ExportExtras`].

use noshape_model::drawing::build::{SheetInfo, render_sheet_with};
use noshape_model::drawing::cad_export::{CAD_VERSIONS, CadVersion};
use noshape_model::drawing::export::{self, ExportOptions};
use noshape_model::drawing::views::{Sources, ViewCache};
use noshape_model::drawing::{AnnotationKind, Drawing, ViewId};
use std::sync::Arc;

/// Export Drawing › Format, in Onshape's order.
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
    /// The format's Version list (empty: no Version select).
    pub fn versions(self) -> Vec<&'static str> {
        match self {
            DrawingFormat::Pdf => PDF_VERSIONS.to_vec(),
            DrawingFormat::Dwg | DrawingFormat::Dxf => CAD_VERSIONS.iter().map(|v| v.0).collect(),
            DrawingFormat::Dwt => DWT_VERSIONS.iter().map(|v| v.0).collect(),
            _ => Vec::new(),
        }
    }
}

pub const PDF_VERSIONS: [&str; 4] = ["1.4", "1.5", "1.6", "1.7"];
pub const DWT_VERSIONS: [(&str, CadVersion); 2] = [("2013", CadVersion::AC1027), ("2018", CadVersion::AC1032)];

/// The Version a format starts with (live: PDF 1.5, DWG 2000, DXF Release
/// 14, DWT 2013).
pub fn default_version(f: DrawingFormat) -> usize {
    match f {
        DrawingFormat::Pdf => 1,
        DrawingFormat::Dwg => noshape_model::drawing::cad_export::DEFAULT_DWG,
        DrawingFormat::Dxf => noshape_model::drawing::cad_export::DEFAULT_DXF,
        _ => 0,
    }
}
pub const SHEETS: [&str; 3] = ["All sheets", "Current sheet", "Custom sheets"];
pub const OVERRIDDEN: [&str; 2] = ["Show underlines", "Hide underlines"];
pub const PDF_TEXT: [&str; 2] = ["Normal", "Selectable"];
pub const DXF_TEXT: [&str; 3] = ["Export as text", "Explode into polylines (notes)", "Explode all into polylines"];
pub const COLOR: [&str; 3] = ["Color", "Black and white", "Grayscale"];
/// DXF / DWG Units: the drawing's, or another (INSUNITS code). Live
/// dialog-export-drawing-all-formats.
pub const UNITS: [(&str, u8); 7] = [("Use drawing units", 0), ("Centimeter", 5), ("Foot", 2), ("Inch", 1), ("Meter", 6), ("Millimeter", 4), ("Yard", 10)];
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
        Self { format: DrawingFormat::Pdf, version: default_version(DrawingFormat::Pdf), sheets: 0, custom: String::new(), overridden: 0, text: 0, color: 0, units: 0, splines: false, z_zero: true, hide_inspection: false, exclude_off_sheet: false, options: 0, print: false }
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
        // Live: "Selected format only supports exporting the active sheet."
        _ if s.format == DrawingFormat::Svg || s.format.raster() => vec![current.min(count.saturating_sub(1))],
        1 => vec![current],
        2 => parse_sheet_range(&s.custom, count),
        _ => (0..count).collect(),
    }
}

/// Several sheets go into a zip (one file per sheet); PDF and DWT hold every
/// sheet, DXF / DWG a layout per sheet (live drw2 l).
pub fn zipped(format: DrawingFormat, sheets: usize) -> bool {
    !matches!(format, DrawingFormat::Pdf | DrawingFormat::Dwt | DrawingFormat::Dxf | DrawingFormat::Dwg) && sheets > 1
}

/// A shaded view's picture on its sheet: view, corner, size (sheet mm), PNG.
pub type ShadedExport = (ViewId, [f64; 2], [f64; 2], Arc<Vec<u8>>);

/// A watermark's filled triangles for a sheet of the given size (mm).
pub type WatermarkFn = dyn Fn(f64, f64) -> Vec<(Vec<[f64; 2]>, [u8; 3], f32)> + Send + Sync;

/// What the app adds to an export: the shaded views' pictures (they replace
/// those views' edges on sheet `shaded_sheet` in PDF, SVG and the rasters)
/// and the release watermark (PDF and rasters).
#[derive(Default)]
pub struct ExportExtras {
    pub shaded: Vec<ShadedExport>,
    pub shaded_sheet: usize,
    pub watermark: Option<Box<WatermarkFn>>,
}

/// The files of an export: one (sheet name, bytes) per sheet, or a single
/// unnamed file (PDF, DWT, several-sheet DXF / DWG). Raster formats give
/// each sheet's SVG for the caller to rasterize; inspection lists are the
/// caller's.
pub fn export_files(drawing: &Drawing, sheets: &[usize], s: &ExportSettings, sources: &Sources, cache: &ViewCache, info: &SheetInfo, extras: &ExportExtras) -> Result<Vec<(String, Vec<u8>)>, String> {
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
    // DWG / DXF / DWT versions; DXF Release 11-12 is our R12 writer.
    let cad_version = match f {
        DrawingFormat::Dwt => Some(DWT_VERSIONS.get(s.version).unwrap_or(&DWT_VERSIONS[0]).1),
        DrawingFormat::Dxf if s.version == 0 => None,
        DrawingFormat::Dwg | DrawingFormat::Dxf => CAD_VERSIONS.get(s.version).map(|v| v.1),
        _ => None,
    };
    let insunits = match UNITS.get(s.units).map_or(0, |u| u.1) {
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
        let picture = f == DrawingFormat::Svg || f.raster();
        let opts = ExportOptions {
            // SVG and rasters: text as outlines (live: no <text> in the SVG).
            explode_all: (f == DrawingFormat::Pdf && s.text == 0) || (f.cad() && s.text == 2) || picture,
            strokes: f.cad(),
            explode,
            underline_overrides: s.overridden == 0,
            exclude_off_sheet: s.exclude_off_sheet,
            // Only PDF has a Color choice; SVG and rasters are all black (live).
            color_mode: if f == DrawingFormat::Pdf {
                s.color as u8
            } else if picture {
                1
            } else {
                0
            },
        };
        let pictures = i == extras.shaded_sheet && !extras.shaded.is_empty() && (f == DrawingFormat::Pdf || f == DrawingFormat::Svg || f.raster());
        // A shaded view's picture replaces its edges.
        let mut g = built.geometry.clone();
        if pictures {
            for (_, r) in g.view_edges.clone().iter().filter(|(v, _)| extras.shaded.iter().any(|x| x.0 == *v)) {
                for l in &mut g.lines[r.clone()] {
                    l.points.clear();
                }
            }
        }
        let mut geo = export::prepare(&g, &opts);
        if picture {
            picture_pens(&mut geo);
        }
        // Release management › Watermarks: over everything, whatever the
        // colour mode, on PDF and the raster formats; model spaces have none.
        if let Some(w) = extras.watermark.as_ref().filter(|_| (f == DrawingFormat::Pdf || f.raster()) && !sheet.imported.as_ref().is_some_and(|m| m.model)) {
            geo.overlays.extend(w(geo.width, geo.height));
        }
        if pictures {
            for (_, pos, size, png) in &extras.shaded {
                geo.images.insert(0, noshape_model::drawing::sheet::GeomImage { pos: *pos, size: *size, data_base64: noshape_model::sketch_feature::base64_encode(png).into(), rgb: None });
            }
        }
        // PDF and DWG / DXF (OLE2FRAME) embed decoded image pixels.
        if f == DrawingFormat::Pdf || f.cad() {
            for im in &mut geo.images {
                im.rgb = noshape_scene::png::decode_rgb(&im.data_base64);
            }
        }
        // DXF / DWG lay the sheets out by their content before exclusion.
        let range = noshape_model::drawing::cad_export::content_x_range(&built.geometry);
        pages.push((drawing.sheets[i].name.clone(), geo, range));
    }
    use noshape_model::drawing::cad_export as cad;
    let files: Vec<(String, Vec<u8>)> = match f {
        DrawingFormat::Pdf => vec![(String::new(), export::to_pdf_pages_version(&pages.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), PDF_VERSIONS.get(s.version).copied().unwrap_or(PDF_VERSIONS[1])))],
        // Several sheets: one file, a layout per sheet (live drw2 l).
        DrawingFormat::Dxf if pages.len() > 1 => vec![(String::new(), cad::to_dxf_sheets(&pages, insunits, cad_version.unwrap_or(CadVersion::AC1009))?)],
        DrawingFormat::Dwg if pages.len() > 1 => vec![(String::new(), cad::to_dwg_sheets(&pages, insunits, cad_version.unwrap_or(CAD_VERSIONS[3].1))?)],
        DrawingFormat::Dxf => match cad_version {
            Some(v) => pages.iter().map(|(n, g, _)| cad::to_dxf_version(g, insunits, v).map(|b| (n.clone(), b))).collect::<Result<Vec<_>, String>>()?,
            // Release 11-12: the view geometry only (live).
            None => pages.iter().map(|(n, g, _)| (n.clone(), cad::to_dxf_r12(g, insunits).into_bytes())).collect(),
        },
        DrawingFormat::Dwg => pages.iter().map(|(n, g, _)| cad::to_dwg_file(g, insunits, cad_version.unwrap_or(CAD_VERSIONS[3].1)).map(|b| (n.clone(), b))).collect::<Result<Vec<_>, String>>()?,
        // With the sheet formats, so Select a DWT gets them back exactly.
        DrawingFormat::Dwt => {
            let templates: Vec<noshape_model::drawing::dwt::SheetTemplate> = sheets.iter().filter_map(|i| drawing.sheets.get(*i)).map(noshape_model::drawing::dwt::SheetTemplate::of).collect();
            vec![(String::new(), noshape_model::drawing::dwt::to_dwt(&pages.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), &templates, insunits, cad_version.unwrap_or(DWT_VERSIONS[0].1))?)]
        }
        _ => pages.iter().map(|(n, g, _)| (n.clone(), export::to_svg(g).into_bytes())).collect(),
    };
    Ok(files)
}

/// SVG and raster line weights (live, at 300 px per inch): visible model
/// edges 5 px, every other line 2 px. Pens set in the drawing (Adjust
/// linestyle) still win.
fn picture_pens(g: &mut noshape_model::drawing::sheet::SheetGeometry) {
    use noshape_model::drawing::sheet::{LineStyle, PenSpan};
    let px = |n: f64| n * 25.4 / RASTER_PX_PER_INCH;
    let width = |st: LineStyle| if st == LineStyle::Visible { px(5.0) } else { px(2.0) };
    let mut spans: Vec<PenSpan> = Vec::new();
    for (i, l) in g.lines.iter().enumerate() {
        match spans.last_mut() {
            Some(sp) if sp.lines.end == i && g.lines[sp.lines.start].style == l.style => sp.lines.end = i + 1,
            _ => spans.push(PenSpan { centered: false, lines: i..i + 1, width_mm: width(l.style), dashes: l.style.dashes().to_vec() }),
        }
    }
    g.pens.splice(0..0, spans);
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

/// Raster exports: 300 pixels per inch of sheet (live: ANSI A is 3300 x
/// 2550), whatever their dpi tag says.
pub const RASTER_PX_PER_INCH: f64 = 300.0;

/// A sheet's exported SVG as PNG, JPEG or TIFF the way live writes them
/// (behaviours "api (10-24) drawing raster export"): 300 px per inch of
/// sheet, opaque white RGB without alpha, no antialiasing (two colours);
/// PNG with a pHYs of 3937 px/m (99.9998 dpi), JPEG quality 75 with 4:2:0
/// chroma and a 100 dpi JFIF density, TIFF uncompressed in one strip at
/// 100 dpi. The SVG is in millimetres, read at 96 dpi.
pub fn rasterize(svg: &[u8], format: DrawingFormat) -> Result<Vec<u8>, String> {
    use resvg::{tiny_skia, usvg};
    let opt = usvg::Options { shape_rendering: usvg::ShapeRendering::CrispEdges, text_rendering: usvg::TextRendering::OptimizeSpeed, ..usvg::Options::default() };
    let tree = usvg::Tree::from_data(svg, &opt).map_err(|e| e.to_string())?;
    let scale = (RASTER_PX_PER_INCH / 96.0) as f32;
    let size = tree.size();
    let (w, h) = ((size.width() * scale).round().max(1.0) as u32, (size.height() * scale).round().max(1.0) as u32);
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("bad image size")?;
    pixmap.fill(tiny_skia::Color::WHITE);
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    // Opaque over white, so the premultiplied colour is the colour.
    let rgb: Vec<u8> = pixmap.pixels().iter().flat_map(|p| [p.red(), p.green(), p.blue()]).collect();
    let mut out = std::io::Cursor::new(Vec::new());
    match format {
        DrawingFormat::Jpeg => {
            let (w16, h16) = (u16::try_from(w).map_err(|_| "image too large")?, u16::try_from(h).map_err(|_| "image too large")?);
            let mut e = jpeg_encoder::Encoder::new(&mut out, 75);
            e.set_sampling_factor(jpeg_encoder::SamplingFactor::R_4_2_0);
            e.set_density(jpeg_encoder::PixelDensity::dpi(100));
            e.encode(&rgb, w16, h16, jpeg_encoder::ColorType::Rgb).map_err(|e| e.to_string())?;
        }
        DrawingFormat::Tiff => {
            use tiff::encoder::{Rational, TiffEncoder, colortype::RGB8};
            let mut enc = TiffEncoder::new(&mut out).map_err(|e| e.to_string())?;
            let mut im = enc.new_image::<RGB8>(w, h).map_err(|e| e.to_string())?;
            im.resolution(tiff::tags::ResolutionUnit::Inch, Rational { n: 100, d: 1 });
            im.rows_per_strip(h).map_err(|e| e.to_string())?;
            im.write_data(&rgb).map_err(|e| e.to_string())?;
        }
        _ => {
            let mut e = png::Encoder::new(&mut out, w, h);
            e.set_color(png::ColorType::Rgb);
            e.set_depth(png::BitDepth::Eight);
            e.set_pixel_dims(Some(png::PixelDimensions { xppu: 3937, yppu: 3937, unit: png::Unit::Meter }));
            e.write_header().and_then(|mut wr| wr.write_image_data(&rgb)).map_err(|e| e.to_string())?;
        }
    }
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_versions_are_onshapes() {
        // qa5 D11 / live dialog-export-drawing-all-formats.
        assert_eq!(PDF_VERSIONS[default_version(DrawingFormat::Pdf)], "1.5");
        assert_eq!(CAD_VERSIONS[default_version(DrawingFormat::Dwg)].0, "2000");
        assert_eq!(CAD_VERSIONS[default_version(DrawingFormat::Dxf)].0, "Release 14");
        assert_eq!(DWT_VERSIONS[default_version(DrawingFormat::Dwt)].0, "2013");
        // qa5 D15: z-height to zero on, "Use drawing units".
        assert!(ExportSettings::default().z_zero);
        assert_eq!(UNITS[ExportSettings::default().units].0, "Use drawing units");
    }

    #[test]
    fn svg_and_rasters_export_the_active_sheet() {
        // Live: "Selected format only supports exporting the active sheet."
        for format in [DrawingFormat::Svg, DrawingFormat::Png, DrawingFormat::Jpeg, DrawingFormat::Tiff] {
            let s = ExportSettings { format, ..Default::default() };
            assert_eq!(export_sheets_of(&s, 3, 1), vec![1], "{format:?}");
        }
        assert_eq!(export_sheets_of(&ExportSettings::default(), 3, 1), vec![0, 1, 2], "PDF: every sheet");
        let dwt = ExportSettings { format: DrawingFormat::Dwt, ..Default::default() };
        assert_eq!(export_sheets_of(&dwt, 3, 2), vec![0, 1]);
    }

    #[test]
    fn custom_sheet_ranges() {
        assert_eq!(parse_sheet_range("1-3, 5", 6), vec![0, 1, 2, 4]);
        assert_eq!(parse_sheet_range("3-1 9", 4), vec![0, 1, 2]);
        assert!(parse_sheet_range("x", 3).is_empty());
    }
}
