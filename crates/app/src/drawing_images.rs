//! Drawing editor: Onshape's Insert image (JPG, PNG, GIF): pick the file,
//! then click and drag to place and size it (aspect ratio kept; a plain
//! click gives a 30 mm high image). Images are drawn under the sheet
//! geometry, dragged to move, and have the context menu Show / Hide image
//! borders (all images), Move to sheet ▸ and Delete.

use crate::drawing_view::*;
use gpui_kit::*;
use noshape_render::{Drawable, ImageData, ImageHandle, Instance};
use std::collections::HashMap;
use std::sync::Arc;

/// GPU textures of the current sheet's images, keyed by their data.
#[derive(Default)]
pub struct ImageCache {
    pub textures: HashMap<usize, (std::sync::Weak<str>, ImageHandle)>,
    /// (image id, texture, lower-left corner, size) for the scene.
    pub placed: Vec<(u64, ImageHandle, [f64; 2], [f64; 2])>,
}

/// Decode an inserted image to RGB8 (for PDF export).
pub fn decode_rgb(data_base64: &str) -> Option<(u32, u32, Arc<Vec<u8>>)> {
    let bytes = noshape_model::sketch_feature::base64_decode(data_base64)?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgb8();
    let (w, h) = img.dimensions();
    Some((w, h, Arc::new(img.into_raw())))
}

fn decode_rgba(data_base64: &str) -> Option<ImageData> {
    let bytes = noshape_model::sketch_feature::base64_decode(data_base64)?;
    let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(ImageData { corners: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]], width, height, rgba: rgba.into_raw() })
}

impl DrawingEditor {
    /// Toolbar › Insert image: pick an image file tab of the document (or
    /// Import one), then click and drag to place it.
    pub fn insert_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let doc = self.doc.clone();
        crate::file_tab_view::pick_file(doc, "Insert image", &[noshape_model::file_tab::FileKind::Image], &["png", "jpg", "jpeg", "gif"], window, cx, move |file, _, cx| {
            let read = file.bytes().ok_or_else(|| "the file data is damaged".to_string()).and_then(|b| noshape_scene::drawing_tools::image_aspect(&b));
            this.update(cx, |this, cx| {
                match read {
                    Ok(aspect) => {
                        this.tool = Tool::PlaceImage { name: file.file_name.clone(), data: file.data_base64.clone(), aspect, start: None };
                        this.status = Some("Click and drag to place the image".into());
                    }
                    Err(e) => this.status = Some(format!("Insert image failed: {e}").into()),
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// The image box from the drag start to the cursor (aspect kept).
    fn image_box(&self, start: [f64; 2], aspect: f64) -> ([f64; 2], [f64; 2]) {
        self.pk().image_box(start, aspect)
    }

    /// Mouse down while placing: the drag starts. True when used.
    pub fn image_mouse_down(&mut self) -> bool {
        if let Tool::PlaceImage { start, .. } = &mut self.tool {
            *start = Some(self.cursor);
            return true;
        }
        false
    }

    /// Mouse up while placing: the image goes in. True when used.
    pub fn image_mouse_up(&mut self, cx: &mut Context<Self>) -> bool {
        let Tool::PlaceImage { name, data, aspect, start: Some(start) } = self.tool.clone() else { return false };
        let (pos, size) = self.image_box(start, aspect);
        self.tool = Tool::Select;
        self.status = None;
        self.edit_sheet("Insert image", cx, |s| {
            noshape_scene::drawing_tools::add_sheet_image(s, name, data, pos, size);
        });
        true
    }

    pub fn image_overlay(&self, overlay: &mut crate::drawing_scene::Overlay) {
        if let Tool::PlaceImage { aspect, start, .. } = &self.tool {
            let (p, s) = self.image_box(start.unwrap_or(self.cursor), *aspect);
            overlay.rect(p, [p[0] + s[0], p[1] + s[1]], true);
        }
    }

    /// Upload the current sheet's images (textures cached by their data).
    pub fn sync_images(&mut self) {
        let sheet = self.current_sheet().clone();
        let up = self.gpu.uploader();
        let cache = &mut self.image_cache;
        cache.textures.retain(|_, (w, _)| w.strong_count() > 0);
        cache.placed.clear();
        for im in &sheet.images {
            let key = Arc::as_ptr(&im.data_base64) as *const u8 as usize;
            let handle = match cache.textures.get(&key) {
                Some((_, h)) => Some(h.clone()),
                None => decode_rgba(&im.data_base64).map(|d| {
                    let h = up.upload_image(&d);
                    cache.textures.insert(key, (Arc::downgrade(&im.data_base64), h.clone()));
                    h
                }),
            };
            if let Some(h) = handle {
                cache.placed.push((im.id, h, im.pos, im.size));
            }
        }
    }

    /// Scene instances of the images (under the sheet geometry).
    pub fn image_instances(&self) -> Vec<Instance> {
        self.image_cache
            .placed
            .iter()
            .map(|(_, h, p, s)| {
                let m = glam::Mat4::from_cols(glam::Vec4::new(s[0] as f32, 0.0, 0.0, 0.0), glam::Vec4::new(0.0, s[1] as f32, 0.0, 0.0), glam::Vec4::Z, glam::Vec4::new(p[0] as f32, p[1] as f32, 0.0, 1.0));
                Instance::new(Drawable::Image(h.clone())).with_transform(m).layer(-2)
            })
            .collect()
    }

    /// Context menu of an image.
    pub fn image_entries(&self, id: u64) -> Vec<MenuEntry> {
        let borders = self.drawing.props.image_borders;
        let cur = self.sheet;
        let sheets: Vec<MenuEntry> = self
            .drawing
            .sheets
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != cur)
            .map(|(i, s)| {
                MenuEntry::item(s.name.clone(), move |t, _, cx| {
                    t.selection.clear();
                    t.edit("Move image to sheet", cx, |d| {
                        if let Some(k) = d.sheets[cur].images.iter().position(|x| x.id == id) {
                            let mut im = d.sheets[cur].images.remove(k);
                            im.id = d.sheets[i].images.iter().map(|x| x.id).max().unwrap_or(0) + 1;
                            d.sheets[i].images.push(im);
                        }
                    });
                })
            })
            .collect();
        let mut out = vec![MenuEntry::item(if borders { "Hide image borders" } else { "Show image borders" }, move |t, _, cx| t.edit("Image borders", cx, |d| d.props.image_borders = !borders))];
        if !sheets.is_empty() {
            out.push(MenuEntry::Submenu("Move to sheet".into(), sheets));
        }
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Delete", move |t, _, cx| {
            t.selection.clear();
            t.edit_sheet("Delete image", cx, |s| s.images.retain(|x| x.id != id));
        }));
        out
    }
}
