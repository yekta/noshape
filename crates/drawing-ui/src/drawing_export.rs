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
use noshape_model::drawing::cad_export::CadVersion;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

// The formats, versions, settings and writers are shared with the agent API.
pub use noshape_ops::drawing_export::{COLOR, DWT_VERSIONS, DXF_TEXT, DrawingFormat, ExportExtras, ExportSettings, OPTIONS, OVERRIDDEN, PDF_TEXT, PDF_VERSIONS, SHEETS, UNITS, default_version, export_sheets_of};
use noshape_model::drawing::cad_export::CAD_VERSIONS;

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
                .child(gpui_kit::component::h_flex().gap(px(4.)).items_end().child(crate::os_ui::field_label("File name")).child(crate::drawing_hooks::export_rules_link("ex-rules")))
                .child(Input::new(&file).h(px(34.)).text_size(px(14.)))
                .child(select("ex-format", "Format", DrawingFormat::ALL.iter().map(|f| f.label().to_string()).collect(), DrawingFormat::ALL.iter().position(|x| *x == f).unwrap_or(0), |s, i| {
                    s.format = DrawingFormat::ALL[i];
                    s.version = default_version(s.format);
                    s.text = 0;
                }));
            // Live dialog-export-drawing-all-formats: each format's note.
            if f == DrawingFormat::Dwg {
                body = body.child(div().pt(px(6.)).text_size(px(13.33)).child("In the exported file, dimensions added to a view with a scale different from the active sheet may be out of scale."));
            }
            if f == DrawingFormat::Svg || f.raster() {
                body = body.child(div().pt(px(6.)).text_size(px(13.33)).child("Selected format only supports exporting the active sheet."));
            }
            match f {
                DrawingFormat::InspectionList => body = body.child(div().pt(px(6.)).text_size(px(13.33)).child("Exported csv file will contain an ordered list of all inspection symbols.")),
                DrawingFormat::DefinitionList => body = body.child(div().pt(px(6.)).text_size(px(13.33)).child("Exported csv file will contain all dimensions and annotations.")),
                _ => {}
            }
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
                if f == DrawingFormat::Dwt {
                    body = body.child(div().pt(px(6.)).text_size(px(13.33)).child("Primary template is the first sheet.")).child(div().text_size(px(13.33)).child("Continuation template is the first or, if it exists, second sheet."));
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
            // DWT has no Overridden dimensions (live).
            if !f.csv() && f != DrawingFormat::Dwt {
                body = body.child(select("ex-overridden", "Overridden dimensions", strs(&OVERRIDDEN), s.overridden, |s, i| s.overridden = i));
            }
            match f {
                DrawingFormat::Pdf => body = body.child(select("ex-text", "Text", strs(&PDF_TEXT), s.text, |s, i| s.text = i)),
                DrawingFormat::Dwg | DrawingFormat::Dxf | DrawingFormat::Dwt => body = body.child(select("ex-text", "Text", strs(&DXF_TEXT), s.text, |s, i| s.text = i)),
                _ => {}
            }
            // Only PDF has Color (live: SVG / PNG / JPEG / TIFF have just
            // Overridden dimensions and Hide inspection items).
            if f == DrawingFormat::Pdf {
                body = body.child(select("ex-color", "Color", strs(&COLOR), s.color, |s, i| s.color = i));
            }
            if matches!(f, DrawingFormat::Dwg | DrawingFormat::Dxf) {
                body = body.child(select("ex-units", "Units", UNITS.iter().map(|u| u.0.to_string()).collect(), s.units, |s, i| s.units = i));
            }
            body = body.child(select("ex-options", "Options", strs(&OPTIONS), s.options, |s, i| s.options = i));
            if f.cad() {
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
        let sheets = export_sheets_of(&s, self.drawing.sheets.len(), self.sheet);
        if !s.format.csv() && sheets.is_empty() {
            self.status = Some("No sheets to export".into());
            cx.notify();
            return;
        }
        let zip = noshape_ops::drawing_export::zipped(s.format, sheets.len());
        let ext = if zip { "zip" } else { s.format.ext() };
        // Store file in a new tab: no download.
        if s.options == 2 {
            self.write_export_with(s, sheets, name, None, cx);
            return;
        }
        let this = cx.entity().downgrade();
        crate::file_dialogs::request_save_path("Export drawing", format!("{name}.{ext}"), ext, window, cx, move |path, cx| {
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
                d.edit_document(&noshape_ops::export::store_description(&file_name), cx, |d| {
                    noshape_ops::export::store_file_tab(d, &file_name, &bytes, tab);
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
        let zip = noshape_ops::drawing_export::zipped(f, sheets.len());
        let file_name = format!("{name}.{}", if zip { "zip" } else { f.ext() });
        if f.csv() {
            let csv = noshape_model::drawing::inspection::rows_csv(&self.inspection_rows(f == DrawingFormat::DefinitionList));
            let done = self.deliver(file_name, csv.into_bytes(), path.as_ref(), store, cx);
            self.status = Some(done.unwrap_or_else(|e| format!("Export failed: {e}")).into());
            cx.notify();
            return;
        }
        // What only the editor has: the shaded views' pictures and the
        // release watermark (shared writers: noshape_ops::drawing_export).
        let extras = ExportExtras {
            shaded: self.shaded_exports(),
            shaded_sheet: self.sheet,
            watermark: crate::drawing_watermark::text(self.doc.read(cx), self.tab).map(|text| Box::new(move |w: f64, h: f64| crate::drawing_watermark::overlays(&text, w, h)) as Box<noshape_ops::drawing_export::WatermarkFn>),
        };
        let drawing = self.drawing.clone();
        let sources = self.sources.clone();
        let cache = self.cache.clone();
        let info = self.info.clone();
        // Live (qa4 r5 s11): a blue banner, closed after about 2–3 s
        // whether or not the file is ready.
        self.show_message(format!("{name} is being prepared for export. You will receive a notification with its status."), crate::os_ui::BubbleKind::Info, Some(std::time::Duration::from_millis(2500)), cx);
        cx.notify();
        let settings = s.clone();
        cx.spawn(async move |this, cx| {
            // Vector output (or SVG for rasterizing) per sheet, off the UI thread.
            let result: Result<Vec<(String, Vec<u8>)>, String> = cx
                .background_spawn(async move {
                    let files = noshape_ops::drawing_export::export_files(&drawing, &sheets, &settings, &sources, &cache, &info, &extras)?;
                    if f.raster() { files.into_iter().map(|(n, svg)| Ok((n, noshape_ops::drawing_export::rasterize(&svg, f)?))).collect() } else { Ok(files) }
                })
                .await;
            this.update(cx, |this, cx| {
                let done = result.and_then(|files| -> Result<String, String> {
                    let bytes = noshape_ops::drawing_export::bundle(files, &name, f);
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
