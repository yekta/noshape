//! ☰ › Print… in a Part Studio or an Assembly (live studios 2349): the Print setup's papers, the dashed frame over
//! the graphics area, the framed view as a PNG at about 300 dpi (Download image) and on a PDF page (Print).

use noshape_render::{Camera, Projection, Style};
use serde::{Deserialize, Serialize};

const MM: f64 = 1.0 / 25.4;

/// The Paper select's options and their portrait size in inches, in live's order (Letter first, the default).
pub const PAPERS: [(&str, f64, f64); 17] = [
    ("Letter", 8.5, 11.0),
    ("Legal", 8.5, 14.0),
    ("Tabloid", 11.0, 17.0),
    ("A0", 841.0 * MM, 1189.0 * MM),
    ("A1", 594.0 * MM, 841.0 * MM),
    ("A2", 420.0 * MM, 594.0 * MM),
    ("A3", 297.0 * MM, 420.0 * MM),
    ("A4", 210.0 * MM, 297.0 * MM),
    ("A5", 148.0 * MM, 210.0 * MM),
    ("A6", 105.0 * MM, 148.0 * MM),
    ("A7", 74.0 * MM, 105.0 * MM),
    ("A8", 52.0 * MM, 74.0 * MM),
    ("A9", 37.0 * MM, 52.0 * MM),
    ("A10", 26.0 * MM, 37.0 * MM),
    ("ANSI C", 17.0, 22.0),
    ("ANSI D", 22.0, 34.0),
    ("ANSI E", 34.0, 44.0),
];

/// The Orientation select's options.
pub const ORIENTATIONS: [&str; 2] = ["Portrait", "Landscape"];

/// Download image's resolution: live's Letter Portrait is 2547 × 3296 px, A4 Landscape 3504 × 2477.
pub const DPI: f64 = 299.64;

/// The frame's share of the graphics area (live: 797 of 886 px high, centred).
const FRAME_FILL: f64 = 0.9;

/// The largest frame the GPU renders (wgpu's default texture limit).
pub const MAX_SIDE: u32 = 8192;

/// The paper and orientation chosen last (live: remembered across tabs and reloads).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrintSetup {
    pub paper: String,
    pub orientation: String,
}

impl Default for PrintSetup {
    fn default() -> Self {
        PrintSetup { paper: PAPERS[0].0.into(), orientation: ORIENTATIONS[0].into() }
    }
}

impl PrintSetup {
    fn path() -> std::path::PathBuf {
        crate::config_dir().join("print_setup.json")
    }

    pub fn load() -> Self {
        std::fs::read(Self::path()).ok().and_then(|b| serde_json::from_slice::<PrintSetup>(&b).ok()).filter(|s| s.paper_index().is_some() && ORIENTATIONS.contains(&s.orientation.as_str())).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(b) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(Self::path(), b);
        }
    }

    /// A setup from the selects' labels; an unknown one names the select at fault.
    pub fn new(paper: &str, orientation: &str) -> Result<Self, &'static str> {
        let paper = PAPERS.iter().find(|(p, ..)| p.eq_ignore_ascii_case(paper)).ok_or("paper")?.0;
        let orientation = ORIENTATIONS.iter().find(|o| o.eq_ignore_ascii_case(orientation)).ok_or("orientation")?;
        Ok(PrintSetup { paper: paper.into(), orientation: orientation.to_string() })
    }

    pub fn paper_index(&self) -> Option<usize> {
        PAPERS.iter().position(|(p, ..)| *p == self.paper)
    }

    pub fn landscape(&self) -> bool {
        self.orientation == ORIENTATIONS[1]
    }

    /// The page's width and height in inches.
    pub fn page_inches(&self) -> (f64, f64) {
        let (_, w, h) = PAPERS[self.paper_index().unwrap_or(0)];
        if self.landscape() { (h, w) } else { (w, h) }
    }

    /// Download image's pixel size.
    pub fn image_size(&self) -> (u32, u32) {
        let (w, h) = self.page_inches();
        ((w * DPI).round() as u32, (h * DPI).round() as u32)
    }

    /// The dashed frame in a graphics area of `area` (width, height): its left, top, width and height, centred,
    /// the paper's shape, 90 % of the area where it fits tighter.
    pub fn frame(&self, area: (f64, f64)) -> (f64, f64, f64, f64) {
        let (pw, ph) = self.page_inches();
        let scale = FRAME_FILL * (area.0 / pw).min(area.1 / ph);
        let (w, h) = (pw * scale, ph * scale);
        ((area.0 - w) / 2.0, (area.1 - h) / 2.0, w, h)
    }
}

/// The camera that shows only what the frame does: the area's camera narrowed to the frame's height (the frame is
/// centred, so the view's centre stays).
pub fn frame_camera(camera: &Camera, area_h: f64, frame_h: f64) -> Camera {
    let s = (frame_h / area_h.max(1.0)).clamp(1e-6, 1.0);
    let mut c = *camera;
    match c.projection {
        Projection::Orthographic => c.distance *= s,
        Projection::Perspective => c.fov_y = 2.0 * (s * (c.fov_y * 0.5).tan()).atan(),
    }
    c
}

/// The print's look: the view's style on white (live's downloaded image has no background gradient).
pub fn print_style(style: &Style) -> Style {
    Style { background_top: [1.0; 4], background_bottom: [1.0; 4], ..*style }
}

/// How to render a `width` × `height` image in one frame of at most [`MAX_SIDE`]: the size to render and the
/// supersampling (2 where it fits, as the app's PNGs; the largest papers render smaller and are scaled up).
pub fn render_plan(width: u32, height: u32) -> (u32, u32, u32) {
    let long = width.max(height).max(1);
    if 2 * long <= MAX_SIDE {
        return (width, height, 2);
    }
    let k = (MAX_SIDE as f64 / long as f64).min(1.0);
    (((width as f64 * k).round() as u32).max(1), ((height as f64 * k).round() as u32).max(1), 1)
}

/// How much thicker than on screen the print's lines and labels are: its pixels per pixel of the frame.
pub fn line_scale(rendered_height: u32, frame_height: f64) -> f32 {
    (rendered_height as f64 / frame_height.max(1.0)).max(1.0) as f32
}

/// `png` at exactly `width` × `height` (a capped render scaled up).
pub fn to_size(png: Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(&png).map_err(|e| e.to_string())?;
    if (img.width(), img.height()) == (width, height) {
        return Ok(png);
    }
    let img = image::imageops::resize(&img.to_rgba8(), width, height, image::imageops::FilterType::Triangle);
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Print's page: the image filling a PDF page of the paper's size.
pub fn page_pdf(png: &[u8], setup: &PrintSetup) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(png).map_err(|e| e.to_string())?.to_rgb8();
    let (w, h) = img.dimensions();
    let (pw, ph) = setup.page_inches();
    let (wmm, hmm) = (pw / MM, ph / MM);
    let picture = noshape_model::drawing::sheet::GeomImage { pos: [0.0, 0.0], size: [wmm, hmm], data_base64: "".into(), rgb: Some((w, h, std::sync::Arc::new(img.into_raw()))) };
    let mut page = noshape_model::drawing::sheet::SheetGeometry::default();
    (page.width, page.height, page.images) = (wmm, hmm, vec![picture]);
    Ok(noshape_model::drawing::export::to_pdf(&page))
}

/// The downloaded file's name: the tab's (live "Part Studio 1.png").
pub fn file_name(tab: &str, ext: &str) -> String {
    let safe: String = tab.chars().map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' }).collect();
    format!("{safe}.{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_and_sizes_as_live() {
        // Live 2349, a 1673 × 886 graphics area: Letter / ANSI E Portrait 616 × 797, A4 Portrait 564 × 797, Letter
        // Landscape 1032 × 797, A4 Landscape 1128 × 797.
        let area = (1673.0, 886.0);
        let frame = |p: &str, o: &str| {
            let (_, _, w, h) = PrintSetup::new(p, o).unwrap().frame(area);
            (w.round() as i64, h.round() as i64)
        };
        assert_eq!(frame("Letter", "Portrait"), (616, 797));
        assert_eq!(frame("ANSI E", "Portrait"), (616, 797));
        assert_eq!(frame("A4", "Portrait"), (564, 797));
        assert_eq!(frame("Letter", "Landscape"), (1032, 797));
        assert_eq!(frame("A4", "Landscape"), (1128, 797));
        assert_eq!(PrintSetup::default().image_size(), (2547, 3296));
        assert_eq!(PrintSetup::new("a4", "landscape").unwrap().image_size(), (3504, 2477));
        assert_eq!(PrintSetup::new("B5", "Portrait"), Err("paper"));
    }

    #[test]
    fn large_papers_render_within_the_gpus_limit() {
        assert_eq!(render_plan(2547, 3296), (2547, 3296, 2));
        let (w, h, ss) = render_plan(9921, 14031);
        assert_eq!((h, ss), (MAX_SIDE, 1));
        assert!((w as f64 / h as f64 - 9921.0 / 14031.0).abs() < 1e-3);
    }

    #[test]
    fn the_page_is_a_pdf_of_the_papers_size() {
        let mut png = Vec::new();
        image::RgbImage::from_pixel(4, 6, image::Rgb([255, 0, 0])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let pdf = page_pdf(&png, &PrintSetup::default()).unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-") && text.contains("/MediaBox [0 0 612.000 792.000]") && text.contains("/Subtype /Image /Width 4 /Height 6"), "{}", &text[..text.len().min(600)]);
    }
}
