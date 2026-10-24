//! Drawing editor: Onshape's "Export Drawing" modal (drawing tab
//! right-click › Export…; live drawing/40, dom/dialog-export-drawing.txt).
//! The fields depend on the format: File name, Format, Version, Sheets
//! (several sheets), Overridden dimensions, Text, Color, Units, Options,
//! Export splines as polylines, Set z-height to zero…, Hide inspection
//! items, Exclude off-sheet content. Several sheets of a single-page format
//! download as one zip; DXF / DWG write them into one file, a layout per
//! sheet (live drw2 l).

use crate::drawing_view::*;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::*;
use noshape_model::drawing::build::render_sheet_with;
use noshape_model::drawing::cad_export::CadVersion;
use noshape_model::drawing::export::{self, ExportOptions};
use noshape_model::drawing::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

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
    fn raster(self) -> bool {
        matches!(self, DrawingFormat::Png | DrawingFormat::Jpeg | DrawingFormat::Tiff)
    }
    /// DWG, DXF and DWT share Text, Units and the CAD checkboxes.
    fn cad(self) -> bool {
        matches!(self, DrawingFormat::Dwg | DrawingFormat::Dxf | DrawingFormat::Dwt)
    }
    fn csv(self) -> bool {
        matches!(self, DrawingFormat::InspectionList | DrawingFormat::DefinitionList)
    }
}

const PDF_VERSIONS: [&str; 4] = ["1.4", "1.5", "1.6", "1.7"];
/// Live dom/dialog-export-drawing-all-formats: DWG / DXF versions (DXF
/// Release 11-12 = our R12 writer), and DWT's.
const CAD_VERSIONS: [(&str, CadVersion); 9] = [
    ("Release 11-12", CadVersion::AC1009),
    ("Release 13", CadVersion::AC1012),
    ("Release 14", CadVersion::AC1014),
    ("2000", CadVersion::AC1015),
    ("2004", CadVersion::AC1018),
    ("2007", CadVersion::AC1021),
    ("2010", CadVersion::AC1024),
    ("2013", CadVersion::AC1027),
    ("2018", CadVersion::AC1032),
];
const DWT_VERSIONS: [(&str, CadVersion); 2] = [("2013", CadVersion::AC1027), ("2018", CadVersion::AC1032)];

/// The Version a format starts with (live: PDF 1.5, DWG 2000, DXF Release
/// 14, DWT 2013).
fn default_version(f: DrawingFormat) -> usize {
    match f {
        DrawingFormat::Pdf => 1,
        DrawingFormat::Dwg => 3,
        DrawingFormat::Dxf => 2,
        _ => 0,
    }
}
const SHEETS: [&str; 3] = ["All sheets", "Current sheet", "Custom sheets"];
const OVERRIDDEN: [&str; 2] = ["Show underlines", "Hide underlines"];
const PDF_TEXT: [&str; 2] = ["Normal", "Selectable"];
const DXF_TEXT: [&str; 3] = ["Export as text", "Explode into polylines (notes)", "Explode all into polylines"];
const COLOR: [&str; 3] = ["Color", "Black and white", "Grayscale"];
/// DXF Units: the drawing's, or another (INSUNITS code).
/// Live dialog-export-drawing-all-formats (DWG / DXF Units).
const UNITS: [(&str, u8); 7] = [("Use drawing units", 0), ("Centimeter", 5), ("Foot", 2), ("Inch", 1), ("Meter", 6), ("Millimeter", 4), ("Yard", 10)];
const OPTIONS: [&str; 3] = ["Download", "Download and store…", "Store file in a new tab"];

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

impl DrawingEditor {
    /// Drawing tab › Export…: the Export Drawing modal.
    pub fn open_export_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_else(|| "Drawing".into());
        // Through the user's export rules (Preferences › User export rules).
        let values = crate::export_naming::values(&self.doc.read(cx).name(), &name, &name, None);
        let name = crate::export_naming::default_name(noshape_model::export_rules::ExportElement::Drawing, ExportSettings::default().format.label(), &values, name);
        let file = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let custom = cx.new(|cx| InputState::new(window, cx).placeholder("1-3, 5"));
        let st = Rc::new(RefCell::new(ExportSettings::default()));
        let ed = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let s = st.borrow().clone();
            let f = s.format;
            let select = |id: &'static str, label: &'static str, opts: Vec<String>, cur: usize, set: fn(&mut ExportSettings, usize)| {
                let st = st.clone();
                div().pt(px(8.)).child(crate::os_ui::field_label(label)).child(div().pt(px(6.)).child(crate::os_ui::select_field(id, opts, cur, move |i, w, _| {
                    set(&mut st.borrow_mut(), i);
                    w.refresh();
                })))
            };
            let check = |id: &'static str, label: &'static str, on: bool, set: fn(&mut ExportSettings, bool)| {
                let st = st.clone();
                crate::os_ui::checkbox(id, label, on).on_click(move |_, w, _| {
                    set(&mut st.borrow_mut(), !on);
                    w.refresh();
                })
            };
            let strs = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            let mut body = crate::os_ui::modal_body()
                // Live drawing/40: "View export rules" next to the label.
                .child(gpui_kit::component::h_flex().gap(px(4.)).items_end().child(crate::os_ui::field_label("File name")).child(crate::os_ui::export_rules_link("ex-rules")))
                .child(Input::new(&file).h(px(34.)).text_size(px(14.)))
                .child(select("ex-format", "Format", DrawingFormat::ALL.iter().map(|f| f.label().to_string()).collect(), DrawingFormat::ALL.iter().position(|x| *x == f).unwrap_or(0), |s, i| {
                    s.format = DrawingFormat::ALL[i];
                    s.version = default_version(s.format);
                    s.text = 0;
                }));
            if f == DrawingFormat::Pdf {
                body = body.child(select("ex-version", "Version", strs(&PDF_VERSIONS), s.version, |s, i| s.version = i));
            }
            if f.cad() {
                let list: &[(&str, CadVersion)] = if f == DrawingFormat::Dwt { &DWT_VERSIONS } else { &CAD_VERSIONS };
                body = body.child(select("ex-version", "Version", list.iter().map(|v| v.0.to_string()).collect(), s.version, |s, i| s.version = i));
                // Live: DXF Release 13 / 14 are model space only.
                if f == DrawingFormat::Dxf && matches!(s.version, 1 | 2) {
                    body = body.child(div().pt(px(6.)).text_size(px(13.33)).child("Model space only"));
                }
            }
            // Live: only DWG / DXF have Sheets (PDF exports every sheet; DWT,
            // SVG and the rasters the active one), whatever the sheet count.
            if matches!(f, DrawingFormat::Dwg | DrawingFormat::Dxf) {
                body = body.child(select("ex-sheets", "Sheets", strs(&SHEETS), s.sheets, |s, i| s.sheets = i));
                if s.sheets == 2 {
                    body = body.child(div().pt(px(6.)).child(Input::new(&custom).h(px(34.)).text_size(px(14.))));
                }
            }
            if !f.csv() {
                body = body.child(select("ex-overridden", "Overridden dimensions", strs(&OVERRIDDEN), s.overridden, |s, i| s.overridden = i));
            }
            match f {
                DrawingFormat::Pdf => body = body.child(select("ex-text", "Text", strs(&PDF_TEXT), s.text, |s, i| s.text = i)),
                DrawingFormat::Dwg | DrawingFormat::Dxf | DrawingFormat::Dwt => body = body.child(select("ex-text", "Text", strs(&DXF_TEXT), s.text, |s, i| s.text = i)),
                _ => {}
            }
            if f == DrawingFormat::Pdf || f.raster() {
                body = body.child(select("ex-color", "Color", strs(&COLOR), s.color, |s, i| s.color = i));
            }
            if matches!(f, DrawingFormat::Dwg | DrawingFormat::Dxf) {
                body = body.child(select("ex-units", "Units", UNITS.iter().map(|u| u.0.to_string()).collect(), s.units, |s, i| s.units = i));
            }
            body = body.child(select("ex-options", "Options", strs(&OPTIONS), s.options, |s, i| s.options = i));
            if !f.csv() && f != DrawingFormat::Pdf {
                body = body.child(div().pt(px(6.)).child(check("ex-splines", "Export splines as polylines", s.splines, |s, v| s.splines = v)));
            }
            if f.cad() {
                body = body.child(check("ex-z", "Set z-height to zero and normals to positive", s.z_zero, |s, v| s.z_zero = v));
            }
            if !f.csv() {
                body = body.child(div().pt(px(6.)).child(check("ex-hide-insp", "Hide inspection items", s.hide_inspection, |s, v| s.hide_inspection = v)));
            }
            if f.cad() {
                body = body.child(check("ex-offsheet", "Exclude off-sheet content", s.exclude_off_sheet, |s, v| s.exclude_off_sheet = v));
            }
            let (st2, file2, custom2, ed2) = (st.clone(), file.clone(), custom.clone(), ed.clone());
            crate::os_ui::modal(dialog, "Export Drawing", 600.)
                .child(body)
                .footer(crate::os_ui::modal_footer().child(crate::os_ui::modal_ok(crate::os_ui::primary_button("ok", "Export"))).child(crate::os_ui::modal_cancel(crate::os_ui::cancel_button("cancel", "Cancel", cx))))
                .on_ok(move |_, window, cx| {
                    let mut s = st2.borrow().clone();
                    s.custom = custom2.read(cx).value().to_string();
                    let name = file2.read(cx).value().trim().to_string();
                    let ed = ed2.clone();
                    window.defer(cx, move |window, cx| ed.update(cx, |t, cx| t.export_with(s, name, window, cx)));
                    true
                })
        });
    }

    /// Export with the modal's settings: ask for the location, then write.
    fn export_with(&mut self, s: ExportSettings, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let name = if name.is_empty() { self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_else(|| "Drawing".into()) } else { name };
        let count = self.drawing.sheets.len();
        let sheets: Vec<usize> = match s.sheets {
            _ if s.format.csv() => vec![],
            // DWT: sheets 1 and 2 (the second is the continuation template).
            _ if s.format == DrawingFormat::Dwt => (0..count.min(2)).collect(),
            1 => vec![self.sheet],
            2 => parse_sheet_range(&s.custom, count),
            _ => (0..count).collect(),
        };
        if !s.format.csv() && sheets.is_empty() {
            self.status = Some("No sheets to export".into());
            cx.notify();
            return;
        }
        let zip = !matches!(s.format, DrawingFormat::Pdf | DrawingFormat::Dwt | DrawingFormat::Dxf | DrawingFormat::Dwg) && sheets.len() > 1;
        let ext = if zip { "zip" } else { s.format.ext() };
        // Store file in a new tab: no download.
        if s.options == 2 {
            self.write_export_with(s, sheets, name, None, cx);
            return;
        }
        let this = cx.entity().downgrade();
        crate::files::request_save_path("Export drawing", format!("{name}.{ext}"), ext, window, cx, move |path, cx| {
            this.update(cx, |this, cx| this.write_export_with(s, sheets, name, Some(path), cx)).ok();
        });
    }

    /// Document menu › Print…: every sheet as a PDF (as Onshape's
    /// print-friendly page), opened in the system viewer to print or save.
    pub fn print(&mut self, cx: &mut Context<Self>) {
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_else(|| "Drawing".into());
        let dir = std::env::temp_dir().join("noshape-print");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = Some(format!("Print failed: {e}").into());
            cx.notify();
            return;
        }
        let safe: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' }).collect();
        let path = dir.join(format!("{safe}.pdf"));
        let s = ExportSettings { print: true, ..Default::default() };
        let sheets = (0..self.drawing.sheets.len()).collect();
        self.write_export_with(s, sheets, name, Some(path), cx);
    }

    /// Download (write to `path`) and / or store the exported file in a new
    /// file tab after the drawing (Options).
    fn deliver(&mut self, file_name: String, bytes: Vec<u8>, path: Option<&PathBuf>, store: bool, cx: &mut Context<Self>) -> Result<String, String> {
        if let Some(p) = path {
            std::fs::write(p, &bytes).map_err(|e| e.to_string())?;
        }
        if store {
            let tab = self.tab;
            self.doc.update(cx, |d, cx| {
                d.edit_document(&format!("Store {file_name}"), cx, |d| {
                    crate::file_tab_view::add_file_tab(d, noshape_model::file_tab::FileTab::new(&file_name, &bytes, noshape_model::file_tab::now()), None, Some(tab));
                })
            });
        }
        Ok(match (path, store) {
            (Some(p), true) => format!("Export completed: {} (also stored in a new tab)", p.display()),
            (Some(p), false) => format!("Export completed: {}", p.display()),
            _ => format!("Export completed: {file_name} stored in a new tab"),
        })
    }

    fn write_export_with(&mut self, s: ExportSettings, sheets: Vec<usize>, name: String, path: Option<PathBuf>, cx: &mut Context<Self>) {
        let f = s.format;
        let store = s.options >= 1;
        let zip = !matches!(f, DrawingFormat::Pdf | DrawingFormat::Dwt | DrawingFormat::Dxf | DrawingFormat::Dwg) && sheets.len() > 1;
        let file_name = format!("{name}.{}", if zip { "zip" } else { f.ext() });
        if f.csv() {
            let csv = noshape_model::drawing::inspection::rows_csv(&self.inspection_rows(f == DrawingFormat::DefinitionList));
            let done = self.deliver(file_name, csv.into_bytes(), path.as_ref(), store, cx);
            self.status = Some(done.unwrap_or_else(|e| format!("Export failed: {e}")).into());
            cx.notify();
            return;
        }
        let mut drawing = self.drawing.clone();
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
        // Shaded views (live: PDF embeds their picture, DXF keeps the edges).
        let (shaded, shaded_sheet) = (self.shaded_exports(), self.sheet);
        let watermark = crate::drawing_watermark::text(self.doc.read(cx), self.tab);
        let sources = self.sources.clone();
        let cache = self.cache.clone();
        let info = self.info.clone();
        let insunits = match UNITS[s.units].1 {
            0 if drawing.units_mm => 4,
            0 => 1,
            u => u,
        };
        self.status = Some(format!("{name} is being prepared for export").into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            // Vector output (or SVG for rasterizing) per sheet, off the UI thread.
            let result: Result<Vec<(String, Vec<u8>)>, String> = cx
                .background_spawn(async move {
                    let mut pages = Vec::new();
                    for &i in &sheets {
                        let built = render_sheet_with(&drawing, i, &sources, &cache, true, &info).ok_or("nothing to export")?;
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
                        let pictures = i == shaded_sheet && (f == DrawingFormat::Pdf || f == DrawingFormat::Svg || f.raster());
                        // A shaded view's picture replaces its edges.
                        let mut g = built.geometry.clone();
                        if pictures {
                            for (_, r) in g.view_edges.clone().iter().filter(|(v, _)| shaded.iter().any(|x| x.0 == *v)) {
                                for l in &mut g.lines[r.clone()] {
                                    l.points.clear();
                                }
                            }
                        }
                        let mut geo = export::prepare(&g, &opts);
                        // Release management › Watermarks: over everything,
                        // whatever the colour mode, on PDF, the raster formats
                        // and print; model spaces have none.
                        if let Some(text) = watermark.as_deref().filter(|_| (f == DrawingFormat::Pdf || f.raster()) && !sheet.imported.as_ref().is_some_and(|m| m.model)) {
                            geo.overlays.extend(crate::drawing_watermark::overlays(text, geo.width, geo.height));
                        }
                        if pictures {
                            for (_, pos, size, png) in &shaded {
                                geo.images.insert(0, noshape_model::drawing::sheet::GeomImage { pos: *pos, size: *size, data_base64: noshape_model::sketch_feature::base64_encode(png).into(), rgb: None });
                            }
                        }
                        // PDF and DWG / DXF (OLE2FRAME) embed decoded image pixels.
                        if f == DrawingFormat::Pdf || f.cad() {
                            for im in &mut geo.images {
                                im.rgb = crate::drawing_images::decode_rgb(&im.data_base64);
                            }
                        }
                        // DXF / DWG lay the sheets out by their content before exclusion.
                        let range = noshape_model::drawing::cad_export::content_x_range(&built.geometry);
                        pages.push((drawing.sheets[i].name.clone(), geo, range));
                    }
                    let files: Vec<(String, Vec<u8>)> = match f {
                        DrawingFormat::Pdf => vec![(String::new(), export::to_pdf_pages_version(&pages.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), PDF_VERSIONS[s.version]))],
                        // Several sheets: one file, a layout per sheet (live drw2 l).
                        DrawingFormat::Dxf if pages.len() > 1 => vec![(String::new(), noshape_model::drawing::cad_export::to_dxf_sheets(&pages, insunits, cad_version.unwrap_or(noshape_model::drawing::cad_export::CadVersion::AC1009))?)],
                        DrawingFormat::Dwg if pages.len() > 1 => vec![(String::new(), noshape_model::drawing::cad_export::to_dwg_sheets(&pages, insunits, cad_version.unwrap_or(CAD_VERSIONS[3].1))?)],
                        DrawingFormat::Dxf => match cad_version {
                            Some(v) => pages.iter().map(|(n, g, _)| noshape_model::drawing::cad_export::to_dxf_version(g, insunits, v).map(|b| (n.clone(), b))).collect::<Result<Vec<_>, String>>()?,
                            // Release 11-12: the view geometry only (live).
                            None => pages.iter().map(|(n, g, _)| (n.clone(), noshape_model::drawing::cad_export::to_dxf_r12(g, insunits).into_bytes())).collect(),
                        },
                        DrawingFormat::Dwg => pages.iter().map(|(n, g, _)| noshape_model::drawing::cad_export::to_dwg_file(g, insunits, cad_version.unwrap_or(CAD_VERSIONS[3].1)).map(|b| (n.clone(), b))).collect::<Result<Vec<_>, String>>()?,
                        // With the sheet formats, so Select a DWT gets them back exactly.
                        DrawingFormat::Dwt => {
                            let templates: Vec<noshape_model::drawing::dwt::SheetTemplate> = sheets.iter().filter_map(|i| drawing.sheets.get(*i)).map(noshape_model::drawing::dwt::SheetTemplate::of).collect();
                            vec![(String::new(), noshape_model::drawing::dwt::to_dwt(&pages.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), &templates, insunits, cad_version.unwrap_or(DWT_VERSIONS[0].1))?)]
                        }
                        _ => pages.iter().map(|(n, g, _)| (n.clone(), export::to_svg(g).into_bytes())).collect(),
                    };
                    Ok(files)
                })
                .await;
            this.update(cx, |this, cx| {
                let done = result.and_then(|files| -> Result<String, String> {
                    let files = if f.raster() { files.into_iter().map(|(n, svg)| Ok((n, rasterize(&svg, f, cx)?))).collect::<Result<Vec<_>, String>>()? } else { files };
                    let bytes = if files.len() == 1 {
                        files.into_iter().next().map(|f| f.1).unwrap_or_default()
                    } else {
                        let named: Vec<(String, Vec<u8>)> = files.into_iter().map(|(sheet, b)| (format!("{name} - {sheet}.{}", f.ext()), b)).collect();
                        export::zip_store(&named)
                    };
                    this.deliver(file_name.clone(), bytes, path.as_ref(), store, cx)
                });
                if s.print {
                    this.status = match done {
                        Ok(_) => {
                            if let Some(p) = &path {
                                cx.open_url(&format!("file://{}", p.display()));
                            }
                            None
                        }
                        Err(e) => Some(format!("Print failed: {e}").into()),
                    };
                    cx.notify();
                    return;
                }
                this.status = Some(done.unwrap_or_else(|e| format!("Export failed: {e}")).into());
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// Render an exported SVG at 300 dpi and encode it.
fn rasterize(svg: &[u8], f: DrawingFormat, cx: &App) -> Result<Vec<u8>, String> {
    // usvg reads the mm size at 96 dpi; gpui renders at twice the factor.
    let img = cx.svg_renderer().render_single_frame(svg, 300.0 / 96.0 / 2.0).map_err(|e| e.to_string())?;
    let size = img.size(0);
    let bgra = img.as_bytes(0).ok_or("no image")?;
    let mut rgba = bgra.to_vec();
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    let buf = image::RgbaImage::from_raw(size.width.0 as u32, size.height.0 as u32, rgba).ok_or("bad image size")?;
    let mut out = std::io::Cursor::new(Vec::new());
    let r = match f {
        DrawingFormat::Jpeg => image::DynamicImage::ImageRgba8(buf).to_rgb8().write_to(&mut out, image::ImageFormat::Jpeg),
        DrawingFormat::Tiff => buf.write_to(&mut out, image::ImageFormat::Tiff),
        _ => buf.write_to(&mut out, image::ImageFormat::Png),
    };
    r.map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::{CAD_VERSIONS, DWT_VERSIONS, DrawingFormat, ExportSettings, PDF_VERSIONS, UNITS, default_version, parse_sheet_range};

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
    fn custom_sheet_ranges() {
        assert_eq!(parse_sheet_range("1-3, 5", 6), vec![0, 1, 2, 4]);
        assert_eq!(parse_sheet_range("3-1 9", 4), vec![0, 1, 2]);
        assert!(parse_sheet_range("x", 3).is_empty());
    }
}

