//! Document thumbnails: after a document window commits changes, the first
//! Part Studio is rendered offscreen and stored as PNG in the document
//! metadata (`Store::set_thumbnail`), which the start window shows. Debounced
//! so it runs at most every few seconds, plus once more when the window
//! closes with pending changes. When that Part Studio is open, its view's own
//! display is drawn ([`StudioDisplays`]); only a closed one is regenerated
//! here (a second regeneration in parallel doubled the time of opening and
//! undoing on large models).

use crate::doc_model::{DocEvent, DocModel};
use crate::gpu::Gpu;
use crate::scene_builder::{self, GpuCache, SceneOptions};
use gpui_kit::*;
use noshape_model::display::StudioDisplay;
use noshape_model::document::{TabContent, TabId};
use noshape_model::partstudio::PartStudio;
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_render::{Camera, Style};
use noshape_units::Quantity;
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Minimum time between two thumbnail renders of one document.
const MIN_INTERVAL: Duration = Duration::from_secs(4);
/// Quiet time after the last change before rendering.
const SETTLE: Duration = Duration::from_millis(1500);

/// How long a thumbnail waits for an open Part Studio's regeneration before
/// regenerating the studio itself.
const MAX_WAIT: Duration = Duration::from_secs(60);
/// How often a waiting thumbnail looks again.
const WAIT_STEP: Duration = Duration::from_millis(250);

/// The displays open Part Studio views made of their studio's head (no
/// rollback, default configuration, not editing a sketch), by document and
/// tab, and whether one of their regenerations is running.
#[derive(Default)]
pub struct StudioDisplays(HashMap<(PathBuf, TabId), Shown>);

impl Global for StudioDisplays {}

#[derive(Default)]
struct Shown {
    /// A regeneration started then (a newer display is coming).
    pending: Option<Instant>,
    made: Option<Made>,
}

/// A view's display and the studio and document variables it was made from.
pub type Made = (Arc<PartStudio>, BTreeMap<String, Quantity>, Arc<StudioDisplay>);

/// A Part Studio view started regenerating.
pub fn regen_started(path: &Path, tab: TabId, cx: &mut App) {
    let shown = cx.default_global::<StudioDisplays>().0.entry((path.to_path_buf(), tab)).or_default();
    shown.pending.get_or_insert_with(Instant::now);
}

/// A Part Studio view finished regenerating: `made` the head display (else
/// `None`: rolled back, another configuration, editing a sketch).
pub fn regen_done(path: &Path, tab: TabId, made: Option<Made>, cx: &mut App) {
    let shown = cx.default_global::<StudioDisplays>().0.entry((path.to_path_buf(), tab)).or_default();
    shown.pending = None;
    shown.made = made;
}

/// A Part Studio view went away.
pub fn view_closed(path: &Path, tab: TabId, cx: &mut App) {
    if cx.has_global::<StudioDisplays>() {
        cx.global_mut::<StudioDisplays>().0.remove(&(path.to_path_buf(), tab));
    }
}

/// What a thumbnail draws.
enum Source {
    /// The open view's display of this very studio.
    Shown(Arc<StudioDisplay>),
    /// The open view is regenerating it: ask again shortly.
    Wait,
    /// Not open (or shown differently): regenerate it here.
    Regenerate,
}

/// The open view's display of `studio`, if it has (or is about to have) one.
fn source(cx: &App, path: &Path, tab: TabId, studio: &PartStudio, doc_vars: &BTreeMap<String, Quantity>) -> Source {
    decide(cx.try_global::<StudioDisplays>().and_then(|d| d.0.get(&(path.to_path_buf(), tab))), studio, doc_vars)
}

fn decide(shown: Option<&Shown>, studio: &PartStudio, doc_vars: &BTreeMap<String, Quantity>) -> Source {
    let Some(shown) = shown else { return Source::Regenerate };
    if shown.pending.is_some_and(|t| t.elapsed() < MAX_WAIT) {
        return Source::Wait;
    }
    match &shown.made {
        Some((s, v, d)) if **s == *studio && v == doc_vars => Source::Shown(d.clone()),
        _ => Source::Regenerate,
    }
}

enum Prepared {
    Job(Box<Job>),
    Wait,
    Nothing,
}

/// Regeneration cache shared with background jobs (incremental across runs).
#[derive(Default)]
struct Engine {
    regen: Regenerator,
    gpu_cache: GpuCache,
}

struct Job {
    /// The open view's display (no regeneration).
    shown: Option<Arc<StudioDisplay>>,
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
            let display = match self.shown {
                Some(d) => d,
                None => {
                    let opts = RegenOptions { document_variables: self.doc_vars, ..Default::default() };
                    Arc::new(engine.regen.regenerate(&self.studio, &self.config, &self.units, &opts).display)
                }
            };
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
            DocEvent::HistoryChanged | DocEvent::Error(_) | DocEvent::Info(_) | DocEvent::Ui(_) => {}
        });
        // Finish a pending thumbnail when the window goes away.
        cx.on_release(|this: &mut Self, cx: &mut App| {
            if !this.dirty && this.task.is_none() {
                return;
            }
            let path = this.doc.read(cx).session.path.clone();
            if let Prepared::Job(job) = this.prepare(false, cx) {
                cx.spawn(async move |cx| {
                    let result = cx.background_spawn((*job).run()).await;
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
            // An open view still regenerating the studio: its display comes.
            let job = loop {
                match this.update(cx, |t, cx| t.prepare(true, cx)) {
                    Ok(Prepared::Job(job)) => break *job,
                    Ok(Prepared::Wait) => cx.background_executor().timer(WAIT_STEP).await,
                    _ => {
                        let _ = this.update(cx, |t, _| t.task = None);
                        return;
                    }
                }
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
    /// `may_wait`: for an open view that is regenerating, `Prepared::Wait`
    /// (else regenerate here).
    fn prepare(&mut self, may_wait: bool, cx: &App) -> Prepared {
        let session = &self.doc.read(cx).session;
        if session.read_only() {
            // Viewing history: the in-memory document is not the head.
            self.dirty = false;
            return Prepared::Nothing;
        }
        let doc = &session.doc;
        // Tab › Select as document thumbnail, else the first Part Studio.
        let chosen = session.store.get_meta("thumbnail_tab").ok().flatten().and_then(|b| String::from_utf8_lossy(&b).parse::<u64>().ok());
        let pick = |t: &&noshape_model::document::Tab| match &t.content {
            TabContent::PartStudio(ps) => Some((t.id, ps.clone())),
            _ => None,
        };
        let tabs = doc.ordered_tabs();
        let Some((tab, studio)) = tabs.iter().filter(|t| Some(t.id.0) == chosen).find_map(pick).or_else(|| tabs.iter().find_map(pick)) else {
            self.dirty = false;
            return Prepared::Nothing;
        };
        let doc_vars = noshape_model::variables::element_variables(doc, tab);
        let shown = match source(cx, &session.path, tab, &studio, &doc_vars) {
            Source::Wait if may_wait => return Prepared::Wait,
            Source::Wait => None,
            Source::Shown(d) => Some(d),
            Source::Regenerate => None,
        };
        self.dirty = false;
        Prepared::Job(Box::new(Job { shown, config: studio.configuration.defaults(), studio, units: doc.settings.units, doc_vars, engine: self.engine.clone(), gpu: self.gpu.clone() }))
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

#[cfg(test)]
mod tests {
    use super::{decide, Duration, Instant, PartStudio, Shown, Source, StudioDisplay, MAX_WAIT};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn kind(s: Source) -> &'static str {
        match s {
            Source::Shown(_) => "shown",
            Source::Wait => "wait",
            Source::Regenerate => "regenerate",
        }
    }

    /// The thumbnail draws an open view's display of the very same studio,
    /// waits while the view regenerates, and regenerates itself otherwise.
    #[test]
    fn thumbnail_reuses_the_open_views_display() {
        let units = noshape_units::UnitPrefs::default();
        let studio = PartStudio::new();
        let mut edited = studio.clone();
        edited.features.push(edited.new_feature("sketch", &units).unwrap());
        let vars = BTreeMap::new();
        let display = Arc::new(StudioDisplay::defaults());
        let made = Shown { pending: None, made: Some((Arc::new(studio.clone()), vars.clone(), display)) };
        assert_eq!(kind(decide(None, &studio, &vars)), "regenerate", "no open view");
        assert_eq!(kind(decide(Some(&made), &studio, &vars)), "shown");
        assert_eq!(kind(decide(Some(&made), &edited, &vars)), "regenerate", "made from another state");
        let regenerating = Shown { pending: Some(Instant::now()), ..made };
        assert_eq!(kind(decide(Some(&regenerating), &edited, &vars)), "wait");
        let stuck = Shown { pending: Instant::now().checked_sub(MAX_WAIT + Duration::from_secs(1)), made: None };
        assert_eq!(kind(decide(Some(&stuck), &edited, &vars)), "regenerate", "waited long enough");
    }
}
