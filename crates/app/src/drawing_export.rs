//! Drawing editor: Onshape's "Export Drawing" modal (drawing tab
//! right-click › Export…; live drawing/40, dom/dialog-export-drawing.txt).
//! The fields depend on the format: File name, Format, Version, Sheets
//! (several sheets), Overridden dimensions, Text, Color, Units, Options,
//! Export splines as polylines, Set z-height to zero…, Hide inspection
//! items, Exclude off-sheet content. Several sheets of a single-page format
//! download as one zip.

use crate::drawing_view::*;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::*;
use noshape_model::drawing::build::render_sheet_with;
use noshape_model::drawing::export::{self, ExportOptions};
use noshape_model::drawing::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// Export Drawing › Format, in Onshape's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawingFormat {
    Pdf,
    Dxf,
    Svg,
    Png,
    Jpeg,
    Tiff,
    InspectionList,
    DefinitionList,
}

impl DrawingFormat {
    pub const ALL: [DrawingFormat; 8] = [DrawingFormat::Pdf, DrawingFormat::Dxf, DrawingFormat::Svg, DrawingFormat::Png, DrawingFormat::Jpeg, DrawingFormat::Tiff, DrawingFormat::InspectionList, DrawingFormat::DefinitionList];
    pub fn label(self) -> &'static str {
        match self {
            DrawingFormat::Pdf => "PDF",
            DrawingFormat::Dxf => "DXF",
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
            DrawingFormat::Dxf => "dxf",
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
    fn csv(self) -> bool {
        matches!(self, DrawingFormat::InspectionList | DrawingFormat::DefinitionList)
    }
}

const PDF_VERSIONS: [&str; 4] = ["1.5", "1.6", "1.7", "1.4"];
const SHEETS: [&str; 3] = ["All sheets", "Current sheet", "Custom sheets"];
const OVERRIDDEN: [&str; 2] = ["Show underlines", "Hide underlines"];
const PDF_TEXT: [&str; 2] = ["Normal", "Selectable"];
const DXF_TEXT: [&str; 3] = ["Export as text", "Explode into polylines (notes)", "Explode all into polylines"];
const COLOR: [&str; 3] = ["Color", "Black and white", "Grayscale"];
/// DXF Units: the drawing's, or another (INSUNITS code).
const UNITS: [(&str, u8); 6] = [("Drawing units", 0), ("Millimeters", 4), ("Centimeters", 5), ("Meters", 6), ("Inches", 1), ("Feet", 2)];
const OPTIONS: [&str; 1] = ["Download"];

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
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self { format: DrawingFormat::Pdf, version: 0, sheets: 0, custom: String::new(), overridden: 0, text: 0, color: 0, units: 0, splines: false, z_zero: false, hide_inspection: false, exclude_off_sheet: false }
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
        let file = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let custom = cx.new(|cx| InputState::new(window, cx).placeholder("1-3, 5"));
        let st = Rc::new(RefCell::new(ExportSettings::default()));
        let ed = cx.entity();
        let sheet_count = self.drawing.sheets.len();
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
                .child(crate::os_ui::field_label("File name"))
                .child(Input::new(&file).h(px(34.)).text_size(px(14.)))
                .child(select("ex-format", "Format", DrawingFormat::ALL.iter().map(|f| f.label().to_string()).collect(), DrawingFormat::ALL.iter().position(|x| *x == f).unwrap_or(0), |s, i| {
                    s.format = DrawingFormat::ALL[i];
                    s.version = 0;
                    s.text = 0;
                }));
            if f == DrawingFormat::Pdf {
                body = body.child(select("ex-version", "Version", strs(&PDF_VERSIONS), s.version, |s, i| s.version = i));
            }
            if f == DrawingFormat::Dxf {
                // The DXF writer emits AutoCAD R12 (AC1009).
                body = body.child(select("ex-version", "Version", vec!["R12".into()], 0, |s, i| s.version = i));
            }
            if !f.csv() && sheet_count > 1 {
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
                DrawingFormat::Dxf => body = body.child(select("ex-text", "Text", strs(&DXF_TEXT), s.text, |s, i| s.text = i)),
                _ => {}
            }
            if f == DrawingFormat::Pdf || f.raster() {
                body = body.child(select("ex-color", "Color", strs(&COLOR), s.color, |s, i| s.color = i));
            }
            if f == DrawingFormat::Dxf {
                body = body.child(select("ex-units", "Units", UNITS.iter().map(|u| u.0.to_string()).collect(), s.units, |s, i| s.units = i));
            }
            body = body.child(select("ex-options", "Options", strs(&OPTIONS), 0, |_, _| {}));
            if !f.csv() && f != DrawingFormat::Pdf {
                body = body.child(div().pt(px(6.)).child(check("ex-splines", "Export splines as polylines", s.splines, |s, v| s.splines = v)));
            }
            if f == DrawingFormat::Dxf {
                body = body.child(check("ex-z", "Set z-height to zero and normals to positive", s.z_zero, |s, v| s.z_zero = v));
            }
            if !f.csv() {
                body = body.child(div().pt(px(6.)).child(check("ex-hide-insp", "Hide inspection items", s.hide_inspection, |s, v| s.hide_inspection = v)));
            }
            if f == DrawingFormat::Dxf {
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
            1 => vec![self.sheet],
            2 => parse_sheet_range(&s.custom, count),
            _ => (0..count).collect(),
        };
        if !s.format.csv() && sheets.is_empty() {
            self.status = Some("No sheets to export".into());
            cx.notify();
            return;
        }
        let zip = s.format != DrawingFormat::Pdf && sheets.len() > 1;
        let ext = if zip { "zip" } else { s.format.ext() };
        let this = cx.entity().downgrade();
        crate::files::request_save_path("Export drawing", format!("{name}.{ext}"), ext, window, cx, move |path, cx| {
            this.update(cx, |this, cx| this.write_export_with(s, sheets, name, path, cx)).ok();
        });
    }

    fn write_export_with(&mut self, s: ExportSettings, sheets: Vec<usize>, name: String, path: PathBuf, cx: &mut Context<Self>) {
        let f = s.format;
        if f.csv() {
            let csv = noshape_model::drawing::inspection::rows_csv(&self.inspection_rows(f == DrawingFormat::DefinitionList));
            self.status = Some(match std::fs::write(&path, csv) {
                Ok(()) => format!("Exported {}", path.display()),
                Err(e) => format!("Export failed: {e}"),
            }
            .into());
            cx.notify();
            return;
        }
        let mut drawing = self.drawing.clone();
        if s.hide_inspection {
            drawing.props.hide_inspection = true;
        }
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
                        // Explode notes: texts picked as notes.
                        let explode: Vec<usize> = if f == DrawingFormat::Dxf && s.text == 1 {
                            let sheet = &drawing.sheets[i];
                            built
                                .geometry
                                .texts
                                .iter()
                                .enumerate()
                                .filter(|(_, t)| matches!(built.picks.get(t.pick as usize), Some(noshape_model::drawing::build::PickTarget::Annotation(id)) if sheet.annotations.iter().any(|a| a.id == *id && matches!(a.kind, AnnotationKind::Note { .. }))))
                                .map(|(k, _)| k)
                                .collect()
                        } else {
                            vec![]
                        };
                        let opts = ExportOptions {
                            explode_all: (f == DrawingFormat::Pdf && s.text == 0) || (f == DrawingFormat::Dxf && s.text == 2) || f.raster(),
                            strokes: f == DrawingFormat::Dxf,
                            explode,
                            underline_overrides: s.overridden == 0,
                            exclude_off_sheet: s.exclude_off_sheet,
                        };
                        pages.push((drawing.sheets[i].name.clone(), export::prepare(&built.geometry, &opts)));
                    }
                    let files: Vec<(String, Vec<u8>)> = match f {
                        DrawingFormat::Pdf => vec![(String::new(), export::to_pdf_pages_version(&pages.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), PDF_VERSIONS[s.version]))],
                        DrawingFormat::Dxf => pages.iter().map(|(n, g)| (n.clone(), export::to_dxf_units(g, insunits).into_bytes())).collect(),
                        _ => pages.iter().map(|(n, g)| (n.clone(), export::to_svg(g).into_bytes())).collect(),
                    };
                    Ok(files)
                })
                .await;
            this.update(cx, |this, cx| {
                let done = result.and_then(|files| {
                    let files = if f.raster() { files.into_iter().map(|(n, svg)| Ok((n, rasterize(&svg, f, cx)?))).collect::<Result<Vec<_>, String>>()? } else { files };
                    let bytes = if files.len() == 1 {
                        files.into_iter().next().map(|f| f.1).unwrap_or_default()
                    } else {
                        let named: Vec<(String, Vec<u8>)> = files.into_iter().map(|(sheet, b)| (format!("{name} - {sheet}.{}", f.ext()), b)).collect();
                        export::zip_store(&named)
                    };
                    std::fs::write(&path, bytes).map_err(|e| e.to_string())
                });
                this.status = Some(match done {
                    Ok(()) => format!("Export completed: {}", path.display()).into(),
                    Err(e) => format!("Export failed: {e}").into(),
                });
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
    for px in rgba.chunks_exact_mut(4) {
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
    use super::parse_sheet_range;

    #[test]
    fn custom_sheet_ranges() {
        assert_eq!(parse_sheet_range("1-3, 5", 6), vec![0, 1, 2, 4]);
        assert_eq!(parse_sheet_range("3-1 9", 4), vec![0, 1, 2]);
        assert!(parse_sheet_range("x", 3).is_empty());
    }
}
