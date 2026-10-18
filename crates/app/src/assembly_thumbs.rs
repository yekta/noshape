//! Insert dialog thumbnails (Onshape shows a 70x40 picture of each Part
//! Studio and part): rendered offscreen in the background with the
//! document-thumbnail look, cached per Part Studio content hash.

use super::*;
use noshape_model::drawing::views::{SourceKey, source_hash};

/// (Part Studio, part or None for the whole studio).
type ThumbKey = (TabId, Option<BodyId>);

#[derive(Default)]
pub(crate) struct InsertThumbs {
    /// (Part Studio, part or None for the whole studio) -> picture.
    pub images: HashMap<ThumbKey, Arc<RenderImage>>,
    /// Content hash each studio's pictures were made for.
    pub made_for: HashMap<TabId, u64>,
    pub busy: bool,
}

impl AssemblyView {
    pub(super) fn insert_thumb(&self, tab: TabId, part: Option<&BodyId>) -> Option<Arc<RenderImage>> {
        self.ui.thumbs.images.get(&(tab, part.cloned())).cloned()
    }

    /// Render the pictures of studios that changed since the last time
    /// (called while the Insert dialog is open).
    pub(super) fn request_insert_thumbs(&mut self, cx: &mut Context<Self>) {
        if self.ui.thumbs.busy {
            return;
        }
        let doc = self.doc.read(cx).session.doc.clone();
        let todo: Vec<(TabId, u64)> = self
            .catalog
            .studios
            .iter()
            .filter_map(|s| source_hash(&doc, &SourceKey::studio(s.tab)).map(|h| (s.tab, h)))
            .filter(|(t, h)| self.ui.thumbs.made_for.get(t) != Some(h))
            .collect();
        if todo.is_empty() {
            return;
        }
        self.ui.thumbs.busy = true;
        let (evaluator, gpu) = (self.evaluator.clone(), self.gpu.clone());
        cx.spawn(async move |this, cx| {
            let made = cx
                .background_spawn(async move {
                    let mut out: Vec<(ThumbKey, Arc<RenderImage>)> = Vec::new();
                    for (tab, _) in &todo {
                        let Some(result) = evaluator.lock().studio(&doc, *tab, &ConfigValues::default()) else { continue };
                        let display = &result.display;
                        let parts: Vec<Option<BodyId>> = std::iter::once(None).chain(display.bodies.iter().filter(|b| matches!(b.kind, BodyKind::Solid | BodyKind::Surface) && !b.hidden).map(|b| Some(b.id.clone()))).collect();
                        for part in parts {
                            let mut d = display.clone();
                            if let Some(p) = &part {
                                for b in &mut d.bodies {
                                    b.hidden = &b.id != p;
                                }
                            }
                            let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: false, show_origin: false, ..Default::default() };
                            let mut cache = crate::scene_builder::GpuCache::default();
                            let built = crate::scene_builder::build(&d, &opts, &mut cache, gpu.uploader());
                            let Some((lo, hi)) = built.scene.bounds() else { continue };
                            let (w, h) = crate::gpu::THUMBNAIL_SIZE;
                            let camera = crate::start_thumbnail::fit_camera(lo.as_dvec3(), hi.as_dvec3(), w as f64 / h as f64);
                            let Ok(png) = gpu.render_thumbnail(Arc::new(built.scene), camera, crate::start_thumbnail::thumbnail_style()).await else { continue };
                            if let Some(img) = decode(&png) {
                                out.push(((*tab, part), img));
                            }
                        }
                    }
                    (todo, out)
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                let (todo, out) = made;
                this.ui.thumbs.busy = false;
                for (tab, h) in todo {
                    this.ui.thumbs.images.retain(|(t, _), _| *t != tab);
                    this.ui.thumbs.made_for.insert(tab, h);
                }
                this.ui.thumbs.images.extend(out);
                cx.notify();
            });
        })
        .detach();
    }
}

/// PNG -> GPUI image (BGRA); the white background becomes transparent so
/// the pictures sit on hovered rows.
fn decode(png: &[u8]) -> Option<Arc<RenderImage>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let mut rgba = img.to_rgba8();
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
        if px[0] >= 250 && px[1] >= 250 && px[2] >= 250 {
            *px = [0; 4];
        }
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}
