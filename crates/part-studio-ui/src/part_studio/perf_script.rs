//! `NOSHAPE_PERF_SCRIPT` (benchmarks, perf.yml `env`), once the opened Part
//! Studio is shown, logging how long each step took to show (target `perf`):
//! `delete-undo-last` deletes its last feature as the Delete key does, then
//! undoes; `edit-undo-redo` commits a new depth for its first Extrude (the
//! feature dialog's commit: every feature after it regenerates), then undoes
//! and redoes. `NOSHAPE_PERF_SCRIPT_WAIT_MS=<ms>`: `edit-undo-redo` starts that long after the studio is shown
//! (COMMIT-LATENCY: the runners' first frames after opening take 250–500 ms on the GPU and hold the UI thread).

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
    /// `edit-undo-redo`: waiting for the opened studio.
    EditOpen,
    /// Shown, waiting `NOSHAPE_PERF_SCRIPT_WAIT_MS` before the edit.
    EditWaiting,
    /// The wait is over: edit now.
    EditNow,
    /// Extrude 1's depth committed then.
    Edited(Instant),
    EditUndone(Instant),
    Redone(Instant),
    Done,
}

/// The script asked for in the environment, if any.
pub(super) fn from_env() -> Option<Script> {
    match std::env::var("NOSHAPE_PERF_SCRIPT").ok()?.as_str() {
        "delete-undo-last" => Some(Script::Open),
        "edit-undo-redo" => Some(Script::EditOpen),
        _ => None,
    }
}

impl PartStudioView {
    /// A stage of the step in progress (the regeneration scheduled, its result in, its display applied), logged
    /// with the time since the step began: where an undo's time goes (COMMIT-LATENCY).
    pub(super) fn perf_script_note(&self, what: &str) {
        if let Some(Script::Edited(t) | Script::EditUndone(t) | Script::Redone(t) | Script::Deleted(t) | Script::Undone(t)) = &self.perf_script {
            log::info!(target: "perf", "perf script: {what} at {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
        }
    }

    /// A full display was applied: the script's next step.
    pub(super) fn perf_script_step(&mut self, cx: &mut Context<Self>) {
        let Some(step) = self.perf_script.take() else { return };
        // What the UI thread spent since the last step, frames included (COMMIT-LATENCY: an undo's regeneration task
        // sometimes first runs 200–300 ms after it was asked, with nothing else logged meanwhile).
        log::info!(target: "perf", "perf script: stages since the last step:");
        noshape_render::perf::flush();
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
            Script::EditOpen | Script::EditNow => {
                if matches!(step, Script::EditOpen) {
                    log::info!(target: "perf", "perf script: open shown {:.0} ms after start", noshape_render::perf::process_start().elapsed().as_secs_f64() * 1e3);
                    let wait = std::env::var("NOSHAPE_PERF_SCRIPT_WAIT_MS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
                    if wait > 0 {
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(std::time::Duration::from_millis(wait)).await;
                            let _ = this.update(cx, |this: &mut Self, cx| {
                                if matches!(this.perf_script, Some(Script::EditWaiting)) {
                                    log::info!(target: "perf", "perf script: waited {wait} ms");
                                    this.perf_script = Some(Script::EditNow);
                                    this.perf_script_step(cx);
                                }
                            });
                        })
                        .detach();
                        self.perf_script = Some(Script::EditWaiting);
                        return;
                    }
                }
                let tab = self.tab;
                let edited = self.doc.update(cx, |doc, cx| {
                    doc.edit_studio(tab, "Edit Extrude 1", cx, |ps| {
                        let f = ps.features.iter_mut().find(|f| f.type_id == "extrude")?;
                        let Some(noshape_model::params::ParamValue::Expr(depth)) = f.params.get("depth") else { return None };
                        let deeper = format!("({}) + 1 mm", depth.text);
                        f.params.set("depth", noshape_model::params::ParamValue::Expr(noshape_units::ExprValue::new(deeper)));
                        Some(())
                    })
                });
                if edited.flatten().is_none() {
                    log::info!(target: "perf", "perf script: no Extrude depth to edit");
                    return;
                }
                Script::Edited(Instant::now())
            }
            Script::Edited(t) => {
                log::info!(target: "perf", "perf script: commit shown in {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
                let t = Instant::now();
                self.doc.update(cx, |doc, cx| doc.undo(cx));
                log::info!(target: "perf", "perf script: undo call {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
                Script::EditUndone(t)
            }
            Script::EditUndone(t) => {
                log::info!(target: "perf", "perf script: undo shown in {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
                let t = Instant::now();
                self.doc.update(cx, |doc, cx| doc.redo(cx));
                log::info!(target: "perf", "perf script: redo call {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
                Script::Redone(t)
            }
            Script::Redone(t) => {
                log::info!(target: "perf", "perf script: redo shown in {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
                Script::Done
            }
            Script::EditWaiting => Script::EditWaiting,
            Script::Done => Script::Done,
        });
    }
}
