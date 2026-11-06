//! Opening on the cached display (`display_cache`): the Part Studio shows its last display at once while the real
//! regeneration runs. Until that lands the view is view-only (`cached_view`): orbit, pan and zoom, a "Regenerating…"
//! row at the top of the feature list, and an action that needs the real model (a feature dialog) says so at once and
//! runs when it lands (lead 11-06: never silence). The real result replaces the cached one in the frame it arrives
//! (its scene is built before it is swapped in: no flash).

use super::{PartStudioView, display_cache};
use gpui_kit::*;
use noshape_model::display::FeatureStatus;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static ENABLED: AtomicBool = AtomicBool::new(false);

/// The app keeps and uses the display cache (called once at startup; tests and the parity driver don't).
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// An action that waits for the real model.
pub(super) type Deferred = Box<dyn FnOnce(&mut PartStudioView, &mut Window, &mut Context<PartStudioView>)>;

/// The message while an action waits for the regeneration.
pub(super) const WAITING: &str = "Regenerating the model: this opens as soon as it is ready.";

/// How long after a full display the cache is written (edits in a row write once).
const SAVE_AFTER: Duration = Duration::from_secs(2);

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
        let found = cx.background_spawn(async move {
            let key = display_cache::key(&doc, tab, &config, fine)?;
            let display = display_cache::load(&display_cache::default_dir(), &key)?;
            Some((key, display))
        });
        self.ui.cache_task = Some(cx.spawn_in(window, async move |this, cx| {
            let Some((key, display)) = found.await else { return };
            let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                this.ui.cache_task = None;
                this.show_cached_display(key, display, window, cx);
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
                Some((this.cache_key_inputs(cx), this.display.clone(), this.ui.cache_key.clone()))
            });
            let Ok(Some(((doc, tab, config, fine), display, known))) = job else { return };
            let saved = cx
                .background_spawn(async move {
                    let key = display_cache::key(&doc, tab, &config, fine)?;
                    if known.as_ref() == Some(&key) {
                        return Some(key);
                    }
                    display_cache::store(&display_cache::default_dir(), &key, &display).ok()?;
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

    /// The feature list's top row while the cached display stands in: Onshape's spinner and "Regenerating…".
    pub(super) fn render_regenerating_row(&self) -> Option<AnyElement> {
        if !self.ui.cached_view {
            return None;
        }
        let pal = crate::theme::p();
        Some(
            gpui_kit::component::h_flex()
                .id("regenerating-row")
                .h(px(crate::ui::ROW_H))
                .flex_none()
                .gap(px(6.))
                .pl(px(9.))
                .child(super::pending::spinner(18.))
                .child(div().text_size(px(crate::ui::ROW_TEXT)).text_color(pal.text_muted).child("Regenerating…"))
                .into_any_element(),
        )
    }
}
