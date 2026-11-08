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
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::file_tab::{FileKind, FileTab};

/// The tab-bar / card icon of a file: Onshape's per-format art (live
/// tabbar/icons2-filetab-*: STEP, X_T, X_B, JT, STL, PVZ, 3MF, SolidWorks,
/// DXF/DWG, DWT, TXT (and MD, live 1756), ZIP have their own; IGES, SAT, 3DM, CATIA, Inventor,
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
        "txt" | "md" => return "file-txt",
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
    /// The file's view (decoded once per file data).
    preview: Option<(std::sync::Arc<str>, std::rc::Rc<Preview>)>,
    /// The PDF viewer's zoom (an index into [`PDF_ZOOMS`]) and scroll.
    pdf_zoom: usize,
    pdf_scroll: ScrollHandle,
    /// The PDF viewer's thumbnail sidebar (☰; open at first, as Chrome's).
    pdf_thumbnails: bool,
    /// The page a thumbnail click went to (its scroll applies at the next
    /// layout), until the view scrolls.
    pdf_jump: Option<usize>,
}

/// What a file tab shows (Onshape views images, PDF, text, Markdown, CSV
/// and JSON; help "Supported File Formats", Non-CAD files; live behaviours
/// "dx (2026-11-02, P3 formats.md non-CAD viewers)"): the picture, a PDF's
/// pages, the text, else the file's card. Video needs a decoder
/// (docs/parity/formats.md).
enum Preview {
    Image(std::sync::Arc<gpui_kit::Image>),
    /// A PDF's first pages (rendered in the background: None until then),
    /// or why it can't be shown.
    Pages(Option<Result<PdfPages, String>>),
    /// TXT and Markdown, raw (live doesn't render Markdown).
    Text(String),
    /// JSON as written, coloured, with line numbers (live: a read-only Ace editor).
    Json(String),
    /// CSV rows (the first [`LINE_LIMIT`]) and the file's row count.
    Table(Vec<Vec<String>>, usize),
    Card,
}

/// A PDF's first pages (each with its size in points) and its page count.
struct PdfPages {
    pages: Vec<(std::sync::Arc<gpui_kit::Image>, [f32; 2])>,
    total: usize,
}

/// A PDF page's pixels per point (1: 72 dpi): sharp up to 112 % zoom.
const PDF_SCALE: f32 = 1.5;

/// The PDF viewer's zoom steps (Chrome's, 100 %: 96 px per inch).
const PDF_ZOOMS: [f32; 11] = [0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5];

/// Text read at most, and lines shown (a larger file shows its start).
const TEXT_LIMIT: usize = 1 << 20;
const LINE_LIMIT: usize = 5000;

fn preview_of(f: &FileTab) -> Preview {
    let ext = f.ext();
    let image = match ext.as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "gif" => Some(ImageFormat::Gif),
        "svg" => Some(ImageFormat::Svg),
        "bmp" => Some(ImageFormat::Bmp),
        "tif" | "tiff" => Some(ImageFormat::Tiff),
        "webp" => Some(ImageFormat::Webp),
        _ => None,
    };
    if ext == "pdf" {
        return Preview::Pages(None);
    }
    let text = matches!(ext.as_str(), "txt" | "md" | "csv" | "json");
    if image.is_none() && !text {
        return Preview::Card;
    }
    let Some(bytes) = f.bytes() else { return Preview::Card };
    if let Some(format) = image {
        return Preview::Image(std::sync::Arc::new(gpui_kit::Image::from_bytes(format, bytes)));
    }
    let raw = String::from_utf8_lossy(&bytes[..bytes.len().min(TEXT_LIMIT)]).into_owned();
    match ext.as_str() {
        "json" => return Preview::Json(raw),
        "csv" => {
            let mut rows = csv_rows(&raw);
            let total = rows.len();
            rows.truncate(LINE_LIMIT);
            return Preview::Table(rows, total);
        }
        _ => {}
    }
    Preview::Text(raw)
}

/// CSV records (RFC 4180: quoted fields may hold commas, quotes as "" and
/// line breaks; CRLF or LF). A final line break starts an empty last row
/// (live's grid shows it).
fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let (mut row, mut field) = (Vec::new(), String::new());
    let (mut quoted, mut chars) = (false, text.chars().peekable());
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            (true, '"') => quoted = false,
            (true, c) => field.push(c),
            (false, '"') if field.is_empty() => quoted = true,
            (false, ',') => row.push(std::mem::take(&mut field)),
            (false, '\r') => {}
            (false, '\n') => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            (false, c) => field.push(c),
        }
    }
    if !text.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// A JSON line's colours (Ace's: strings and keys blue, numbers and
/// true / false / null dark blue).
fn json_highlights(line: &str) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    let colour = |c: u32| HighlightStyle { color: Some(gpui_kit::rgb(c).into()), ..Default::default() };
    let b = line.as_bytes();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < b.len() {
        let start = i;
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i = (i + 1).min(b.len());
                out.push((start..i, colour(0x1a1aa6)));
            }
            b'-' | b'0'..=b'9' => {
                while i < b.len() && matches!(b[i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    i += 1;
                }
                out.push((start..i, colour(0x0000cd)));
            }
            b't' | b'f' | b'n' => {
                while i < b.len() && b[i].is_ascii_alphabetic() {
                    i += 1;
                }
                if matches!(&line[start..i], "true" | "false" | "null") {
                    out.push((start..i, colour(0x0000cd)));
                }
            }
            _ => i += 1,
        }
    }
    out
}

impl FileTabView {
    pub fn new(doc: Entity<DocModel>, tab: TabId, cx: &mut Context<Self>) -> Self {
        cx.observe(&doc, |_, _, cx| cx.notify()).detach();
        Self { doc, tab, preview: None, pdf_zoom: 7, pdf_scroll: ScrollHandle::new(), pdf_thumbnails: true, pdf_jump: None }
    }

    fn preview(&mut self, f: &FileTab, cx: &mut Context<Self>) -> std::rc::Rc<Preview> {
        match &self.preview {
            Some((data, p)) if std::sync::Arc::ptr_eq(data, &f.data_base64) => p.clone(),
            _ => {
                let p = std::rc::Rc::new(preview_of(f));
                self.preview = Some((f.data_base64.clone(), p.clone()));
                // A PDF's pages are rendered in the background.
                if matches!(*p, Preview::Pages(None)) {
                    let (data, bytes) = (f.data_base64.clone(), f.bytes());
                    cx.spawn(async move |this, cx| {
                        let pages = cx
                            .background_spawn(async move {
                                let bytes = bytes.ok_or_else(|| "the file data is damaged".to_string())?;
                                noshape_ops::file_preview::pdf_pages(&bytes, PDF_SCALE, 0, noshape_ops::file_preview::PDF_PAGE_LIMIT)
                            })
                            .await
                            .map(|(total, pngs)| PdfPages {
                                pages: pngs
                                    .into_iter()
                                    .map(|png| {
                                        // The page's size in points, from the PNG header (IHDR).
                                        let dim = |i: usize| png.get(i..i + 4).map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]])) as f32 / PDF_SCALE;
                                        let size = [dim(16), dim(20)];
                                        (std::sync::Arc::new(gpui_kit::Image::from_bytes(ImageFormat::Png, png)), size)
                                    })
                                    .collect(),
                                total,
                            });
                        let _ = this.update(cx, |v, cx| {
                            if v.preview.as_ref().is_some_and(|(d, _)| std::sync::Arc::ptr_eq(d, &data)) {
                                v.preview = Some((data, std::rc::Rc::new(Preview::Pages(Some(pages)))));
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                }
                p
            }
        }
    }

    /// A PDF as Chrome's viewer shows it in live's tab (dark theme): a bar
    /// with ☰, the title, the page box "1 / 2" and − 100 % +; the pages'
    /// thumbnails at the left (the current one outlined, a click goes to
    /// it); the pages one under the other.
    fn pdf_view(&self, f: &FileTab, pages: &Option<Result<PdfPages, String>>, cx: &mut Context<Self>) -> AnyElement {
        let white = gpui_kit::rgb_static(0xffffff);
        let body = || div().flex_1().min_h_0().bg(gpui_kit::rgb(0x282828));
        let note = |t: String| body().flex().items_center().justify_center().text_size(px(14.)).text_color(white).child(t).into_any_element();
        let pdf = match pages {
            None => return note("Rendering…".into()),
            Some(Err(e)) => return note(format!("This PDF can't be shown: {e}. Download it to open it.")),
            Some(Ok(p)) => p,
        };
        let zoom = PDF_ZOOMS[self.pdf_zoom];
        let (gap, px_per_pt) = (8.0, 96.0 / 72.0 * zoom);
        // Each page's top in the list, and the page at the top of the view.
        let tops: Vec<f32> = pdf.pages.iter().scan(gap, |y, (_, s)| {
            let top = *y;
            *y += s[1] * px_per_pt + gap;
            Some(top)
        }).collect();
        let top = -f32::from(self.pdf_scroll.offset().y);
        let current = self.pdf_jump.unwrap_or_else(|| tops.iter().zip(&pdf.pages).position(|(t, (_, s))| t + s[1] * px_per_pt + gap > top + 1.0).unwrap_or(0));
        let title = f.file_name.rsplit_once('.').map_or(f.file_name.clone(), |(t, _)| t.to_string());
        let round = |id: &'static str| div().id(id).size(px(32.)).flex().items_center().justify_center().rounded_full().cursor_pointer().hover(|d| d.bg(gpui_kit::rgb(0x505050)));
        let zoom_button = |id: &'static str, label: &'static str, step: isize| {
            round(id).text_size(px(20.)).child(label).on_click(cx.listener(move |v, _, _, cx| {
                v.pdf_zoom = v.pdf_zoom.saturating_add_signed(step).min(PDF_ZOOMS.len() - 1);
                cx.notify();
            }))
        };
        let field = |t: String| div().px(px(6.)).py(px(2.)).bg(gpui_kit::rgb(0x191919)).child(t);
        let bar = h_flex()
            .h(px(56.))
            .flex_none()
            .px(px(16.))
            .gap(px(10.))
            .bg(gpui_kit::rgb(0x3c3c3c))
            .text_color(white)
            .text_size(px(14.))
            .child(round("pdf-thumbnails").text_size(px(18.)).child("☰").on_click(cx.listener(|v, _, _, cx| {
                v.pdf_thumbnails = !v.pdf_thumbnails;
                cx.notify();
            })))
            .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(crate::ui::user_text(title)))
            .child(field(format!("{}", current + 1)))
            .child(format!("/  {}", pdf.total))
            .child(div().w(px(1.)).h(px(24.)).mx(px(6.)).bg(gpui_kit::rgb(0x6b6b6b)))
            .child(zoom_button("pdf-zoom-out", "−", -1))
            .child(field(format!("{}%", (zoom * 100.0).round())))
            .child(zoom_button("pdf-zoom-in", "+", 1))
            .child(div().flex_1());
        let thumbnails = self.pdf_thumbnails.then(|| {
            div()
                .id("pdf-thumbnails-list")
                .w(px(300.))
                .flex_none()
                .h_full()
                .overflow_y_scroll()
                .border_r_1()
                .border_color(gpui_kit::rgb(0x5f6368))
                .child(v_flex().items_center().gap(px(16.)).py(px(18.)).children(pdf.pages.iter().enumerate().map(|(k, (i, s))| {
                    let y = tops[k];
                    v_flex()
                        .id(("pdf-thumbnail", k))
                        .items_center()
                        .gap(px(6.))
                        .cursor_pointer()
                        .text_size(px(13.))
                        .text_color(white)
                        .child(div().p(px(4.)).when(k == current, |d| d.bg(gpui_kit::rgb(0x8ab4f8))).child(img(i.clone()).flex_none().w(px(112.)).h(px(112. * s[1] / s[0].max(1.)))))
                        .child((k + 1).to_string())
                        .on_click(cx.listener(move |v, _, _, cx| {
                            v.pdf_scroll.set_offset(point(px(0.), px(-(y - gap))));
                            v.pdf_jump = Some(k);
                            cx.notify();
                        }))
                })))
        });
        let list = div()
            .id("file-pages")
            .flex_1()
            .h_full()
            .min_w_0()
            .overflow_scroll()
            .track_scroll(&self.pdf_scroll)
            .on_scroll_wheel(cx.listener(|v, _, _, cx| {
                v.pdf_jump = None;
                cx.notify();
            }))
            .child(
                v_flex()
                    .min_w_full()
                    .items_center()
                    .gap(px(gap))
                    .py(px(gap))
                    .children(pdf.pages.iter().map(|(i, s)| img(i.clone()).flex_none().w(px(s[0] * px_per_pt)).h(px(s[1] * px_per_pt))))
                    .when(pdf.total > pdf.pages.len(), |d| d.child(div().text_size(px(13.)).text_color(white).child(format!("… {} more pages (Download for the whole file)", pdf.total - pdf.pages.len())))),
            );
        v_flex().size_full().child(bar).child(body().flex().children(thumbnails).child(list)).into_any_element()
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
        let preview = self.preview(&f, cx);
        if !matches!(*preview, Preview::Card) {
            // A viewable file fills the tab, no bar (live: Download and
            // Update… are in the tab's menu).
            let mono = |d: Stateful<Div>| d.size_full().overflow_y_scroll().bg(gpui_kit::rgb_static(0xffffff)).font_family("Inconsolata").text_size(px(13.)).text_color(gpui_kit::rgb_static(0x000000));
            return match &*preview {
                // At its size from the top left, on white (live).
                Preview::Image(image) => div().id("file-image").size_full().overflow_scroll().bg(gpui_kit::rgb_static(0xffffff)).child(img(image.clone()).flex_none()).into_any_element(),
                Preview::Pages(pages) => self.pdf_view(&f, pages, cx),
                // The browser's plain-text view (live, Markdown too): monospace, wrapped.
                Preview::Text(text) => mono(div().id("file-text"))
                    .p(px(8.))
                    .whitespace_normal()
                    .children(text.lines().take(LINE_LIMIT).map(|l| div().min_h(px(16.)).child(crate::ui::user_text(l.to_string()))))
                    .when(text.lines().count() > LINE_LIMIT, |d| d.child(div().pt(px(8.)).text_color(pal.text_muted).child(format!("… {} more lines (Download for the whole file)", text.lines().count() - LINE_LIMIT))))
                    .into_any_element(),
                // A read-only code view (live's Ace editor): line numbers,
                // JSON colours, the text as written.
                Preview::Json(text) => mono(div().id("file-json"))
                    .child(
                        div()
                            .flex()
                            .children(text.lines().take(LINE_LIMIT).enumerate().map(|(i, l)| {
                                h_flex()
                                    .w_full()
                                    .items_start()
                                    .child(div().w(px(40.)).flex_none().pr(px(6.)).bg(gpui_kit::rgb(0xebebeb)).text_color(gpui_kit::rgb(0x333333)).flex().justify_end().child((i + 1).to_string()))
                                    .child(div().flex_1().min_w_0().pl(px(4.)).whitespace_normal().child(StyledText::new(l.to_string()).with_highlights(json_highlights(l))))
                            }))
                            .flex_col(),
                    )
                    .into_any_element(),
                // A spreadsheet-like grid (live's csv-viewer-table): column
                // numbers above, row numbers beside, every line a row.
                Preview::Table(rows, total) => {
                    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0).min(200);
                    let widths: Vec<f32> = (0..cols).map(|c| rows.iter().filter_map(|r| r.get(c)).map(|v| v.chars().count()).max().unwrap_or(0).clamp(2, 40) as f32 * 7.5 + 12.).collect();
                    let num_w = (rows.len().to_string().len() as f32 * 7.5 + 12.).max(24.);
                    let (border, head) = (gpui_kit::rgb(0xcccccc), gpui_kit::rgb(0xeeeeee));
                    let cell = |w: f32| div().w(px(w)).flex_none().h(px(22.)).px(px(5.)).flex().items_center().border_r_1().border_b_1().border_color(border).truncate();
                    div()
                        .id("file-table")
                        .size_full()
                        .overflow_scroll()
                        .bg(gpui_kit::rgb_static(0xffffff))
                        .child(
                            v_flex()
                                .w(px(num_w + widths.iter().sum::<f32>() + 1.))
                                .border_l_1()
                                .border_t_1()
                                .border_color(border)
                                .text_size(px(13.))
                                .text_color(gpui_kit::rgb_static(0x000000))
                                .child(h_flex().bg(head).child(cell(num_w)).children(widths.iter().enumerate().map(|(c, w)| cell(*w).justify_center().child((c + 1).to_string()))))
                                .children(rows.iter().enumerate().map(|(i, r)| {
                                    h_flex().child(cell(num_w).bg(head).justify_center().child((i + 1).to_string())).children(widths.iter().enumerate().map(|(c, w)| cell(*w).child(crate::ui::user_text(r.get(c).cloned().unwrap_or_default()))))
                                }))
                                .when(*total > rows.len(), |d| d.child(div().pt(px(8.)).text_color(pal.text_muted).child(format!("… {} more rows (Download for the whole file)", total - rows.len())))),
                        )
                        .into_any_element()
                }
                Preview::Card => div().into_any_element(),
            };
        }
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

/// The insert-from-file dialog's state: Current document / Other
/// documents, the library's documents (read once) and the opened one's
/// files.
/// The pickers' "Import…" icon: an arrow up onto a bar (live drawing/984).
const GLYPH_UPLOAD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor"><path d="M8 1.5 3 7h3.2v4.5h3.6V7H13z"/><rect x="2.5" y="13" width="11" height="1.8"/></svg>"#;

#[derive(Default)]
struct PickState {
    other: bool,
    docs: Option<Vec<crate::library::DocEntry>>,
    opened: Option<(String, Vec<(String, FileTab)>)>,
}

/// Onshape's insert-from-file dialog (Insert image, Insert DXF or DWG,
/// Select a DWT): search the document's file tabs of `kinds`, click one to
/// insert it; "Import" brings a local file into the document (as a file
/// tab) and inserts it. "Other documents" lists the library's documents;
/// open one to pick from its files (live: Insert image / Select a DWT
/// have Current document | Other documents, drawing.md EX17, S13).
/// The open file picker's search box (the parity runner's focus: a field while it has the keyboard).
struct PickerSearch(Option<WeakEntity<InputState>>);
impl Global for PickerSearch {}

/// Whether the file picker's search box has the keyboard.
pub fn picker_search_focused(window: &Window, cx: &App) -> bool {
    cx.try_global::<PickerSearch>().and_then(|p| p.0.as_ref()).and_then(|w| w.upgrade()).is_some_and(|s| s.read(cx).focus_handle(cx).is_focused(window))
}

pub fn pick_file(doc: Entity<DocModel>, title: &'static str, kinds: &'static [FileKind], exts: &'static [&'static str], window: &mut Window, cx: &mut App, on_pick: impl Fn(FileTab, &mut Window, &mut App) + 'static) {
    let search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search files"));
    // Live (live2 drw-insert-image): the search box has the keyboard as the picker opens.
    cx.set_global(PickerSearch(Some(search.downgrade())));
    let focus = search.clone();
    window.defer(cx, move |window, cx| focus.update(cx, |s, cx| s.focus(window, cx)));
    let state = cx.new(|_| PickState::default());
    let on_pick = std::rc::Rc::new(on_pick);
    // This document isn't one of the others (its path may be relative,
    // opened from a shell).
    let here = doc.read(cx).session.path.clone();
    let here = std::fs::canonicalize(&here).unwrap_or(here);
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let q = search.read(cx).value().to_lowercase();
        let st = state.read(cx);
        let other = st.other;
        let tab = |id: &'static str, label: &'static str, on: bool, to_other: bool| {
            let state = state.clone();
            // Live (drawing/984): two halves, the active one blue with a 2 px
            // underline.
            div()
                .id(id)
                .map(|d| crate::ui_script::mark(d, || format!("tab:{label}")))
                .flex_1()
                .h(px(32.))
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .when(on, |d| d.text_color(gpui_kit::rgb(0x1651b0)).font_weight(FontWeight::MEDIUM).child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(gpui_kit::rgb(0x1651b0))))
                .when(!on, |d| d.text_color(pal.text))
                .child(label)
                .on_click(move |_, _, cx| {
                    let here = state.clone();
                    let load = state.update(cx, |s, cx| {
                        s.other = to_other;
                        cx.notify();
                        to_other && s.docs.is_none()
                    });
                    if load {
                        here.update(cx, |s, _| s.docs = Some(Vec::new()));
                        cx.spawn(async move |cx| {
                            let docs = cx.background_spawn(async { crate::start_library::Library::default().scan(&crate::start_library::ScanInput::current()).docs }).await;
                            here.update(cx, |s, cx| {
                                s.docs = Some(docs);
                                cx.notify();
                            });
                        })
                        .detach();
                    }
                })
        };
        let tabs = h_flex().flex_none().border_b_1().border_color(gpui_kit::rgb(0xeeeeee)).child(tab("pick-tab-current", "Current document", !other, false)).child(tab("pick-tab-other", "Other documents", other, true));
        let mut list = v_flex().id("pick-file-list").flex_1().min_h_0().overflow_y_scroll();
        let row = |id: SharedString| h_flex().id(id).h(px(32.)).px(px(8.)).gap(px(8.)).text_size(px(13.)).cursor_pointer().hover(|d| d.bg(gpui_kit::rgb(0xdef1fb)));
        let files: Vec<(TabId, String, FileTab)> = if other {
            match &st.opened {
                // An opened other document: ‹ back, then its files.
                Some((name, files)) => {
                    let (back, search) = (state.clone(), search.clone());
                    list = list.child(row("pick-back".into()).child(div().child("‹")).child(div().flex_1().truncate().font_weight(FontWeight::SEMIBOLD).child(crate::ui::user_text(name.clone()))).on_click(move |_, window, cx| {
                        search.update(cx, |s, cx| s.set_value("", window, cx));
                        back.update(cx, |s, cx| {
                            s.opened = None;
                            cx.notify();
                        })
                    }));
                    files.iter().filter(|(n, _)| n.to_lowercase().contains(&q)).map(|(n, f)| (TabId(0), n.clone(), f.clone())).collect()
                }
                None => {
                    let docs: Vec<_> = st.docs.iter().flatten().filter(|d| !d.broken && !d.trashed && d.path != here && d.name.to_lowercase().contains(&q)).cloned().collect();
                    if st.docs.as_ref().is_some_and(|d| d.is_empty()) {
                        list = list.child(div().p(px(12.)).text_size(px(13.)).text_color(pal.text_muted).child("Loading documents…"));
                    }
                    for (i, d) in docs.into_iter().enumerate() {
                        let (open, search) = (state.clone(), search.clone());
                        list = list.child(
                            row(SharedString::from(format!("pick-doc-{i}")))
                                .child(crate::assets::icon("document", 18., cx))
                                .child(div().flex_1().truncate().child(crate::ui::user_text(d.name.clone())))
                                .child(div().text_size(px(12.)).text_color(pal.text_muted).child(d.owner.clone()))
                                .on_click(move |_, window, cx| {
                                    // The search then filters its files.
                                    search.update(cx, |s, cx| s.set_value("", window, cx));
                                    // Its workspace as it is now: the files of `kinds`.
                                    let files: Vec<(String, FileTab)> = (|| {
                                        let store = noshape_store::Document::open_read_only(&d.path).ok()?;
                                        let head = store.head(store.default_branch().ok()?).ok()?;
                                        let snap = crate::derived::external_snapshot(&d.path, head).ok()?;
                                        Some(snap.ordered_tabs().into_iter().filter_map(|t| match &t.content {
                                            TabContent::File(f) if kinds.contains(&f.kind()) => Some((t.name.clone(), f.clone())),
                                            _ => None,
                                        }).collect())
                                    })()
                                    .unwrap_or_default();
                                    open.update(cx, |s, cx| {
                                        s.opened = Some((d.name.clone(), files));
                                        cx.notify();
                                    });
                                }),
                        );
                    }
                    Vec::new()
                }
            }
        } else {
            doc.read(cx)
                .session
                .doc
                .tabs
                .iter()
                .filter_map(|t| match &t.content {
                    TabContent::File(f) if kinds.contains(&f.kind()) && t.name.to_lowercase().contains(&q) => Some((t.id, t.name.clone(), f.clone())),
                    _ => None,
                })
                .collect()
        };
        if files.is_empty() && !other {
            list = list.child(div().p(px(12.)).text_size(px(13.)).text_color(pal.text_muted).child("No files available."));
        }
        if files.is_empty() && other && st.opened.is_some() {
            list = list.child(div().p(px(12.)).text_size(px(13.)).text_color(pal.text_muted).child("No files of this type in this document."));
        }
        let mut texts: Vec<String> = vec![title.into(), "Current document".into(), "Other documents".into()];
        let names: Vec<String> = files.iter().map(|(_, n, _)| n.clone()).collect();
        for (k, (id, name, f)) in files.into_iter().enumerate() {
            let (pick, f2) = (on_pick.clone(), f.clone());
            let mark = name.clone();
            // Live: 40 px rows, the file's icon, its name (no size).
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("pick-{id}-{k}")))
                    .map(|d| crate::ui_script::mark(d, || format!("list_row:{mark}")))
                    .h(px(40.))
                    .flex_none()
                    .px(px(6.))
                    .gap(px(10.))
                    .text_size(px(13.))
                    .text_color(pal.text)
                    .cursor_pointer()
                    .hover(|d| d.bg(gpui_kit::rgb(0xdef1fb)))
                    .tooltip(crate::ui::tooltip(f.size_label()))
                    .child(div().w(px(52.)).flex_none().flex().justify_center().child(crate::assets::icon(file_icon(&f), 28., cx)))
                    .child(div().flex_1().truncate().child(crate::ui::user_text(name)))
                    .on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        pick(f2.clone(), window, cx);
                    }),
            );
        }
        let (doc2, pick2) = (doc.clone(), on_pick.clone());
        // Live (drawing/123, 984): a 300 px flyout under the toolbar at the
        // left (beside Sheet properties), not a centred modal: title and ✕,
        // the tabs, this document's name and branch, Search files, the list,
        // and "⤒ Import…" at the bottom.
        let header = h_flex()
            .h(px(30.))
            .flex_none()
            .pl(px(6.))
            .pr(px(6.))
            .border_b_1()
            .border_color(gpui_kit::rgb(0xd7d7d7))
            .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(title))
            .child(div().flex_none().child(gpui_kit::component::dialog::DialogClose::new().child(div().id("pick-close").size(px(20.)).flex().items_center().justify_center().cursor_pointer().text_color(pal.text_faint).hover(|d| d.text_color(pal.text_muted)).child(gpui_kit::component::Icon::empty().data(crate::os_ui::GLYPH_MODAL_CLOSE).size(px(14.))))));
        let (doc_name, branch) = {
            let d = doc.read(cx);
            (d.name(), d.session.store.branch(d.session.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into()))
        };
        if !other {
            texts.extend([doc_name.clone(), branch.clone()]);
        }
        texts.extend(names);
        texts.push("Import…".into());
        // The panel's texts for the ui-parity runner (README "### Drawings": live's is a flyout of the drawing).
        crate::ui_script::note_dialog(|| serde_json::json!({ "kind": "drawing-panel", "title": title, "text": texts }));
        let document_row = h_flex().h(px(40.)).flex_none().px(px(6.)).child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(div().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).line_height(px(19.)).text_color(pal.text).child(crate::ui::user_text(doc_name)))
                .child(h_flex().gap_1().opacity(0.5).text_size(px(12.)).line_height(px(15.6)).text_color(pal.text).child(crate::ui::glyph("branch", 10., pal.text)).child(branch)),
        );
        let search_row = div().flex_none().p(px(6.)).child(Input::new(&search).h(px(30.)).text_size(px(13.)));
        dialog
            .w(px(300.))
            .margin_left(px(290.))
            .margin_top(px(78.))
            .overlay(false)
            .close_button(false)
            .p_0()
            .gap_0()
            .rounded(px(0.))
            .border_0()
            .bg(pal.panel)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .child(
                v_flex()
                    .h(px(584.))
                    .child(header)
                    .child(tabs)
                    .when(!other, |d| d.child(document_row))
                    .child(search_row)
                    .child(list)
                    .child(
                        h_flex()
                            .id("pick-import")
                            .h(px(24.))
                            .flex_none()
                            .px(px(6.))
                            .gap(px(6.))
                            .border_t_1()
                            .border_color(gpui_kit::rgb(0xeaeaea))
                            .text_size(px(13.))
                            .text_color(pal.text)
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .child(gpui_kit::component::Icon::empty().data(GLYPH_UPLOAD).size(px(13.)))
                            .child("Import…")
                            .on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            // This window, not the active one (none while
                            // the path prompt closes, or without a window
                            // manager focusing it).
                            let (doc, pick, here) = (doc2.clone(), pick2.clone(), window.window_handle());
                            crate::file_dialogs::request_open_path(title, "Files", exts, window, cx, move |path, cx| {
                                let Ok(bytes) = std::fs::read(&path) else { return };
                                let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                                let file = FileTab::new(&name, &bytes, noshape_model::file_tab::now());
                                let f2 = file.clone();
                                doc.update(cx, |d, cx| d.edit_document(&format!("Import {name}"), cx, |d| {
                                    add_file_tab(d, f2, None, None);
                                }));
                                // After the prompt's own click (its window
                                // can't be updated from inside it).
                                cx.defer(move |cx| {
                                    let _ = here.update(cx, |_, window, cx| pick(file, window, cx));
                                });
                            });
                        }),
                    ),
            )
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn csv_rows_follow_rfc_4180() {
        let rows = super::csv_rows("a,b,c\r\n1,\"x, y\",\"say \"\"hi\"\"\"\n2,\"two\nlines\",\n");
        assert_eq!(rows, vec![vec!["a", "b", "c"], vec!["1", "x, y", "say \"hi\""], vec!["2", "two\nlines", ""], vec![""]]);
        assert_eq!(super::csv_rows("x"), vec![vec!["x"]]);
    }

    #[test]
    fn json_lines_are_coloured_by_token() {
        let line = r#"{"a": "x\"y", "n": -1.5e3, "t": true}"#;
        let spans: Vec<&str> = super::json_highlights(line).into_iter().map(|(r, _)| &line[r]).collect();
        assert_eq!(spans, [r#""a""#, r#""x\"y""#, r#""n""#, "-1.5e3", r#""t""#, "true"]);
    }
}
