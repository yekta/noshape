//! The user's images library (a folder's Set library ▸ My images; live behaviours "dx (11-21 15:31)"): a sketch's
//! Insert image › Libraries lists it as "‹user› images" above Onshape images. Its Types are the folder's top-level
//! documents and subfolders, by name; a subfolder's images are those of every document under it. Each document is
//! read at its latest version.

use noshape_model::document::TabContent;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct ImagesLibrary {
    pub folder: PathBuf,
    pub types: Vec<ImageType>,
}

/// A Type: a top-level document or subfolder of the library's folder.
#[derive(Clone, Debug)]
pub struct ImageType {
    pub name: String,
    pub images: Vec<LibraryImage>,
}

#[derive(Clone, Debug)]
pub struct LibraryImage {
    /// The image's file name (the Image row's text).
    pub name: String,
    /// The image's file tab at the document's version (an `ext:` value, as Insert image's Other documents give).
    pub value: String,
}

/// The library's name in Insert image's Library ▾ (live: "‹the user's full name› images").
pub fn title() -> String {
    format!("{} images", noshape_store::local_user_name())
}

/// The library, if a folder is set as My images.
pub fn load() -> Option<ImagesLibrary> {
    let folder = crate::prefs::LibraryFolders::load().0.get("images").cloned()?;
    Some(build(&folder))
}

/// Reads `folder`'s documents at their latest versions; Types without an image are left out.
pub fn build(folder: &Path) -> ImagesLibrary {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(folder).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    entries.sort();
    let mut types = Vec::new();
    for p in entries {
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            let mut docs = Vec::new();
            crate::forms_library::collect(&p, &mut Vec::new(), &mut docs);
            let images = docs.iter().filter_map(|(_, d)| document_images(d)).flat_map(|(_, i)| i).collect();
            types.push(ImageType { name, images });
        } else if p.extension().is_some_and(|e| e == "noshape")
            && let Some((name, images)) = document_images(&p)
        {
            types.push(ImageType { name, images });
        }
    }
    types.retain(|t| !t.images.is_empty());
    types.sort_by_cached_key(|t| t.name.to_lowercase());
    ImagesLibrary { folder: folder.to_path_buf(), types }
}

impl ImagesLibrary {
    /// The Type and image a value is from.
    pub fn find(&self, value: &str) -> Option<(&ImageType, &LibraryImage)> {
        self.types.iter().find_map(|t| Some((t, t.images.iter().find(|i| i.value == value)?)))
    }
}

/// A document's name and its images at its latest named version (none without one: live builds the library from
/// versioned images).
fn document_images(path: &Path) -> Option<(String, Vec<LibraryImage>)> {
    let (info, label, mv) = crate::forms_library::latest(path)?;
    if !info.versions.iter().any(|v| v.label == label && v.label != "Current state") {
        return None;
    }
    let doc = crate::derived::external_snapshot(path, mv).ok()?;
    let images = doc
        .tabs
        .iter()
        .filter_map(|t| match &t.content {
            TabContent::File(f) if crate::forms_library::is_image(&f.file_name) => Some(LibraryImage {
                name: f.file_name.clone(),
                value: crate::derived::ExternalRef { path: path.to_path_buf(), microversion: mv, tab: t.id.to_string() }.key(),
            }),
            _ => None,
        })
        .collect();
    Some((info.name, images))
}
