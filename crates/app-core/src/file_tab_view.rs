//! File tabs (Onshape's imported / stored files): the tab shows a centred
//! card (live documents/67): the type icon, the bold file name, the type
//! ("STEP file"), "File size: 49 KB", "Uploaded 10:20 pm Oct 18 2026" and
//! [Update…] [Download]. Helpers here also add file tabs (Import, drawing
//! export "Store file in a new tab") and translate them (CAD → Part
//! Studio, DXF / DWG → Drawing).

use crate::doc_model::DocModel;
use crate::theme::p;
use gpui_kit::component::button::Button;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::file_tab::{FileKind, FileTab};

/// The tab-bar / card icon of a file: Onshape's per-format art (live
/// tabbar/icons2-filetab-*: STEP, X_T, X_B, JT, STL, PVZ, 3MF, SolidWorks,
/// DXF/DWG, DWT, TXT, ZIP have their own; IGES, SAT, 3DM, CATIA, Inventor,
/// Solid Edge and NX share the generic geometry card; CSV / JSON the page).
pub fn file_icon(f: &FileTab) -> &'static str {
    let ext = f.file_name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "step" | "stp" => return "file-cad",
        "x_t" | "xmt_txt" => return "file-x_t",
        "x_b" | "xmt_bin" => return "file-x_b",
        "jt" => return "file-jt",
        "stl" => return "file-stl",
        "pvz" => return "file-pvz",
        "3mf" => return "file-3mf",
        "sldprt" | "sldasm" | "slddrw" => return "file-sw",
        "dxf" | "dwg" => return "file-dwg",
        "dwt" => return "file-dwt",
        "txt" => return "file-txt",
        "zip" => return "file-zip",
        "igs" | "iges" | "sat" | "sab" | "3dm" | "catpart" | "catproduct" | "ipt" | "iam" | "par" | "psm" | "asm" | "prt" | "obj" | "brep" | "x3d" => return "file-geometry",
        "csv" | "json" => return "file",
        _ => {}
    }
    match f.kind() {
        FileKind::Cad => "file-cad",
        FileKind::Drawing2d | FileKind::Template => "file-dwg",
        FileKind::Image => "file-image",
        FileKind::Pdf => "file-pdf",
        FileKind::Csv | FileKind::Other => "file",
    }
}

/// Add a file tab named after the file, in `folder` when given (shared with
/// the agent API).
pub use noshape_ops::import::add_file_tab;

/// The "CAD Imports" folder (Onshape keeps imported CAD files there), made if needed.
pub fn cad_imports_folder(d: &mut Document) -> TabId {
    match d.settings.folders.iter().find(|f| f.name == "CAD Imports") {
        Some(f) => f.id,
        None => d.add_folder("CAD Imports", None),
    }
}

/// A new Drawing whose first sheet holds the DXF / DWG's model space as
/// sketch geometry (Onshape imports a DWG/DXF as a file tab plus a drawing).
pub fn drawing_from_cad(bytes: &[u8]) -> Result<noshape_model::drawing::Drawing, String> {
    let parsed = noshape_sketch::cad::parse_cad(bytes).map_err(|e| e.to_string())?;
    let mut drawing = noshape_model::drawing::Drawing::default();
    let scale = parsed.unit_scale.map(|m| m * 1000.0).unwrap_or(1.0);
    if let Some(s) = drawing.sheets.first_mut() {
        s.sketch.sketch.insert_drawing(&parsed, scale, noshape_sketch::DVec2::ZERO);
        let _ = s.sketch.sketch.solve();
    }
    Ok(drawing)
}

pub struct FileTabView {
    doc: Entity<DocModel>,
    tab: TabId,
}

impl FileTabView {
    pub fn new(doc: Entity<DocModel>, tab: TabId, cx: &mut Context<Self>) -> Self {
        cx.observe(&doc, |_, _, cx| cx.notify()).detach();
        Self { doc, tab }
    }

    fn file(&self, cx: &App) -> Option<FileTab> {
        match &self.doc.read(cx).session.doc.tab(self.tab)?.content {
            TabContent::File(f) => Some(f.clone()),
            _ => None,
        }
    }
}

/// The save dialog's filter extension (known types; else none).
fn static_ext(e: &str) -> &'static str {
    const KNOWN: [&str; 20] = ["step", "stp", "iges", "igs", "brep", "stl", "obj", "dxf", "dwg", "dwt", "png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "svg", "pdf", "csv"];
    KNOWN.iter().find(|k| **k == e).copied().unwrap_or("")
}

/// Download: save the file's bytes where the user says.
pub fn download(doc: &Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut App) {
    let Some(TabContent::File(f)) = doc.read(cx).session.doc.tab(tab).map(|t| t.content.clone()) else { return };
    let ext = static_ext(&f.ext());
    let doc = doc.clone();
    crate::file_dialogs::request_save_path("Download", f.file_name.clone(), ext, window, cx, move |path, cx| {
        let r = f.bytes().ok_or_else(|| "the file data is damaged".to_string()).and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string()));
        let msg = match r {
            Ok(()) => format!("Downloaded {}", path.display()),
            Err(e) => format!("Download failed: {e}"),
        };
        doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::Error(msg)));
    });
}

/// Update…: replace the file with another one (the tab keeps its name).
pub fn update(doc: &Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut App) {
    let doc = doc.clone();
    crate::file_dialogs::request_open_path("Update file", "All files", &["*"], window, cx, move |path, cx| {
        let Ok(bytes) = std::fs::read(&path) else { return };
        let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        doc.update(cx, |d, cx| {
            d.edit_document(&format!("Update {name}"), cx, |d| {
                if let Some(t) = d.tab_mut(tab) {
                    t.content = TabContent::File(FileTab::new(&name, &bytes, noshape_model::file_tab::now()));
                }
            })
        });
    });
}

/// "Uploaded 10:20 pm Oct 18 2026" (local time).
fn uploaded_label(secs: i64) -> String {
    use chrono::{Local, TimeZone};
    match Local.timestamp_opt(secs, 0).single() {
        Some(t) => format!("Uploaded {}", t.format("%-I:%M %P %b %-d %Y")),
        None => String::new(),
    }
}

impl Render for FileTabView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let Some(f) = self.file(cx) else { return div().size_full().into_any_element() };
        let (doc, tab) = (self.doc.clone(), self.tab);
        let doc2 = doc.clone();
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            // Live: the card sits on the gray work area.
            .bg(gpui_kit::rgb(0x999999))
            .child(
                v_flex()
                    .w(px(300.))
                    .py(px(36.))
                    .px(px(20.))
                    .gap(px(10.))
                    .items_center()
                    .bg(crate::theme::p().panel)
                    .rounded(px(5.))
                    .border_1()
                    .border_color(gpui_kit::rgb(0xd3d7d9))
                    .text_color(pal.text)
                    .child(crate::assets::icon(file_icon(&f), 100., cx))
                    .child(div().pt(px(16.)).text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).text_center().child(crate::ui::user_text(f.file_name.clone())))
                    .child(div().text_size(px(12.)).child(f.type_label()))
                    .child(div().text_size(px(12.)).child(format!("File size: {}", f.size_label())))
                    .child(div().text_size(px(12.)).child(uploaded_label(f.uploaded)))
                    .child(
                        gpui_kit::component::h_flex()
                            .gap(px(10.))
                            .child(Button::new("file-update").label("Update…").outline().small().on_click(move |_, w, cx| update(&doc, tab, w, cx)))
                            .child(Button::new("file-download").label("Download").outline().small().on_click(move |_, w, cx| download(&doc2, tab, w, cx))),
                    ),
            )
            .into_any_element()
    }
}

/// Onshape's insert-from-file dialog (Insert image, Insert DXF or DWG,
/// Select a DWT): search the document's file tabs of `kinds`, click one to
/// insert it; "Import" brings a local file into the document (as a file
/// tab) and inserts it.
pub fn pick_file(doc: Entity<DocModel>, title: &'static str, kinds: &'static [FileKind], exts: &'static [&'static str], window: &mut Window, cx: &mut App, on_pick: impl Fn(FileTab, &mut Window, &mut App) + 'static) {
    let search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search files"));
    let on_pick = std::rc::Rc::new(on_pick);
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let q = search.read(cx).value().to_lowercase();
        let files: Vec<(TabId, String, FileTab)> = doc
            .read(cx)
            .session
            .doc
            .tabs
            .iter()
            .filter_map(|t| match &t.content {
                TabContent::File(f) if kinds.contains(&f.kind()) && t.name.to_lowercase().contains(&q) => Some((t.id, t.name.clone(), f.clone())),
                _ => None,
            })
            .collect();
        let mut list = v_flex().id("pick-file-list").h(px(240.)).overflow_y_scroll().border_1().border_color(gpui_kit::rgb(0xd3d7d9));
        if files.is_empty() {
            list = list.child(div().p(px(12.)).text_size(px(13.)).text_color(pal.text_muted).child("No files in this document. Use Import to add one."));
        }
        for (id, name, f) in files {
            let (pick, f2) = (on_pick.clone(), f.clone());
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("pick-{id}")))
                    .h(px(32.))
                    .px(px(8.))
                    .gap(px(8.))
                    .text_size(px(13.))
                    .cursor_pointer()
                    .hover(|d| d.bg(gpui_kit::rgb(0xdef1fb)))
                    .child(crate::assets::icon(file_icon(&f), 20., cx))
                    .child(div().flex_1().truncate().child(crate::ui::user_text(name)))
                    .child(div().text_size(px(12.)).text_color(pal.text_muted).child(f.size_label()))
                    .on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        pick(f2.clone(), window, cx);
                    }),
            );
        }
        let (doc2, pick2) = (doc.clone(), on_pick.clone());
        crate::os_ui::modal(dialog, title, 560.)
            .child(crate::os_ui::modal_body().gap(px(8.)).child(Input::new(&search).h(px(30.)).text_size(px(13.))).child(list))
            .footer(
                crate::os_ui::modal_footer()
                    .child(
                        Button::new("pick-import").label("Import").outline().on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            let (doc, pick) = (doc2.clone(), pick2.clone());
                            crate::file_dialogs::request_open_path(title, "Files", exts, window, cx, move |path, cx| {
                                let Ok(bytes) = std::fs::read(&path) else { return };
                                let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                                let file = FileTab::new(&name, &bytes, noshape_model::file_tab::now());
                                let f2 = file.clone();
                                doc.update(cx, |d, cx| d.edit_document(&format!("Import {name}"), cx, |d| {
                                    add_file_tab(d, f2, None, None);
                                }));
                                if let Some(w) = cx.active_window() {
                                    let _ = w.update(cx, |_, window, cx| pick(file, window, cx));
                                }
                            });
                        }),
                    )
                    .child(crate::os_ui::modal_cancel(crate::os_ui::cancel_button("pick-cancel", "Cancel", cx))),
            )
    });
}
