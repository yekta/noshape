//! `NOSHAPE_PERF_SCRIPT` (benchmarks, perf.yml `env`), once the opened Part
//! Studio is shown, logging how long each step took to show (target `perf`):
//! `delete-undo-last` deletes its last feature as the Delete key does, then
//! undoes; `edit-undo-redo` commits a new depth for its first Extrude (the
//! feature dialog's commit: every feature after it regenerates), then undoes
//! and redoes. `NOSHAPE_PERF_SCRIPT_WAIT_MS=<ms>`: `edit-undo-redo` starts that long after the studio is shown
//! (COMMIT-LATENCY: the runners' first frames after opening take 250–500 ms on the GPU and hold the UI thread).
//! `ui-latency` times the UI thread's part of showing an action, from the action to the end of the paint of the
//! first frame that draws it (its present to the screen excluded): a feature dialog opening (Edit on each of the last
//! features in turn, cancelled after) and a feature row click (the list's and the graphics' highlight), 20 each, then
//! logs `BENCH SUMMARY dialog-open` / `row-click`.

use super::PartStudioView;
use gpui_kit::*;
use std::time::Instant;

pub(super) enum Script {
    /// `open`: waiting for the opened studio's first full display, then quitting once its frame is up.
    OpenOnly,
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
    /// `ui-latency`: waiting for the opened studio, then running (`run_ui_latency`).
    LatencyOpen,
    Latency,
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
        "open" => Some(Script::OpenOnly),
        "delete-undo-last" => Some(Script::Open),
        "edit-undo-redo" => Some(Script::EditOpen),
        "ui-latency" => Some(Script::LatencyOpen),
        _ => None,
    }
}

/// A window frame's stages on the UI thread (gpui's own timings).
const FRAME_STAGES: [&str; 3] = ["gpui.request_layout", "gpui.prepaint", "gpui.paint"];
/// Their UI-thread CPU time (app-core gpu.rs `draw_stat`).
const FRAME_STAGES_CPU: [&str; 3] = ["gpui.request_layout_cpu", "gpui.prepaint_cpu", "gpui.paint_cpu"];
/// Where a ui-latency sample's time goes (wall ms, summed over the action and its frame): applying the regenerated
/// model (on the UI thread: its start and, once the scene is built, its end) and the scene build (a background
/// thread for a dialog's preview), the Part Studio's and the dialog's renders, each slice's, then GPUI's phases.
/// A ui-latency sample: (the action, waiting, the frame, the action's CPU, the frame's CPU), and [`PARTS`].
type Sample = ([f32; 5], [f32; PARTS.len()]);
const PARTS: [&str; 15] = ["ps.apply_display", "ps.apply_display_end", "s.build", "ps.render", "fd.render", "ps.slice_toolbar", "ps.slice_underlay", "ps.slice_overlay", "ps.slice_features", "ps.slice_parts", "ps.row_feature", "ps.row_part", "gpui.request_layout", "gpui.prepaint", "gpui.paint"];

impl PartStudioView {
    /// A stage of the step in progress (the regeneration scheduled, its result in, its display applied), logged
    /// with the time since the step began: where an undo's time goes (COMMIT-LATENCY).
    /// `NOSHAPE_PERF`: the opened studio's first preview / full display is about to be applied: its regeneration
    /// time logged, the frame that first draws it armed (`perf::arm_frame_milestone`: document open → model on
    /// screen, the `open` script and tools/perf/open-times.py).
    pub(super) fn note_open_display(&mut self, full: bool, regen_ms: f32) {
        if !noshape_render::perf::enabled() {
            return;
        }
        let stage = if full { 2 } else { 1 };
        if self.ui.open_frames >= stage {
            return;
        }
        self.ui.open_frames = stage;
        let what = if full { "full" } else { "preview" };
        noshape_render::perf::milestone(&format!("open: {what} display in (regen {regen_ms:.1} ms)"));
        noshape_render::perf::arm_frame_milestone(format!("open: {what} model frame presented"));
    }

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
            Script::OpenOnly => {
                log::info!(target: "perf", "perf script: open shown {:.0} ms after start", noshape_render::perf::process_start().elapsed().as_secs_f64() * 1e3);
                // Long enough for the armed frame to be presented and logged.
                cx.spawn(async move |_, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
                    cx.update(|cx| cx.quit());
                })
                .detach();
                Script::Done
            }
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
            Script::LatencyOpen => {
                self.run_ui_latency(cx);
                Script::Latency
            }
            Script::Latency => Script::Latency,
            Script::EditWaiting => Script::EditWaiting,
            Script::Done => Script::Done,
        });
    }

    /// `ui-latency`: the dialog-open and row-click samples, one at a time, 300 ms apart.
    fn run_ui_latency(&mut self, cx: &mut Context<Self>) {
        let Some(window) = cx.active_window().or_else(|| cx.windows().first().copied()) else { return };
        let features: Vec<noshape_model::reference::FeatureId> = self.studio(cx).map(|s| s.features.iter().rev().filter(|f| f.type_id != "sketch").take(4).map(|f| f.id).collect()).unwrap_or_default();
        if features.is_empty() {
            log::info!(target: "perf", "perf script: no feature to open");
            return;
        }
        log::info!(target: "perf", "perf script: ui-latency on {} features", features.len());
        cx.spawn(async move |this, cx| {
            let pause = || std::time::Duration::from_millis(300);
            for (kind, n) in [("dialog-open", 20usize), ("row-click", 20)] {
                let mut ms = Vec::new();
                let (mut handler, mut wait, mut frame) = (Vec::new(), Vec::new(), Vec::new());
                let mut cpu = Vec::new();
                let mut parts: Vec<Vec<f32>> = vec![Vec::new(); PARTS.len()];
                for k in 0..n {
                    cx.background_executor().timer(pause()).await;
                    let id = features[k % features.len()];
                    let shown: std::sync::Arc<std::sync::Mutex<Option<Sample>>> = Default::default();
                    let tx = shown.clone();
                    let ok = cx.update_window(window, |_, w, cx| {
                        for st in FRAME_STAGES.iter().chain(&FRAME_STAGES_CPU).chain(PARTS.iter().filter(|p| !FRAME_STAGES.contains(p))) {
                            noshape_render::perf::series_start(st);
                        }
                        let t0 = Instant::now();
                        let c0 = noshape_render::perf::thread_cpu_time().unwrap_or_default();
                        let _ = this.update(cx, |this: &mut Self, cx| match kind {
                            "dialog-open" => this.edit_feature(id, w, cx),
                            _ => this.click_feature(id, false, cx),
                        });
                        let handler = t0.elapsed().as_secs_f32() * 1e3;
                        let handler_cpu = noshape_render::perf::thread_cpu_time().unwrap_or_default().saturating_sub(c0).as_secs_f32() * 1e3;
                        // The first frame after the action draws it: its start, then its layout, prepaint and paint.
                        w.on_next_frame(move |w, _| {
                            let start = t0.elapsed().as_secs_f32() * 1e3;
                            w.on_next_frame(move |_, _| {
                                let firsts: Vec<f32> = FRAME_STAGES.iter().map(|st| noshape_render::perf::series_take(st).first().copied().unwrap_or(0.0)).collect();
                                let draw: f32 = firsts.iter().sum();
                                let draw_cpu: f32 = FRAME_STAGES_CPU.iter().map(|st| noshape_render::perf::series_take(st).first().copied().unwrap_or(0.0)).sum();
                                let parts = PARTS.map(|p| match FRAME_STAGES.iter().position(|f| *f == p) {
                                    Some(i) => firsts[i],
                                    None => {
                                        let v = noshape_render::perf::series_take(p);
                                        // Whole ms, and the count in thousandths.
                                        v.iter().sum::<f32>().round() + v.len().min(999) as f32 / 1000.0
                                    }
                                });
                                *tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(([handler, start - handler, draw, handler_cpu, draw_cpu], parts));
                            });
                        });
                    });
                    if ok.is_err() {
                        return;
                    }
                    for _ in 0..1000 {
                        if let Some(([h, w, f, hc, fc], ps)) = *shown.lock().unwrap_or_else(|e| e.into_inner()) {
                            let each: Vec<String> = PARTS.iter().zip(ps).filter(|(_, x)| *x >= 0.5).map(|(p, x)| format!("{p} {x:.1}")).collect();
                            log::info!(target: "perf", "BENCH SAMPLE {kind} {k}: action {h:.1} wait {w:.1} frame {f:.1} ms, CPU {:.1} ms | {}", hc + fc, each.join(", "));
                            parts.iter_mut().zip(ps).for_each(|(v, x)| v.push(x));
                            ms.push(h + w + f);
                            cpu.push(hc + fc);
                            handler.push(h);
                            wait.push(w);
                            frame.push(f);
                            break;
                        }
                        cx.background_executor().timer(std::time::Duration::from_millis(2)).await;
                    }
                    if kind == "dialog-open" {
                        cx.background_executor().timer(pause()).await;
                        let _ = cx.update_window(window, |_, w, cx| {
                            let _ = this.update(cx, |this: &mut Self, cx| this.cancel_edit(w, cx));
                        });
                    }
                }
                let st = noshape_render::perf::Stats::of(&mut ms);
                let (h, w, f) = (noshape_render::perf::Stats::of(&mut handler), noshape_render::perf::Stats::of(&mut wait), noshape_render::perf::Stats::of(&mut frame));
                let c = noshape_render::perf::Stats::of(&mut cpu);
                let split: Vec<String> = PARTS.iter().zip(&mut parts).map(|(p, v)| format!("{p} {:.3}", noshape_render::perf::Stats::of(v).p50)).collect();
                log::info!(target: "perf", "BENCH SUMMARY {kind}: {} samples | action to the end of the paint of the first frame showing it mean {:.1} p50 {:.1} p95 {:.1} max {:.1} ms | the action p50 {:.1} p95 {:.1}, waiting for the frame p50 {:.1} p95 {:.1}, the frame's layout + prepaint + paint p50 {:.1} p95 {:.1} ms | UI-thread CPU (the action + that frame) p50 {:.1} p95 {:.1} ms", ms.len(), st.mean, st.p50, st.p95, st.max, h.p50, h.p95, w.p50, w.p95, f.p50, f.p95, c.p50, c.p95);
                log::info!(target: "perf", "BENCH SPLIT {kind} (p50 ms.calls, nested): {}", split.join(", "));
            }
            noshape_render::perf::flush();
        })
        .detach();
    }
}
