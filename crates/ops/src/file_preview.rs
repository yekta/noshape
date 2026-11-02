//! File tabs' previews that need decoding: a PDF's pages (Onshape views
//! PDFs in their file tab; help "Supported File Formats", docs/parity/formats.md).

/// Pages rendered at most (a longer PDF shows its first pages).
pub const PDF_PAGE_LIMIT: usize = 50;

/// A PDF's page count and its pages `first..first + count` (at most
/// [`PDF_PAGE_LIMIT`]) as PNGs on white, `scale` pixels per point (1: 72 dpi).
pub fn pdf_pages(bytes: &[u8], scale: f32, first: usize, count: usize) -> Result<(usize, Vec<Vec<u8>>), String> {
    let pdf = hayro::hayro_syntax::Pdf::new(bytes.to_vec()).map_err(|e| format!("not a readable PDF ({e:?})"))?;
    let pages = pdf.pages();
    let cache = hayro::RenderCache::new();
    let settings = hayro::hayro_interpret::InterpreterSettings::default();
    let pixmap = hayro::PixmapSettings { x_scale: scale, y_scale: scale, bg_color: hayro::vello_cpu::color::palette::css::WHITE };
    let pngs = pages
        .iter()
        .skip(first)
        .take(count.min(PDF_PAGE_LIMIT))
        .map(|p| hayro::render(p, &cache, &settings, &hayro::RenderSettings::default(), &pixmap).into_png().map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((pages.len(), pngs))
}
