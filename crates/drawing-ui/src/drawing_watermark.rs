//! Company settings › Release management › Watermarks on drawings (Help
//! enterprise_settings_release_management, image
//! release/example-drawing-watermark): the release state's text across the
//! middle of the sheet, rising along the sheet's diagonal. Live (the drawing
//! app's `OsWatermarkTracker`): Noto Sans filled `rgba(192,192,192,0.25)`
//! over the sheet's content, as wide as a quarter of the diagonal, centred
//! on the sheet with its baseline a sixth of the font size below the
//! diagonal; one per sheet.

use crate::doc_model::DocModel;
use noshape_model::document::TabId;
use noshape_model::drawing::sheet::{Anchor, Text2};

/// The fill, `rgba(192,192,192,0.25)` (blended in sRGB, as exports do).
pub const COLOR: [u8; 3] = [192, 192, 192];
pub const ALPHA: f32 = 0.25;

/// The editor's fill: [`COLOR`] at [`ALPHA`], live's `#efefef` over the
/// white sheet and `#303030` over black ink.
pub const EDITOR_COLOR: u32 = 0xc0c0c0;
pub const EDITOR_ALPHA: f32 = 0.25;

/// The watermark of a drawing tab, if any (the release settings and log are the app's:
/// `release_dialog::drawing_watermark_text`).
pub fn text(doc: &DocModel, tab: TabId) -> Option<String> {
    crate::drawing_hooks::watermark_text(doc, tab)
}

/// The watermark as sheet text, along the diagonal from the bottom left.
pub fn geometry(text: &str, width: f64, height: f64) -> Text2 {
    let diag = width.hypot(height);
    let angle = height.atan2(width);
    // Text width = diag / 4 (the font sized to fit it).
    let cap = (diag / 4.0) / noshape_model::drawing::font::text_width(text, 1.0).max(1e-9);
    // Noto Sans' cap height is 0.714 em; the baseline sits em / 6 below
    // the diagonal.
    let below = cap / 0.714 / 6.0;
    let (c, s) = (angle.cos(), angle.sin());
    Text2 { pos: [width / 2.0 + below * s, height / 2.0 - below * c], height: cap, text: text.to_string(), angle, anchor: Anchor::BottomCenter, pick: u32::MAX }
}

/// The watermark for an export (PDF, raster formats, print): its glyphs as
/// translucent triangles drawn over the sheet's content, as in the editor.
pub fn overlays(text: &str, width: f64, height: f64) -> Vec<(Vec<[f64; 2]>, [u8; 3], f32)> {
    let (pts, idx) = noshape_model::drawing::font::text_fill_font(&geometry(text, width, height), None);
    idx.as_chunks::<3>().0.iter().map(|t| (t.iter().map(|&i| pts[i as usize]).collect(), COLOR, ALPHA)).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_watermark_runs_along_the_diagonal() {
        let (w, h) = (279.4, 215.9);
        let t = super::geometry("In progress", w, h);
        assert!((t.angle.to_degrees() - 37.7).abs() < 0.1);
        let width = noshape_model::drawing::font::text_width(&t.text, t.height);
        assert!((width - w.hypot(h) / 4.0).abs() < 1e-6, "a quarter of the diagonal");
        // The baseline's middle is just below the sheet centre, across the diagonal.
        let (dx, dy) = (t.pos[0] - w / 2.0, t.pos[1] - h / 2.0);
        assert!(dy < 0.0 && (dx * t.angle.cos() + dy * t.angle.sin()).abs() < 1e-9);
        let tris = super::overlays("In progress", w, h);
        assert!(!tris.is_empty() && tris.iter().all(|(p, c, a)| p.len() == 3 && *c == [192; 3] && *a == 0.25));
    }
}
