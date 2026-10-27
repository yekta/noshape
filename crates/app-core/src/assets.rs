//! Embedded application assets (noshape's CAD icons) layered over GPUI Kit's
//! component icons.

use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

#[derive(rust_embed::RustEmbed)]
#[folder = "../app/assets"]
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
        let mut v: Vec<SharedString> = AppIcons::iter().filter(|p| p.starts_with(path) && !p.starts_with("icons/dark/")).map(|p| p.to_string().into()).collect();
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

/// Rasterized icons by (name, pixel size, dark theme).
type IconCache = HashMap<(String, u32, bool), Arc<RenderImage>>;

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
    // Theming › Dark mode (not inside the drawing editor, which stays light).
    let dark = gpui_kit::hex_theme_active();
    let key = (name.to_string(), device_px, dark);
    if let Some(img) = ICON_CACHE.lock().ok()?.get(&key) {
        return Some(img.clone());
    }
    let (name, file) = match AppIcons::get(&format!("icons/{name}.svg")) {
        Some(f) => (name, f),
        None => ("part", AppIcons::get("icons/part.svg")?),
    };
    let data = if dark { dark_icon(name, file.data) } else { file.data };
    // The icons are drawn on a 24-unit canvas; render it at the target size.
    let image = cx.svg_renderer().render_single_frame(&data, device_px as f32 / 24.0).ok()?;
    ICON_CACHE.lock().ok()?.insert(key, image.clone());
    Some(image)
}

/// The dark theme's art for an icon: `icons/dark/<name>.svg` where one is
/// drawn, else the light art with Onshape's themed icon colours flipped
/// (`--os-icon-outline-primary` #333 → #fff, `--os-icon-accent-primary`
/// #1651b0 → #3e94ff; live dom/icons2-dark-icon-fills.json). Icons whose
/// Onshape symbol uses the `--static` colours keep light faces in dark,
/// which only drawn art gives.
fn dark_icon(name: &str, light: Cow<'static, [u8]>) -> Cow<'static, [u8]> {
    match AppIcons::get(&format!("icons/dark/{name}.svg")) {
        Some(f) => f.data,
        None => Cow::Owned(flip_themed_colors(&light)),
    }
}

/// `svg` with the hex colours Onshape themes replaced by their dark values.
/// `<mask>` contents keep theirs: their black is "cut out", not ink.
fn flip_themed_colors(svg: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(svg.len());
    let mut i = 0;
    let mut in_mask = false;
    while i < svg.len() {
        if svg[i..].starts_with(b"<mask") {
            in_mask = true;
        } else if svg[i..].starts_with(b"</mask>") {
            in_mask = false;
        }
        if svg[i] == b'#' && !in_mask {
            let n = svg[i + 1..].iter().take_while(|c| c.is_ascii_hexdigit()).count();
            let to: Option<&[u8]> = match svg[i + 1..i + 1 + n].to_ascii_lowercase().as_slice() {
                b"333" | b"333333" | b"000" | b"000000" => Some(b"ffffff"),
                b"1651b0" => Some(b"3e94ff"),
                _ => None,
            };
            if let Some(to) = to {
                out.push(b'#');
                out.extend_from_slice(to);
                i += 1 + n;
                continue;
            }
        }
        out.push(svg[i]);
        i += 1;
    }
    out
}

/// A full-color app icon at `size` logical pixels. `name` is an icon name
/// ("extrude") or an asset path from [`icon_path`].
pub fn icon(name: &str, size: f32, cx: &App) -> gpui_kit::AnyElement {
    match icon_image(name, size, cx) {
        Some(image) => img(ImageSource::Render(image)).size(px(size)).flex_none().into_any_element(),
        None => gpui_kit::div().size(px(size)).flex_none().into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn flip_themed_colors_maps_only_whole_themed_hex() {
        let svg = br##"<path fill="#333" stroke="#333333"/><path fill="#1651B0" stroke="#3333aa"/><g fill="#999"/>"##;
        let out = String::from_utf8(super::flip_themed_colors(svg)).unwrap();
        assert_eq!(out, r##"<path fill="#ffffff" stroke="#ffffff"/><path fill="#3e94ff" stroke="#3333aa"/><g fill="#999"/>"##);
    }

    #[test]
    fn flip_themed_colors_keeps_mask_cutouts() {
        let svg = br##"<mask id="m"><rect fill="#fff"/><path fill="#000"/></mask><circle fill="#333" mask="url(#m)"/>"##;
        let out = String::from_utf8(super::flip_themed_colors(svg)).unwrap();
        assert_eq!(out, r##"<mask id="m"><rect fill="#fff"/><path fill="#000"/></mask><circle fill="#ffffff" mask="url(#m)"/>"##);
    }
}
