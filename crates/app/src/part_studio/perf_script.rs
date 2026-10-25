//! `NOSHAPE_PERF_SCRIPT=delete-undo-last` (benchmarks, perf.yml `env`): once
//! the opened Part Studio is shown, delete its last feature as the Delete key
//! does, then undo, logging how long each took to show (target `perf`).

use super::PartStudioView;
use gpui_kit::*;
use std::time::Instant;

pub(super) enum Script {
    /// Waiting for the opened studio's first full display.
    Open,
    /// The last feature deleted then.
    Deleted(Instant),
    /// Undone then.
    Undone(Instant),
    Done,
}

/// The script asked for in the environment, if any.
pub(super) fn from_env() -> Option<Script> {
    (std::env::var("NOSHAPE_PERF_SCRIPT").ok()? == "delete-undo-last").then_some(Script::Open)
}

impl PartStudioView {
    /// A full display was applied: the script's next step.
    pub(super) fn perf_script_step(&mut self, cx: &mut Context<Self>) {
        let Some(step) = self.perf_script.take() else { return };
        self.perf_script = Some(match step {
            Script::Open => {
                log::info!(target: "perf", "perf script: open shown {:.0} ms after start", noshape_render::perf::process_start().elapsed().as_secs_f64() * 1e3);
                let Some(last) = self.studio(cx).and_then(|s| s.features.last()).map(|f| f.id) else {
                    log::info!(target: "perf", "perf script: no feature to delete");
                    return;
                };
                self.delete_features(vec![last], cx);
                Script::Deleted(Instant::now())
            }
            Script::Deleted(t) => {
                log::info!(target: "perf", "perf script: delete shown in {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
                self.doc.update(cx, |doc, cx| doc.undo(cx));
                Script::Undone(Instant::now())
            }
            Script::Undone(t) => {
                log::info!(target: "perf", "perf script: undo shown in {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
                Script::Done
            }
            Script::Done => Script::Done,
        });
    }
}
