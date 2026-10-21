//! Document thumbnails: after a document window commits changes, the first
//! Part Studio is regenerated in the background, rendered offscreen and
//! stored as PNG in the document metadata (`Store::set_thumbnail`), which the
//! start window shows. Debounced so it runs at most every few seconds, plus
//! once more when the window closes with pending changes.

use crate::doc_model::{DocEvent, DocModel};
use crate::gpu::Gpu;
use crate::scene_builder::{self, GpuCache, SceneOptions};
use gpui_kit::*;
use noshape_model::document::TabContent;
use noshape_model::partstudio::PartStudio;
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_render::{Camera, Style};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Minimum time between two thumbnail renders of one document.
const MIN_INTERVAL: Duration = Duration::from_secs(4);
/// Quiet time after the last change before rendering.
const SETTLE: Duration = Duration::from_millis(1500);

/// Regeneration cache shared with background jobs (incremental across runs).
#[derive(Default)]
struct Engine {
    regen: Regenerator,
    gpu_cache: GpuCache,
}

struct Job {
    studio: PartStudio,
    config: noshape_model::configuration::ConfigValues,
    units: noshape_units::UnitPrefs,
    doc_vars: std::collections::BTreeMap<String, noshape_units::Quantity>,
    engine: Arc<Mutex<Engine>>,
    gpu: Gpu,
}

impl Job {
    /// Regenerate, build the scene and render. `Ok(None)`: nothing to show.
    async fn run(self) -> anyhow::Result<Option<Vec<u8>>> {
        let (scene, camera) = {
            let mut engine = self.engine.lock();
            let opts = RegenOptions { document_variables: self.doc_vars, ..Default::default() };
            let result = engine.regen.regenerate(&self.studio, &self.config, &self.units, &opts);
            let display = result.display;
            let has_bodies = display.bodies.iter().any(|b| !b.hidden);
            if !has_bodies && display.sketches.is_empty() {
                return Ok(None);
            }
            // Parts only when there are parts; sketches otherwise.
            let scene_opts = SceneOptions { show_planes: false, show_sketches: !has_bodies, show_origin: false, ..Default::default() };
            let Engine { gpu_cache, .. } = &mut *engine;
            let built = scene_builder::build(&display, &scene_opts, gpu_cache, self.gpu.uploader());
            let Some((lo, hi)) = built.scene.bounds() else { return Ok(None) };
            let (w, h) = crate::gpu::THUMBNAIL_SIZE;
            (Arc::new(built.scene), fit_camera(lo.as_dvec3(), hi.as_dvec3(), w as f64 / h as f64))
        };
        let png = self.gpu.render_thumbnail(scene, camera, thumbnail_style()).await?;
        Ok(Some(png))
    }
}

/// Isometric camera framing the box tightly (projected extents, not the
/// bounding sphere, so flat parts fill the thumbnail).
pub(crate) fn fit_camera(lo: glam::DVec3, hi: glam::DVec3, aspect: f64) -> Camera {
    let mut camera = Camera::default();
    let (right, up) = (camera.right(), camera.up());
    let center = (lo + hi) * 0.5;
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for i in 0..8 {
        let c = glam::DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z }) - center;
        let (x, y) = (c.dot(right), c.dot(up));
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    let target = center + right * ((x0 + x1) * 0.5) + up * ((y0 + y1) * 0.5);
    let half_h = ((y1 - y0) * 0.5).max((x1 - x0) * 0.5 / aspect).max(1e-5);
    // fit_sphere adds a 15 % margin; keep about 10 %.
    camera.fit_sphere(target, half_h * 1.1 / 1.15, aspect.max(1.0));
    camera
}

/// Thumbnail look: Onshape's white document thumbnails.
pub(crate) fn thumbnail_style() -> Style {
    Style {
        background_top: crate::theme::lin(0xffffff),
        background_bottom: crate::theme::lin(0xffffff),
        show_edges: true,
        msaa: 4,
        ..Style::default()
    }
}

/// Watches one document and keeps its thumbnail up to date.
pub struct Thumbnailer {
    doc: Entity<DocModel>,
    gpu: Gpu,
    engine: Arc<Mutex<Engine>>,
    dirty: bool,
    last: Option<Instant>,
    task: Option<Task<()>>,
    _sub: Subscription,
}

impl Thumbnailer {
    pub fn new(doc: Entity<DocModel>, gpu: Gpu, cx: &mut Context<Self>) -> Self {
        let sub = cx.subscribe(&doc, |this: &mut Self, _, ev: &DocEvent, cx| match ev {
            DocEvent::TabChanged(_) | DocEvent::TabsChanged | DocEvent::Reloaded | DocEvent::UnitsChanged { .. } => this.schedule(cx),
            DocEvent::HistoryChanged | DocEvent::Error(_) | DocEvent::Info(_) => {}
        });
        // Finish a pending thumbnail when the window goes away.
        cx.on_release(|this: &mut Self, cx: &mut App| {
            if !this.dirty && this.task.is_none() {
                return;
            }
            let path = this.doc.read(cx).session.path.clone();
            if let Some(job) = this.prepare(cx) {
                cx.spawn(async move |cx| {
                    let result = cx.background_spawn(job.run()).await;
                    store_to_file(path, result);
                })
                .detach();
            }
        })
        .detach();
        let missing = doc.read(cx).session.store.get_meta("thumbnail").ok().flatten().is_none();
        let mut this = Self { doc, gpu, engine: Arc::default(), dirty: false, last: None, task: None, _sub: sub };
        if missing {
            this.schedule(cx);
        }
        this
    }

    fn schedule(&mut self, cx: &mut Context<Self>) {
        self.dirty = true;
        if self.task.is_some() {
            return;
        }
        let since = self.last.map(|t| t.elapsed()).unwrap_or(MIN_INTERVAL);
        let delay = MIN_INTERVAL.saturating_sub(since).max(SETTLE);
        self.task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let Ok(Some(job)) = this.update(cx, |t, cx| t.prepare(cx)) else {
                let _ = this.update(cx, |t, _| t.task = None);
                return;
            };
            let result = cx.background_spawn(job.run()).await;
            let _ = this.update(cx, |t, cx| {
                t.task = None;
                t.last = Some(Instant::now());
                t.store(result, cx);
                if t.dirty {
                    t.schedule(cx);
                }
            });
        }));
    }

    /// Snapshot what to render (first Part Studio at the branch head).
    fn prepare(&mut self, cx: &App) -> Option<Job> {
        self.dirty = false;
        let session = &self.doc.read(cx).session;
        if session.read_only() {
            // Viewing history: the in-memory document is not the head.
            return None;
        }
        let doc = &session.doc;
        // Tab › Select as document thumbnail, else the first Part Studio.
        let chosen = session.store.get_meta("thumbnail_tab").ok().flatten().and_then(|b| String::from_utf8_lossy(&b).parse::<u64>().ok());
        let pick = |t: &&noshape_model::document::Tab| match &t.content {
            TabContent::PartStudio(ps) => Some((t.id, ps.clone())),
            _ => None,
        };
        let tabs = doc.ordered_tabs();
        let (tab, studio) = tabs.iter().filter(|t| Some(t.id.0) == chosen).find_map(pick).or_else(|| tabs.iter().find_map(pick))?;
        Some(Job {
            config: studio.configuration.defaults(),
            studio,
            units: doc.settings.units,
            doc_vars: noshape_model::variables::element_variables(doc, tab),
            engine: self.engine.clone(),
            gpu: self.gpu.clone(),
        })
    }

    fn store(&mut self, result: anyhow::Result<Option<Vec<u8>>>, cx: &mut Context<Self>) {
        match result {
            Ok(png) => self.doc.update(cx, |d, _| {
                if let Err(e) = d.session.store.set_thumbnail(png.as_deref()) {
                    log::warn!("could not store thumbnail: {e}");
                }
            }),
            Err(e) => log::warn!("thumbnail render failed: {e:#}"),
        }
    }
}

/// Write a thumbnail after the document window closed.
fn store_to_file(path: PathBuf, result: anyhow::Result<Option<Vec<u8>>>) {
    match result {
        Ok(png) => {
            let r = noshape_store::Document::open(&path).and_then(|mut s| s.set_thumbnail(png.as_deref()));
            if let Err(e) = r {
                log::warn!("could not store thumbnail for {}: {e}", path.display());
            }
        }
        Err(e) => log::warn!("thumbnail render failed: {e:#}"),
    }
}
