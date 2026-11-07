//! Opening on the cached display (`display_cache`): the Part Studio shows its last display at once while the real
//! regeneration runs. Until that lands the view is view-only (`cached_view`): orbit, pan and zoom, a "Regenerating…"
//! row at the top of the feature list, and an action that needs the real model (a feature dialog) says so at once and
//! runs when it lands (lead 11-06: never silence). The real result replaces the cached one in the frame it arrives
//! (its scene is built before it is swapped in: no flash).

use super::{PartStudioView, display_cache};
use crate::display_cache::{WAITING, enabled};
use gpui_kit::*;
use noshape_model::display::FeatureStatus;
use std::sync::Arc;
use std::time::Duration;

/// An action that waits for the real model.
pub(super) type Deferred = Box<dyn FnOnce(&mut PartStudioView, &mut Window, &mut Context<PartStudioView>)>;

/// How long after a full display the cache is written (edits in a row write once).
const SAVE_AFTER: Duration = Duration::from_secs(2);

/// The key of a display's coarse (preview) level.
fn coarse_key(key: &str) -> String {
    format!("{key}-coarse")
}

impl PartStudioView {
    fn cache_key_inputs(&self, cx: &App) -> (noshape_model::Document, noshape_model::document::TabId, noshape_model::configuration::ConfigValues, bool) {
        (self.doc.read(cx).session.doc.clone(), self.tab, self.config_values.clone(), self.ui.high_quality_seen)
    }

    /// On the studio's first regeneration: its cached display, if there is one, shown until the real one lands.
    pub(super) fn load_cached_display(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !enabled() || std::mem::replace(&mut self.ui.cache_checked, true) {
            return;
        }
        let (doc, tab, config, fine) = self.cache_key_inputs(cx);
        // The coarse display first (a fraction of the refined one's triangles: drawn at once), then the refined one.
        let first = cx.background_spawn(async move {
            let key = display_cache::key(&doc, tab, &config, fine)?;
            let dir = display_cache::default_dir();
            let coarse = display_cache::load(&dir, &coarse_key(&key));
            Some((key, coarse))
        });
        self.ui.cache_task = Some(cx.spawn_in(window, async move |this, cx| {
            let Some((key, coarse)) = first.await else { return };
            let had_coarse = coarse.is_some();
            if let Some(display) = coarse {
                let key = key.clone();
                let _ = this.update_in(cx, |this: &mut Self, window, cx| this.show_cached_display(key, display, window, cx));
            }
            let full = cx.background_spawn(async move { display_cache::load(&display_cache::default_dir(), &key).map(|d| (key, d)) }).await;
            let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                this.ui.cache_task = None;
                // Over the coarse cached one, or alone when there was none; never over the real model.
                if let Some((key, display)) = full.filter(|_| this.ui.cached_view || !had_coarse) {
                    this.show_cached_display(key, display, window, cx);
                }
            });
        }));
    }

    /// The cached display stands in until the real one is in (unless that was quicker).
    pub(super) fn show_cached_display(&mut self, key: String, display: noshape_model::display::StudioDisplay, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.real_display_in {
            return;
        }
        noshape_render::perf::milestone("open: cached display in");
        noshape_render::perf::arm_frame_milestone("open: cached model frame presented");
        self.ui.cached_view = true;
        self.ui.cache_key = Some(key);
        self.apply_display_async(Arc::new(display), window, cx);
        cx.notify();
    }

    /// A real regeneration's display is in: the view is the model's own again, a waiting action runs, and a full
    /// display is cached once the studio is idle.
    pub(super) fn note_real_display(&mut self, full: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.real_display_in = true;
        self.note_new_contexts(cx);
        self.ui.cache_task = None;
        if std::mem::take(&mut self.ui.cached_view) {
            if self.message.as_ref().is_some_and(|(_, m)| m == WAITING) {
                self.message = None;
            }
            cx.notify();
        }
        if let Some(action) = self.ui.after_regen.take() {
            let view = cx.entity();
            window.defer(cx, move |window, cx| view.update(cx, |this, cx| action(this, window, cx)));
        }
        if full && enabled() && self.edit.is_none() {
            self.schedule_cache_save(window, cx);
        }
    }

    /// While the cached display stands in: `action` waits for the real model, and the user is told at once.
    /// Returns whether it was deferred.
    pub(super) fn defer_until_regenerated(&mut self, action: Deferred, cx: &mut Context<Self>) -> bool {
        if !self.ui.cached_view {
            return false;
        }
        self.ui.after_regen = Some(action);
        self.message = Some((FeatureStatus::Info, WAITING.into()));
        cx.notify();
        true
    }

    fn schedule_cache_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let seq = self.regen_seq;
        // The opened document's first full display at once (it may be closed soon); edits in a row once they pause.
        let wait = if self.ui.cache_key.is_none() { Duration::ZERO } else { SAVE_AFTER };
        self.ui.cache_save_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let job = this.update(cx, |this: &mut Self, cx| {
                // Only what the document is now: no newer regeneration, no dialog.
                if this.regen_seq != seq || this.edit.is_some() || this.pending.is_some() {
                    return None;
                }
                let coarse = this.ui.preview_for_cache.as_ref().filter(|(s, _)| *s == seq).map(|(_, d)| d.clone());
                Some((this.cache_key_inputs(cx), this.display.clone(), coarse, this.ui.cache_key.clone()))
            });
            let Ok(Some(((doc, tab, config, fine), display, coarse, known))) = job else { return };
            let saved = cx
                .background_spawn(async move {
                    let key = display_cache::key(&doc, tab, &config, fine)?;
                    if known.as_ref() == Some(&key) {
                        return Some(key);
                    }
                    let dir = display_cache::default_dir();
                    if let Some(coarse) = &coarse {
                        display_cache::store(&dir, &coarse_key(&key), coarse).ok()?;
                    }
                    display_cache::store(&dir, &key, &display).ok()?;
                    Some(key)
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, _| {
                this.ui.cache_save_task = None;
                if saved.is_some() {
                    this.ui.cache_key = saved;
                }
            });
        }));
    }

    /// The feature list's top row while the cached display stands in.
    pub(super) fn render_regenerating_row(&self) -> Option<AnyElement> {
        self.ui.cached_view.then(crate::display_cache::regenerating_row)
    }
}
