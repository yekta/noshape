//! Drawing editor: Onshape's Insert image (JPG, PNG, GIF): pick the file,
//! then click and drag to place and size it (aspect ratio kept; a plain
//! click gives a 30 mm high image). Images are drawn under the sheet
//! geometry, dragged to move, and have the context menu Show / Hide image
//! borders (all images), Move to sheet ▸ and Delete.

use crate::drawing_view::*;
use gpui_kit::*;
use noshape_render::{Drawable, ImageData, ImageHandle, Instance};
use noshape_model::document::TabId;
use noshape_model::drawing::SheetLayer;
use std::collections::HashMap;
use std::sync::Arc;

/// GPU textures of the current sheet's images, keyed by their data.
#[derive(Default)]
pub struct ImageCache {
    pub textures: HashMap<usize, (std::sync::Weak<str>, ImageHandle)>,
    /// (image id, texture, lower-left corner, size) for the scene.
    pub placed: Vec<(u64, ImageHandle, [f64; 2], [f64; 2])>,
}

fn decode_rgba(data_base64: &str) -> Option<ImageData> {
    let bytes = noshape_model::sketch_feature::base64_decode(data_base64)?;
    let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(ImageData { corners: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]], width, height, rgba: rgba.into_raw() })
}

impl DrawingEditor {
    /// Toolbar › Insert image: pick an image file tab of the document (or
    /// Import one), then two clicks set opposite corners of its box.
    pub fn insert_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let doc = self.doc.clone();
        crate::drawing_hooks::pick_file(doc, "Insert image", &[noshape_model::file_tab::FileKind::Image], &["png", "jpg", "jpeg", "gif"], window, cx, move |file, _, cx| {
            let read = file.bytes().ok_or_else(|| "the file data is damaged".to_string()).and_then(|b| noshape_scene::drawing_tools::image_aspect(&b));
            this.update(cx, |this, cx| {
                match read {
                    Ok(aspect) => {
                        this.tool = Tool::PlaceImage { name: file.file_name.clone(), data: file.data_base64.clone(), aspect, start: None };
                        this.status = Some("Click two opposite corners to place the image".into());
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

    /// Mouse down while placing: the first click sets a corner, the second
    /// the opposite one (live request 1613: two clicks). True when used.
    pub fn image_mouse_down(&mut self, cx: &mut Context<Self>) -> bool {
        match &mut self.tool {
            Tool::PlaceImage { start: start @ None, .. } => {
                *start = Some(self.cursor);
                self.status = Some("Click to place the opposite corner".into());
                cx.notify();
                true
            }
            Tool::PlaceImage { start: Some(_), .. } => {
                self.place_image(cx);
                true
            }
            _ => false,
        }
    }

    /// Mouse up while placing: a drag places the image at once, a click
    /// waits for the second corner. True when used.
    pub fn image_mouse_up(&mut self, cx: &mut Context<Self>) -> bool {
        let Tool::PlaceImage { start, .. } = &self.tool else { return false };
        if start.is_some_and(|s| (self.cursor[0] - s[0]).hypot(self.cursor[1] - s[1]) > self.tol_mm(6.0)) {
            self.place_image(cx);
        }
        true
    }

    fn place_image(&mut self, cx: &mut Context<Self>) {
        let Tool::PlaceImage { name, data, aspect, start: Some(start) } = self.tool.clone() else { return };
        let (pos, size) = self.image_box(start, aspect);
        self.tool = Tool::Select;
        self.status = None;
        self.edit_sheet("Insert image", cx, |s| {
            noshape_scene::drawing_tools::add_sheet_image(s, name, data, pos, size);
        });
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

    /// The image tab an image came from (its data), with the tab's name.
    pub fn image_tab(doc: &noshape_model::Document, im: &noshape_model::drawing::SheetImage) -> Option<(TabId, String)> {
        doc.tabs.iter().find_map(|t| match &t.content {
            noshape_model::document::TabContent::File(f) if Arc::ptr_eq(&f.data_base64, &im.data_base64) || f.data_base64 == im.data_base64 => Some((t.id, t.name.clone())),
            _ => None,
        })
    }

    /// Whether an image's tab is gone (the drawing then needs an Update).
    pub fn images_stale(&self, cx: &App) -> bool {
        let doc = &self.doc.read(cx).session.doc;
        self.drawing.sheets.iter().flat_map(|s| &s.images).any(|im| Self::image_tab(doc, im).is_none())
    }

    /// Update: images whose image tab is gone leave the sheets, with live's
    /// bubble (live 1613 (a): "Failed to resolve image target tab").
    pub fn drop_unresolved_images(&mut self, cx: &mut Context<Self>) {
        let doc = self.doc.read(cx).session.doc.clone();
        let lost: Vec<(usize, u64)> = self.drawing.sheets.iter().enumerate().flat_map(|(si, s)| s.images.iter().filter(|im| Self::image_tab(&doc, im).is_none()).map(move |im| (si, im.id))).collect();
        if lost.is_empty() {
            return;
        }
        self.selection.retain(|s| !matches!(s, Sel::Image(i) if lost.iter().any(|(_, l)| l == i)));
        self.edit("Update", cx, |d| {
            for (si, id) in &lost {
                if let Some(s) = d.sheets.get_mut(*si) {
                    s.images.retain(|im| im.id != *id);
                }
            }
        });
        self.status = Some("Failed to resolve image target tab".into());
    }

    /// Bring to front / Send to back: the image list is the stacking order.
    fn order_image(&mut self, id: u64, front: bool, cx: &mut Context<Self>) {
        self.edit_sheet(if front { "Bring to front" } else { "Send to back" }, cx, |s| {
            if let Some(i) = s.images.iter().position(|x| x.id == id) {
                let im = s.images.remove(i);
                if front {
                    s.images.push(im);
                } else {
                    s.images.insert(0, im);
                }
            }
        });
    }

    /// Image properties... (live 1613 (a)): read only.
    fn open_image_props(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(im) = self.current_sheet().images.iter().find(|i| i.id == id).cloned() else { return };
        let dm = self.doc.read(cx);
        let tab = Self::image_tab(&dm.session.doc, &im);
        let rows = vec![
            ("Document", dm.name()),
            ("Workspace", "Main".to_string()),
            ("Reference", tab.as_ref().map(|t| format!("{}  In progress", t.1)).unwrap_or_else(|| im.name.clone())),
            ("Sheet", self.current_sheet().name.clone()),
        ];
        self.panel = Some(Panel::ImageProps { rows, tab: tab.map(|t| t.0) });
        cx.notify();
    }

    /// Context menu of an image (live 1613 (a), in order): Image
    /// properties..., Show / Hide image borders, Paste, Copy, Paste as
    /// table, Move to ▸, Bring to front, Send to back, (ours: Move to sheet
    /// ▸), Switch to <image tab>, Clear selection, Zoom to fit, Delete.
    pub fn image_entries(&self, id: u64, cx: &App) -> Vec<MenuEntry> {
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
                    t.edit(noshape_ops::drawing::MOVE_IMAGE_TO_SHEET, cx, |d| {
                        noshape_ops::drawing::move_image_to_sheet(d, cur, id, i);
                    });
                })
            })
            .collect();
        let mut out = vec![
            MenuEntry::item("Image properties...", move |t, _, cx| t.open_image_props(id, cx)),
            MenuEntry::item(noshape_ops::drawing::image_borders_label(borders), move |t, _, cx| t.edit(noshape_ops::drawing::IMAGE_BORDERS, cx, |d| d.props.image_borders = !borders)),
            crate::drawing_props::paste_always(),
            MenuEntry::item("Copy", move |t, _, cx| {
                if !t.selection.contains(&Sel::Image(id)) {
                    t.selection = vec![Sel::Image(id)];
                }
                t.copy_entities(cx);
            }),
            MenuEntry::item("Paste as table", |t, _, cx| {
                t.start_paste_table(cx);
            }),
        ];
        let sheet = self.current_sheet();
        let cur = sheet.images.iter().find(|i| i.id == id).map(|i| i.layer).unwrap_or_default();
        let layers: Vec<MenuEntry> = SheetLayer::FORMAT
            .iter()
            .copied()
            .chain((!cur.is_drawing()).then_some(SheetLayer::Drawing))
            .filter(|l| *l != cur)
            .map(|l| {
                MenuEntry::item(l.name(), move |t, _, cx| {
                    t.edit_sheet("Move to", cx, |s| s.images.iter_mut().filter(|i| i.id == id).for_each(|i| i.layer = l));
                })
            })
            .collect();
        out.push(MenuEntry::Submenu("Move to".into(), layers));
        // The image list is their stacking order; live greys Bring to front
        // when the image is already in front.
        let front = sheet.images.last().is_some_and(|i| i.id == id);
        out.push(MenuEntry::item("Bring to front", move |t, _, cx| t.order_image(id, true, cx)).disabled(front));
        out.push(MenuEntry::item("Send to back", move |t, _, cx| t.order_image(id, false, cx)));
        if !sheets.is_empty() {
            out.push(MenuEntry::Submenu("Move to sheet".into(), sheets));
        }
        let tab = self.current_sheet().images.iter().find(|i| i.id == id).and_then(|im| Self::image_tab(&self.doc.read(cx).session.doc, im));
        if let Some((tab, name)) = tab {
            out.push(MenuEntry::Separator);
            out.push(MenuEntry::item(format!("Switch to {name}"), move |_, w, cx| crate::actions::request_activate_tab(tab, w, cx)));
        }
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Clear selection", |t, _, cx| {
            t.selection.clear();
            t.redraw(cx);
        }));
        out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
        out.push(MenuEntry::item("Delete", move |t, _, cx| {
            t.selection.clear();
            t.edit_sheet("Delete image", cx, |s| s.images.retain(|x| x.id != id));
        }));
        out
    }
}
