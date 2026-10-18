//! Embedded application assets (noshape's CAD icons) layered over GPUI Kit's
//! component icons.

use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/*.svg"]
struct AppIcons;

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(f) = AppIcons::get(path) {
            return Ok(Some(f.data));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut v: Vec<SharedString> = AppIcons::iter().filter(|p| p.starts_with(path)).map(|p| p.to_string().into()).collect();
        v.extend(gpui_kit::assets::Assets.list(path)?);
        Ok(v)
    }
}

/// Whether an app icon with this name exists.
pub fn has_icon(name: &str) -> bool {
    AppIcons::get(&format!("icons/{name}.svg")).is_some()
}

/// Asset path of an app icon, falling back to a generic icon.
pub fn icon_path(name: &str) -> SharedString {
    if has_icon(name) {
        format!("icons/{name}.svg").into()
    } else {
        "icons/part.svg".into()
    }
}

// ------------------------------------------------------------------ icons
//
// noshape's icons are multi-color (Onshape style: gray line art with blue,
// green, red and yellow accents). GPUI's `svg()` / `Icon` render an SVG as
// a single-color mask, so full-color icons go through `img()` instead. The
// SVG is rasterized once per (icon, device pixel size) at exactly the size
// it is shown at, so 20 px toolbar icons stay crisp, and cached for the
// lifetime of the process.

use gpui_kit::{App, ImageSource, IntoElement, RenderImage, Styled, img, px};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

/// Rasterized icons by (name, pixel size).
type IconCache = HashMap<(String, u32), Arc<RenderImage>>;

static ICON_CACHE: LazyLock<Mutex<IconCache>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// Device scale factor of the most recently rendered window, x100.
static SCALE_X100: AtomicU32 = AtomicU32::new(100);

/// Record the window's scale factor (called by the document and start windows).
pub fn set_scale_factor(scale: f32) {
    SCALE_X100.store((scale * 100.0).round().max(100.0) as u32, Ordering::Relaxed);
}

fn icon_image(name: &str, size: f32, cx: &App) -> Option<Arc<RenderImage>> {
    // Accept both "extrude" and the asset path "icons/extrude.svg".
    let name = name.trim_start_matches("icons/").trim_end_matches(".svg");
    let scale = SCALE_X100.load(Ordering::Relaxed) as f32 / 100.0;
    let device_px = (size * scale).round().max(1.0) as u32;
    let key = (name.to_string(), device_px);
    if let Some(img) = ICON_CACHE.lock().ok()?.get(&key) {
        return Some(img.clone());
    }
    let file = AppIcons::get(&format!("icons/{name}.svg")).or_else(|| AppIcons::get("icons/part.svg"))?;
    // The icons are drawn on a 24-unit canvas; render it at the target size.
    let image = cx.svg_renderer().render_single_frame(&file.data, device_px as f32 / 24.0).ok()?;
    ICON_CACHE.lock().ok()?.insert(key, image.clone());
    Some(image)
}

/// A full-color app icon at `size` logical pixels. `name` is an icon name
/// ("extrude") or an asset path from [`icon_path`].
pub fn icon(name: &str, size: f32, cx: &App) -> gpui_kit::AnyElement {
    match icon_image(name, size, cx) {
        Some(image) => img(ImageSource::Render(image)).size(px(size)).flex_none().into_any_element(),
        None => gpui_kit::div().size(px(size)).flex_none().into_any_element(),
    }
}
