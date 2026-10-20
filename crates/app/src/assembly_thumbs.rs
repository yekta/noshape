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
    /// Other documents' Part Studios (external key, part) -> picture, and
    /// the keys being rendered or done.
    pub external: HashMap<ExternalThumbKey, Arc<RenderImage>>,
    pub external_requested: HashSet<String>,
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
                        // Sketches (Sketches filter): each sketch alone, seen
                        // normal to its plane.
                        for sk in &display.sketches {
                            let mut d = display.clone();
                            for b in &mut d.bodies {
                                b.hidden = true;
                            }
                            d.sketches.retain(|x| x.feature == sk.feature);
                            if let Some(x) = d.sketches.first_mut() {
                                x.hidden = false;
                            }
                            let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: true, show_origin: false, ..Default::default() };
                            let mut cache = crate::scene_builder::GpuCache::default();
                            let built = crate::scene_builder::build(&d, &opts, &mut cache, gpu.uploader());
                            let Some((lo, hi)) = built.scene.bounds() else { continue };
                            let (w, h) = crate::gpu::THUMBNAIL_SIZE;
                            let camera = fit_camera_normal(lo.as_dvec3(), hi.as_dvec3(), w as f64 / h as f64, &sk.frame);
                            let Ok(png) = gpu.render_thumbnail(Arc::new(built.scene), camera, crate::start_thumbnail::thumbnail_style()).await else { continue };
                            if let Some(img) = decode(&png) {
                                out.push(((*tab, Some(noshape_model::assembly::eval::sketch_body_id(sk.feature))), img));
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

impl AssemblyView {
    pub(super) fn external_thumb(&self, key: &str, part: Option<&BodyId>) -> Option<Arc<RenderImage>> {
        self.ui.thumbs.external.get(&(key.to_string(), part.cloned())).cloned()
    }

    /// Render the pictures of another document's Part Studio and its parts
    /// (Insert › Other documents), once per pinned version.
    pub(super) fn request_external_thumbs(&mut self, key: String, cx: &mut Context<Self>) {
        if !self.ui.thumbs.external_requested.insert(key.clone()) {
            return;
        }
        let (evaluator, gpu) = (self.evaluator.clone(), self.gpu.clone());
        cx.spawn(async move |this, cx| {
            let k2 = key.clone();
            let made = cx
                .background_spawn(async move {
                    let mut out: Vec<(ExternalThumbKey, Arc<RenderImage>)> = Vec::new();
                    let Some((_, result)) = evaluator.lock().external_studio(&k2, &ConfigValues::default()) else { return out };
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
                            out.push(((k2.clone(), part), img));
                        }
                    }
                    out
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.ui.thumbs.external.extend(made);
                cx.notify();
            });
        })
        .detach();
    }
}

/// Another document's Part Studio (pinned version key) and one of its parts
/// (`None`: the whole studio).
pub(crate) type ExternalThumbKey = (String, Option<BodyId>);

/// A camera looking at the box normal to a sketch plane (its X right).
fn fit_camera_normal(lo: DVec3, hi: DVec3, aspect: f64, frame: &noshape_model::display::PlaneFrame) -> noshape_render::Camera {
    let mut camera = noshape_render::Camera { rotation: noshape_render::Camera::look_rotation(-frame.normal, frame.y), ..Default::default() };
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
    camera.fit_sphere(target, half_h * 1.1 / 1.15, aspect.max(1.0));
    camera
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
