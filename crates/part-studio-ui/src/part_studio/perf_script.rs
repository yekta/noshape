//! `NOSHAPE_PERF_SCRIPT` (benchmarks, perf.yml `env`), once the opened Part
//! Studio is shown, logging how long each step took to show (target `perf`):
//! `delete-undo-last` deletes its last feature as the Delete key does, then
//! undoes; `edit-undo-redo` commits a new depth for its first Extrude (the
//! feature dialog's commit: every feature after it regenerates), then undoes
//! and redoes.

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
            Script::EditOpen => {
                log::info!(target: "perf", "perf script: open shown {:.0} ms after start", noshape_render::perf::process_start().elapsed().as_secs_f64() * 1e3);
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
                self.doc.update(cx, |doc, cx| doc.undo(cx));
                Script::EditUndone(Instant::now())
            }
            Script::EditUndone(t) => {
                log::info!(target: "perf", "perf script: undo shown in {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
                self.doc.update(cx, |doc, cx| doc.redo(cx));
                Script::Redone(Instant::now())
            }
            Script::Redone(t) => {
                log::info!(target: "perf", "perf script: redo shown in {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
                Script::Done
            }
            Script::Done => Script::Done,
        });
    }
}
