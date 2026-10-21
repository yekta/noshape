//! Offscreen rendering to PNG: the viewport's look (style from the Graphics
//! preferences) and the thumbnail's framing, without a window.

use crate::graphics_prefs::GraphicsPrefs;
use glam::DVec3;
use noshape_render::{Camera, RenderParams, Renderer, Scene, StandardView, Style};

/// The 3D viewport's style for the Graphics preferences (what the Part
/// Studio view applies live).
pub fn viewport_style(g: &GraphicsPrefs) -> Style {
    let s = crate::graphics_prefs::lin3(g.specular);
    Style { specular: [s[0], s[1], s[2], g.shininess], construction_dash: g.construction, ambient_occlusion: g.ambient_occlusion, ..Style::default() }
}

/// A camera looking from `view` that frames the box tightly (projected
/// extents with about 10 % margin), like document thumbnails.
pub fn fit_camera(lo: DVec3, hi: DVec3, aspect: f64, view: Option<StandardView>) -> Camera {
    let mut camera = Camera::default();
    if let Some(v) = view {
        camera.set_view(v);
    }
    let (right, up) = (camera.right(), camera.up());
    let center = (lo + hi) * 0.5;
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for i in 0..8 {
        let c = DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z }) - center;
        let (x, y) = (c.dot(right), c.dot(up));
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    let target = center + right * ((x0 + x1) * 0.5) + up * ((y0 + y1) * 0.5);
    let half_h = ((y1 - y0) * 0.5).max((x1 - x0) * 0.5 / aspect).max(1e-5);
    // fit_sphere adds a 15 % margin; keep about 10 %.
    camera.fit_sphere(target, half_h * 1.1 / 1.15, aspect.max(1.0));
    camera
}

/// Supersampling factor of PNG renders (as the app's `Gpu::render_png`).
pub const SUPERSAMPLE: u32 = 2;

/// Convert a rendered BGRA frame (sRGB) to an opaque PNG of `width` x
/// `height`, downsampling a supersampled frame.
pub fn encode_png(bgra: Vec<u8>, frame_w: u32, frame_h: u32, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut px = bgra;
    for c in px.as_chunks_mut::<4>().0 {
        c.swap(0, 2);
        c[3] = 255;
    }
    let full = image::RgbaImage::from_raw(frame_w, frame_h, px).ok_or_else(|| "frame size mismatch".to_string())?;
    let img = if frame_w == width && frame_h == height { full } else { image::imageops::resize(&full, width, height, image::imageops::FilterType::Triangle) };
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(png)
}

/// A headless renderer (its own GPU device; a software adapter on machines
/// without a GPU).
pub struct Offscreen {
    pub renderer: Renderer,
    pub cache: crate::builder::GpuCache,
}

impl Offscreen {
    pub fn new() -> Result<Self, String> {
        Ok(Offscreen { renderer: Renderer::new().map_err(|e| format!("no GPU adapter for rendering: {e}"))?, cache: Default::default() })
    }

    /// Render a scene to PNG bytes.
    pub fn render_png(&mut self, scene: &Scene, camera: &Camera, style: &Style, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let frame = self
            .renderer
            .render(RenderParams { scene, camera, width: width * SUPERSAMPLE, height: height * SUPERSAMPLE, device_pixel_ratio: SUPERSAMPLE as f32, style, pick: false })
            .map_err(|e| e.to_string())?;
        encode_png(frame.bgra, frame.width, frame.height, width, height)
    }
}

/// An inserted drawing image's pixels (RGB) from its base64 data (PDF
/// export embeds them).
pub fn decode_rgb(data_base64: &str) -> Option<(u32, u32, std::sync::Arc<Vec<u8>>)> {
    let bytes = noshape_model::sketch_feature::base64_decode(data_base64)?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgb8();
    let (w, h) = img.dimensions();
    Some((w, h, std::sync::Arc::new(img.into_raw())))
}
