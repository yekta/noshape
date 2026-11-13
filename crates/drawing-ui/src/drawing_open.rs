//! Opening a drawing: the opened sheet's cached build (`crate::display_cache`) shown at once while its sources
//! regenerate and its views are computed, view-only until the real sheet is complete (lead 11-06: "Regenerating…"
//! over the sheet, a tool asked for waits with a message and starts then, a click on the sheet says so); the real
//! sheet replaces it in the refresh that completes it. `NOSHAPE_PERF`'s open milestones (the frame that first draws
//! the sheet's views, then their exact geometry, as the Part Studio's preview and full model:
//! tools/perf/open-times.py) and `NOSHAPE_PERF_SCRIPT=open`'s quit once the exact one is up.

use crate::display_cache::{self as cache, WAITING};
use crate::drawing_view::DrawingEditor;
use gpui_kit::*;
use noshape_model::drawing::build::BuiltSheet;
use noshape_model::document::TabId;
use std::sync::Arc;
use std::time::Duration;

/// An action that waits for the real sheet.
pub(crate) type Deferred = Box<dyn FnOnce(&mut DrawingEditor, &mut Context<DrawingEditor>)>;

/// How long after a complete sheet the cache is written (edits in a row write once).
const SAVE_AFTER: Duration = Duration::from_secs(2);

/// The display cache's state of a drawing editor.
#[derive(Default)]
pub(crate) struct CachedOpen {
    checked: bool,
    /// The cached build of sheet `.0`, shown until its real one is complete.
    shown: Option<(usize, Arc<BuiltSheet>)>,
    /// A complete real sheet was shown.
    real_in: bool,
    task: Option<Task<()>>,
    save_task: Option<Task<()>>,
    /// The key of what was last cached or shown from the cache.
    key: Option<String>,
    after_real: Option<Deferred>,
    /// Opening (live2 feedback.json documentLoad): when, whether a view was drawn yet, and the timer that shows the
    /// loading state after `cache::LOADING_DELAY`.
    opened_at: Option<std::time::Instant>,
    loaded: bool,
    loading_timer: Option<Task<()>>,
}

fn key(doc: &noshape_model::Document, tab: TabId, sheet: usize) -> Option<String> {
    cache::key(doc, &("drawing", tab, sheet))
}

/// A sheet's build in the cache: as JSON (its sheet data's serde attributes need a self-describing format).
pub(crate) fn store_sheet(dir: &std::path::Path, key: &str, built: &BuiltSheet) -> std::io::Result<()> {
    cache::store(dir, key, &serde_json::to_vec(built).map_err(std::io::Error::other)?)
}

pub(crate) fn load_sheet(dir: &std::path::Path, key: &str) -> Option<BuiltSheet> {
    serde_json::from_slice(&cache::load::<Vec<u8>>(dir, key)?).ok()
}

impl DrawingEditor {
    /// While the cached sheet stands in.
    pub(crate) fn cached_view(&self) -> bool {
        self.cached_open.shown.is_some()
    }

    /// On the first refresh: the opened sheet's cached build, if there is one, shown until the real one is complete.
    fn load_cached_sheet(&mut self, cx: &mut Context<Self>) {
        if !cache::enabled() || std::mem::replace(&mut self.cached_open.checked, true) {
            return;
        }
        let (doc, tab, sheet) = (self.doc.read(cx).session.doc.clone(), self.tab, self.sheet);
        let load = cx.background_spawn(async move {
            let key = key(&doc, tab, sheet)?;
            let built = load_sheet(&cache::default_dir(), &key)?;
            Some((key, built))
        });
        self.cached_open.task = Some(cx.spawn(async move |this, cx| {
            let Some((key, built)) = load.await else { return };
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.cached_open.task = None;
                if this.cached_open.real_in || this.sheet != sheet {
                    return;
                }
                noshape_render::perf::milestone("open: cached display in");
                noshape_render::perf::arm_frame_milestone("open: cached model frame presented");
                this.cached_open.shown = Some((sheet, Arc::new(built)));
                this.cached_open.key = Some(key);
                this.refresh(cx);
            });
        }));
    }

    /// `refresh` assembled sheet `si`'s build, `complete` when every view's source is in and its exact geometry
    /// computed: the build to show (the cached one while it stands in), the real one cached once complete.
    pub(crate) fn sheet_to_show(&mut self, si: usize, built: BuiltSheet, complete: bool, cx: &mut Context<Self>) -> BuiltSheet {
        self.load_cached_sheet(cx);
        if !built.placements.is_empty() {
            self.cached_open.loaded = true;
        }
        if !complete {
            return match &self.cached_open.shown {
                Some((s, cached)) if *s == si => (**cached).clone(),
                _ => built,
            };
        }
        self.cached_open.real_in = true;
        if self.cached_open.shown.take().is_some() {
            if self.toast.as_ref().is_some_and(|t| t.as_ref() == WAITING) {
                self.toast = None;
            }
            if let Some(action) = self.cached_open.after_real.take() {
                let view = cx.entity();
                cx.defer(move |cx| view.update(cx, |this, cx| action(this, cx)));
            }
        }
        if cache::enabled() && self.preview.is_none() && self.inplace_field.is_none() && self.inplace_cell.is_none() {
            self.schedule_sheet_save(si, built.clone(), cx);
        }
        built
    }

    /// Tests: opened a second ago, nothing drawn yet.
    #[cfg(test)]
    pub(crate) fn opening_for_test(&mut self) {
        self.cached_open.loaded = false;
        self.cached_open.opened_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
    }

    /// Tests: the opened sheet's build `built` stands in as if read from the cache.
    #[cfg(test)]
    pub(crate) fn stand_in(&mut self, sheet: usize, built: BuiltSheet) {
        self.cached_open.real_in = false;
        self.cached_open.shown = Some((sheet, Arc::new(built)));
    }

    /// Tests: a complete real sheet was shown.
    #[cfg(test)]
    pub(crate) fn real_sheet_in(&self) -> bool {
        self.cached_open.real_in
    }

    /// While the cached sheet stands in: `action` waits for the real one, and the user is told at once. Returns
    /// whether it waits.
    pub(crate) fn wait_for_sheet(&mut self, action: Deferred, cx: &mut Context<Self>) -> bool {
        if !self.cached_view() {
            return false;
        }
        self.cached_open.after_real = Some(action);
        self.say_waiting(cx);
        true
    }

    /// What can't wait (a click on the sheet) only says so.
    pub(crate) fn say_waiting(&mut self, cx: &mut Context<Self>) {
        self.show_message(WAITING, crate::os_ui::BubbleKind::Info, None, cx);
    }

    fn schedule_sheet_save(&mut self, si: usize, built: BuiltSheet, cx: &mut Context<Self>) {
        let wait = if self.cached_open.key.is_none() { Duration::ZERO } else { SAVE_AFTER };
        let (doc, tab, known) = (self.doc.read(cx).session.doc.clone(), self.tab, self.cached_open.key.clone());
        self.cached_open.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let saved = cx
                .background_spawn(async move {
                    let key = key(&doc, tab, si)?;
                    let dir = cache::default_dir();
                    if known.as_ref() == Some(&key) || cache::contains(&dir, &key) {
                        return Some(key);
                    }
                    store_sheet(&dir, &key, &built).ok()?;
                    Some(key)
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, _| {
                this.cached_open.save_task = None;
                if saved.is_some() {
                    this.cached_open.key = saved;
                }
            });
        }));
    }

    /// Over the sheet's top left while the cached sheet stands in: Onshape's spinner and "Regenerating…".
    /// The editor opened: the loading state shows if no view is drawn after a moment.
    pub(crate) fn start_loading_timer(&mut self, cx: &mut Context<Self>) {
        self.cached_open.opened_at = Some(std::time::Instant::now());
        self.cached_open.loading_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(cache::LOADING_DELAY).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        }));
    }

    /// The opening drawing's loading message (live2 feedback.json documentLoad, as a Part Studio's): "Loading studio
    /// data…" while a view's source regenerates, "Loading graphics…" while the views compute; none once a view (or
    /// the cached sheet) is drawn, on a sheet with no views, or in the first moment of a quick open.
    pub(crate) fn loading_message(&self) -> Option<&'static str> {
        if self.cached_open.loaded || self.cached_view() || self.cached_open.opened_at.is_none_or(|t| t.elapsed() < cache::LOADING_DELAY) {
            return None;
        }
        self.current_sheet().real_views().next()?;
        Some(if self.regenerating.is_empty() { "Loading graphics…" } else { "Loading studio data…" })
    }

    pub(crate) fn render_regenerating(&self) -> Option<AnyElement> {
        self.cached_view().then(|| div().absolute().top(px(8.)).left(px(8.)).bg(crate::theme::p().panel).rounded(px(3.)).pr(px(8.)).child(cache::regenerating_row()).into_any_element())
    }

    /// The sheet was rebuilt: `shown` with every view's geometry, `exact` with its exact geometry.
    pub(crate) fn note_open_sheet(&mut self, shown: bool, exact: bool, cx: &mut Context<Self>) {
        if !noshape_render::perf::enabled() {
            return;
        }
        let stage = if exact { 2 } else if shown { 1 } else { 0 };
        if stage <= self.open_stage {
            return;
        }
        for (k, what) in [(1, "preview"), (2, "full")] {
            if self.open_stage < k && stage >= k {
                noshape_render::perf::milestone(&format!("open: {what} display in (regen 0.0 ms)"));
                noshape_render::perf::arm_frame_milestone(format!("open: {what} model frame presented"));
            }
        }
        self.open_stage = stage;
        if stage < 2 || std::env::var("NOSHAPE_PERF_SCRIPT").as_deref() != Ok("open") {
            return;
        }
        // Long enough for the armed frame to be presented and logged, and the cache written.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(1500)).await;
            for _ in 0..200 {
                if this.update(cx, |this, _| this.cached_open.save_task.is_none()).unwrap_or(true) {
                    break;
                }
                cx.background_executor().timer(Duration::from_millis(100)).await;
            }
            noshape_render::perf::flush();
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
}
