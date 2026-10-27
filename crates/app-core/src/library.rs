//! The local document library's entries (the start window's documents list, the drawing Insert view picker's Other
//! documents): what a scan of the `.noshape` files reports. The scan itself is the app's `start_library`.

use gpui_kit::RenderImage;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const EXT: &str = "noshape";

#[derive(Clone)]
pub struct DocEntry {
    pub path: PathBuf,
    /// Display name (store metadata name, else the file stem).
    pub name: String,
    /// Last change (unix ms): the store's `modified`, else the file mtime.
    pub modified: i64,
    /// Author of the latest change ("Modified by").
    pub modified_by: String,
    /// Author of the first change ("Owned by").
    pub owner: String,
    /// Document description (the "Description" column).
    pub description: String,
    /// The workspace it opens in (the row's "⚲ Main" chip).
    pub workspace: String,
    pub thumbnail: Option<Arc<RenderImage>>,
    /// Position in the recent list.
    pub recent_rank: Option<usize>,
    /// Could not be read as a noshape document.
    pub broken: bool,
    /// In the Trash (`path` is the trashed copy).
    pub trashed: bool,
}

impl DocEntry {
    pub fn folder(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new(""))
    }
}

/// A document's PNG thumbnail as a GPUI image (the white background transparent, as Onshape's rows).
pub fn decode_thumbnail(png: &[u8]) -> Option<Arc<RenderImage>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let mut rgba = img.to_rgba8();
    // GPUI render images are BGRA. The white background becomes
    // transparent so thumbnails sit on hovered and selected rows like
    // Onshape's.
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
        if px[0] >= 250 && px[1] >= 250 && px[2] >= 250 {
            *px = [0; 4];
        }
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}
