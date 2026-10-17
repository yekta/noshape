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
