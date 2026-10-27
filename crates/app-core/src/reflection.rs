//! The Reflection analysis's settings and environment images (Analysis tools menu), shared by the Part Studio
//! and the Assembly.

use noshape_scene::environment_maps;
use std::sync::Arc;

/// An environment image: a library map or an image file (encoded bytes).
#[derive(Clone, Debug, PartialEq)]
pub enum EnvImageRef {
    Library(&'static str),
    Encoded { name: String, bytes: Arc<[u8]> },
}

impl EnvImageRef {
    pub fn name(&self) -> &str {
        match self {
            EnvImageRef::Library(n) => n,
            EnvImageRef::Encoded { name, .. } => name,
        }
    }

    /// Cache key of the decoded image.
    pub fn key(&self) -> String {
        match self {
            EnvImageRef::Library(n) => format!("lib:{n}"),
            EnvImageRef::Encoded { name, bytes } => format!("img:{name}:{}:{:p}", bytes.len(), bytes.as_ptr()),
        }
    }

    /// Decoded sRGB RGBA8 (width, height, pixels).
    pub fn decode(&self) -> Option<(u32, u32, Vec<u8>)> {
        match self {
            EnvImageRef::Library(n) => environment_maps::generate(n).map(|i| (i.width, i.height, i.rgba)),
            EnvImageRef::Encoded { bytes, .. } => {
                let img = image::load_from_memory(bytes).ok()?.to_rgba8();
                Some((img.width(), img.height(), img.into_raw()))
            }
        }
    }
}

/// Reflection analysis settings (they stay on after the dialog is accepted).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReflectionSettings {
    pub image: Option<EnvImageRef>,
    pub rotation_deg: f64,
    pub opposite: bool,
    pub show_edges: bool,
    pub blur: bool,
    pub lock: bool,
}
