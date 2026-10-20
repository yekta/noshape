//! NOSHAPE PATCH (agent perf): per-phase timings of window draws for the
//! application's own instrumentation (noshape's `NOSHAPE_PERF`). Off (no
//! clock reads) until a hook is set.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Receives a phase name and its duration on the UI thread:
/// `gpui.request_layout` (views' `render` and layout requests),
/// `gpui.prepaint` (layout computation and prepaint), `gpui.paint`, and
/// `gpui.present` (encoding and submitting the scene to the GPU).
pub type DrawStatsHook = fn(&'static str, Duration);

static HOOK: OnceLock<DrawStatsHook> = OnceLock::new();

/// Install the hook (once per process).
pub fn set_draw_stats_hook(hook: DrawStatsHook) {
    let _ = HOOK.set(hook);
}

/// The current time, if a hook is installed.
pub(crate) fn draw_stats_now() -> Option<Instant> {
    HOOK.get().map(|_| Instant::now())
}

/// Report `phase` as the time since `start` (no-op without a hook).
pub(crate) fn draw_stat(phase: &'static str, start: Option<Instant>) {
    if let (Some(hook), Some(start)) = (HOOK.get(), start) {
        hook(phase, start.elapsed());
    }
}
