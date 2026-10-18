//! Background render thread: the UI thread submits jobs and receives frames
//! without ever blocking on the GPU.
//!
//! * Pending jobs are coalesced per target, so only the newest one renders.
//! * Frames are pipelined: on real GPUs a target may have two frames in
//!   flight, so frame N+1 is encoded and rendered while frame N is being
//!   read back and copied out (each target has its own readback buffers).
//! * Pick queries read small rectangles of the (lazily rendered) pick buffer.

use crate::camera::Camera;
use crate::perf;
use crate::renderer::{Completed, FrameRequest, PickRect, RenderError, Renderer, Uploader};
use crate::scene::{Scene, Style};
use std::sync::Arc;
use std::sync::mpsc;


#[derive(Clone)]
pub struct RenderJob {
    /// Which viewport this frame is for; echoed back to the callback.
    pub target: u64,
    pub scene: Arc<Scene>,
    pub camera: Camera,
    pub width: u32,
    pub height: u32,
    pub device_pixel_ratio: f32,
    pub style: Style,
    /// The geometry, camera or pick state changed since the last frame, so
    /// the pick buffer must be re-rendered before the next pick query.
    pub pick: bool,
    /// Monotonic sequence number echoed back with the frame.
    pub seq: u64,
}

enum Msg {
    Render(Box<RenderJob>),
    Pick { target: u64, token: u64, rect: PickRect },
    Drop(u64),
}

pub struct RenderWorker {
    tx: mpsc::Sender<Msg>,
    uploader: Uploader,
    adapter: String,
    frames_in_flight: usize,
}

impl RenderWorker {
    /// Spawns the render thread. `on_done` is called on the render thread for
    /// every completed frame and pick read.
    pub fn spawn(on_done: impl Fn(Completed) + Send + 'static) -> Result<Self, RenderError> {
        let mut renderer = Renderer::new()?;
        let uploader = renderer.uploader();
        let adapter = renderer.adapter_name();
        let frames_in_flight = renderer.frames_in_flight();
        let (tx, rx) = mpsc::channel::<Msg>();
        std::thread::Builder::new()
            .name("noshape-render".into())
            .spawn(move || {
                renderer.prewarm(Style::default().msaa);
                let mut jobs: Vec<RenderJob> = Vec::new();
                let mut picks: Vec<(u64, u64, PickRect)> = Vec::new();
                let mut connected = true;
                let absorb = |m: Msg, jobs: &mut Vec<RenderJob>, picks: &mut Vec<(u64, u64, PickRect)>, renderer: &mut Renderer| match m {
                    Msg::Render(job) => {
                        let job = *job;
                        // Coalesce: keep only the newest waiting job per target,
                        // but never lose a pick invalidation.
                        if let Some(slot) = jobs.iter_mut().find(|j| j.target == job.target) {
                            let pick = slot.pick || job.pick;
                            *slot = job;
                            slot.pick = pick;
                            perf::record_ms("w.coalesced", 1.0);
                        } else {
                            jobs.push(job);
                        }
                    }
                    Msg::Pick { target, token, rect } => picks.push((target, token, rect)),
                    Msg::Drop(target) => {
                        jobs.retain(|j| j.target != target);
                        picks.retain(|p| p.0 != target);
                        renderer.drop_target(target);
                    }
                };
                loop {
                    // Block for input only when there is nothing else to do.
                    if jobs.is_empty() && picks.is_empty() && !renderer.has_pending() {
                        if !connected {
                            break;
                        }
                        match rx.recv() {
                            Ok(m) => absorb(m, &mut jobs, &mut picks, &mut renderer),
                            Err(_) => break,
                        }
                    }
                    loop {
                        match rx.try_recv() {
                            Ok(m) => absorb(m, &mut jobs, &mut picks, &mut renderer),
                            Err(mpsc::TryRecvError::Empty) => break,
                            Err(mpsc::TryRecvError::Disconnected) => {
                                connected = false;
                                break;
                            }
                        }
                    }
                    // Submit frames for targets with a free slot.
                    let mut i = 0;
                    while i < jobs.len() {
                        if renderer.can_submit(jobs[i].target) {
                            let job = jobs.swap_remove(i);
                            let (target, seq) = (job.target, job.seq);
                            let req = FrameRequest {
                                scene: job.scene,
                                camera: job.camera,
                                width: job.width,
                                height: job.height,
                                device_pixel_ratio: job.device_pixel_ratio,
                                style: job.style,
                                invalidate_pick: job.pick,
                            };
                            if let Err(e) = renderer.submit_frame(target, seq, req) {
                                on_done(Completed::Frame { target, seq, result: Err(e) });
                            }
                        } else {
                            i += 1;
                        }
                    }
                    // Pick queries must see the newest frame of their target:
                    // hold them while a newer frame is still waiting.
                    if !picks.is_empty() {
                        let mut targets: Vec<u64> = picks.iter().map(|p| p.0).filter(|t| !jobs.iter().any(|j| j.target == *t)).collect();
                        targets.sort_unstable();
                        targets.dedup();
                        for t in targets {
                            let queries: Vec<(u64, PickRect)> = picks.iter().filter(|p| p.0 == t).map(|p| (p.1, p.2)).collect();
                            picks.retain(|p| p.0 != t);
                            renderer.submit_picks(t, &queries);
                        }
                    }
                    let done = renderer.collect();
                    let delivered = !done.is_empty();
                    for c in done {
                        on_done(c);
                    }
                    if !delivered && renderer.has_pending() {
                        let _t = perf::scope("w.gpu_wait");
                        renderer.wait_oldest();
                    }
                }
            })
            .expect("spawn render thread");
        Ok(Self { tx, uploader, adapter, frames_in_flight })
    }

    pub fn submit(&self, job: RenderJob) {
        let _ = self.tx.send(Msg::Render(Box::new(job)));
    }

    /// Read a rectangle (device pixels) of `target`'s pick buffer for its
    /// newest frame; the result is reported with `token`.
    pub fn pick(&self, target: u64, token: u64, rect: PickRect) {
        let _ = self.tx.send(Msg::Pick { target, token, rect });
    }

    /// Release a target's GPU resources.
    pub fn drop_target(&self, target: u64) {
        let _ = self.tx.send(Msg::Drop(target));
    }

    pub fn uploader(&self) -> &Uploader {
        &self.uploader
    }

    pub fn adapter_name(&self) -> &str {
        &self.adapter
    }

    /// Frames a target may have in flight (submitting more only queues them).
    pub fn frames_in_flight(&self) -> usize {
        self.frames_in_flight
    }
}
