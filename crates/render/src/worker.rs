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
use crate::renderer::{Completed, FrameRequest, PickRect, Present, RenderError, Renderer, Uploader};
use crate::scene::{Scene, Style};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

static PREWARM_HELD: AtomicBool = AtomicBool::new(false);
static PREWARM_RELEASED: AtomicBool = AtomicBool::new(false);
/// The longest a held shader compile waits for [`release_prewarm`].
const PREWARM_MAX_HOLD: Duration = Duration::from_secs(5);

/// Hold the render worker's shader compile ([`Renderer::prewarm`]) until
/// [`release_prewarm`]: compiled alongside the first window's own shaders,
/// ours held the window back (macOS 14 runners: drawn at 2.7 s, 1.2 s
/// without). Call before the worker starts.
pub fn hold_prewarm() {
    PREWARM_HELD.store(true, Ordering::Relaxed);
}

/// The first window has drawn: compile now.
pub fn release_prewarm() {
    PREWARM_RELEASED.store(true, Ordering::Relaxed);
}

/// Compile at once instead of after the first window draw, on low-priority threads ([`low_priority_compile`]) so the
/// window's own work comes first: on 8 or more cores, where it doesn't slow the window (the Mac mini's dock: the first
/// frame waited 240 ms for the compile started at the window's draw); the hold is for small machines (3-core macOS
/// runners drew their window at 2.7 s against 1.2 s). `NOSHAPE_PREWARM=early` / `held` force either.
pub fn prewarm_early() -> bool {
    static EARLY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *EARLY.get_or_init(|| match std::env::var("NOSHAPE_PREWARM").as_deref() {
        Ok("early") => true,
        Ok("held") => false,
        _ => std::thread::available_parallelism().is_ok_and(|n| n.get() >= 8),
    })
}

/// macOS: run this thread's work (and the Metal compiler work it asks for) at utility priority.
pub fn low_priority_compile() {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
        }
        const QOS_CLASS_UTILITY: u32 = 0x11;
        // SAFETY: sets the calling thread's own QoS class; no memory is involved.
        unsafe {
            pthread_set_qos_class_self_np(QOS_CLASS_UTILITY, 0);
        }
    }
}

fn wait_for_prewarm(closing: &AtomicBool) {
    if !PREWARM_HELD.load(Ordering::Relaxed) || prewarm_early() {
        return;
    }
    let deadline = Instant::now() + PREWARM_MAX_HOLD;
    while !PREWARM_RELEASED.load(Ordering::Relaxed) && !closing.load(Ordering::Relaxed) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
}


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

impl RenderJob {
    fn request(self) -> FrameRequest {
        FrameRequest {
            scene: self.scene,
            camera: self.camera,
            width: self.width,
            height: self.height,
            device_pixel_ratio: self.device_pixel_ratio,
            style: self.style,
            invalidate_pick: self.pick,
        }
    }
}

/// How long a software rasteriser's view stays unchanged before its frame is drawn again at the full sample count.
const SETTLE: Duration = Duration::from_millis(150);

static FULL_FRAMES_ONLY: AtomicBool = AtomicBool::new(false);

/// Tests: a software rasteriser's frames drawn at their full sample count at once, never a 1x frame settled later, so a
/// frame checked shows the same pixels however slow the runner (Windows CI's WARP, nightly 45490384199).
pub fn full_frames_only(on: bool) {
    FULL_FRAMES_ONLY.store(on, Ordering::Relaxed);
}

/// A software rasteriser's frames (lead 11-22: 4x MSAA on llvmpipe was ~7x the frame): 1x while the view changes
/// (orbit, drag, animation, hover), the last one again at the style's sample count once it has settled.
#[derive(Default)]
struct Settle {
    /// Per target: its newest job at the full sample count, due then.
    due: Vec<(RenderJob, Instant)>,
}

impl Settle {
    /// `job`'s frame now (1x when it has more samples), its full frame queued for [`SETTLE`] after `now`.
    fn moving(&mut self, job: RenderJob, now: Instant) -> FrameRequest {
        if job.style.msaa <= 1 {
            self.forget(job.target);
            return job.request();
        }
        self.queue(job.clone(), now);
        FrameRequest { style: Style { msaa: 1, ..job.style }, ..job.request() }
    }

    /// A frame shown again without drawing ([`RenderWorker::adopt`]): its full frame once settled too.
    fn queue(&mut self, job: RenderJob, now: Instant) {
        self.forget(job.target);
        if job.style.msaa > 1 {
            self.due.push((RenderJob { pick: false, ..job }, now + SETTLE));
        }
    }

    fn forget(&mut self, target: u64) {
        self.due.retain(|d| d.0.target != target);
    }

    /// The settled job due by `now` whose target `ready` takes, taken.
    fn take_due(&mut self, now: Instant, ready: impl Fn(u64) -> bool) -> Option<RenderJob> {
        let i = self.due.iter().position(|d| d.1 <= now && ready(d.0.target))?;
        Some(self.due.swap_remove(i).0)
    }

    /// How long until the next one is due (none: nothing to wait for).
    fn wait(&self, now: Instant) -> Option<Duration> {
        // (At least 1 ms: a due job whose target has no free slot yet must not spin the thread.)
        self.due.iter().map(|d| d.1.saturating_duration_since(now).max(Duration::from_millis(1))).min()
    }
}

enum Msg {
    Render(Box<RenderJob>),
    Adopt(Box<RenderJob>),
    Pick { target: u64, token: u64, rect: PickRect },
    SetPresent(u64, Present),
    SetOverlay(u64, Option<Arc<crate::overlay::Overlay>>),
    SetCameraGeometry(u64, Vec<Arc<dyn crate::camera_geometry::CameraGeometry>>),
    SetTextShading(crate::overlay::TextShading),
    Drop(u64),
}

pub struct RenderWorker {
    tx: mpsc::Sender<Msg>,
    /// The render thread, joined on drop: its device torn down before its owner goes on (a test binary's exit ran
    /// Mesa's and LLVM's destructors under a thread still compiling: assembly-ui's SIGSEGV after its tests, CI 11-19).
    thread: Option<std::thread::JoinHandle<()>>,
    /// Set on drop: a thread not yet past its shader compile skips it.
    closing: Arc<AtomicBool>,
    uploader: Uploader,
    adapter: String,
    software: bool,
    frames_in_flight: usize,
}

impl RenderWorker {
    /// Spawns the render thread. `on_done` is called on the render thread for
    /// every completed frame and pick read.
    pub fn spawn(on_done: impl Fn(Completed) + Send + 'static) -> Result<Self, RenderError> {
        Ok(Self::spawn_with(Renderer::new()?, on_done))
    }

    /// [`Self::spawn`] with an existing renderer (e.g. one on the UI
    /// toolkit's device, [`Renderer::with_device`]).
    pub fn spawn_with(mut renderer: Renderer, on_done: impl Fn(Completed) + Send + 'static) -> Self {
        let uploader = renderer.uploader();
        let adapter = renderer.adapter_name();
        let software = renderer.is_software();
        let frames_in_flight = renderer.frames_in_flight();
        let (tx, rx) = mpsc::channel::<Msg>();
        let closing = Arc::new(AtomicBool::new(false));
        let closed = closing.clone();
        let thread = std::thread::Builder::new()
            .name("noshape-render".into())
            .spawn(move || {
                wait_for_prewarm(&closed);
                if !closed.load(Ordering::Relaxed) {
                    renderer.prewarm(Style::default().msaa);
                }
                let mut jobs: Vec<RenderJob> = Vec::new();
                let mut picks: Vec<(u64, u64, PickRect)> = Vec::new();
                let mut connected = true;
                let mut cpu_at_frame: Option<std::time::Duration> = None;
                let mut settle = Settle::default();
                let absorb = |m: Msg, jobs: &mut Vec<RenderJob>, picks: &mut Vec<(u64, u64, PickRect)>, renderer: &mut Renderer, settle: &mut Settle| match m {
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
                    Msg::Adopt(job) => {
                        if software && !FULL_FRAMES_ONLY.load(Ordering::Relaxed) {
                            settle.queue((*job).clone(), Instant::now());
                        }
                        renderer.adopt_frame(job.target, job.request())
                    }
                    Msg::Pick { target, token, rect } => picks.push((target, token, rect)),
                    Msg::SetPresent(target, present) => renderer.set_present(target, present),
                    Msg::SetOverlay(target, overlay) => renderer.set_overlay(target, overlay),
                    Msg::SetCameraGeometry(target, geometry) => renderer.set_camera_geometry(target, geometry),
                    Msg::SetTextShading(shading) => renderer.set_text_shading(shading),
                    Msg::Drop(target) => {
                        settle.forget(target);
                        jobs.retain(|j| j.target != target);
                        picks.retain(|p| p.0 != target);
                        renderer.drop_target(target);
                    }
                };
                loop {
                    // No one left to deliver to: only the GPU's frames in flight are seen to the end.
                    if !connected {
                        jobs.clear();
                        picks.clear();
                        settle.due.clear();
                    }
                    // Block for input only when there is nothing else to do.
                    if jobs.is_empty() && picks.is_empty() && !renderer.has_pending() {
                        if !connected {
                            break;
                        }
                        match settle.wait(Instant::now()) {
                            None => match rx.recv() {
                                Ok(m) => absorb(m, &mut jobs, &mut picks, &mut renderer, &mut settle),
                                Err(_) => break,
                            },
                            Some(wait) => match rx.recv_timeout(wait) {
                                Ok(m) => absorb(m, &mut jobs, &mut picks, &mut renderer, &mut settle),
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                Err(mpsc::RecvTimeoutError::Disconnected) => connected = false,
                            },
                        }
                    } else if picks.is_empty() && !renderer.has_pending() && !jobs.iter().any(|j| renderer.can_submit(j.target)) {
                        // Only jobs whose target waits for the UI to release a
                        // presented texture: nothing signals that, so look
                        // again shortly (or as soon as a message arrives).
                        match rx.recv_timeout(std::time::Duration::from_millis(1)) {
                            Ok(m) => absorb(m, &mut jobs, &mut picks, &mut renderer, &mut settle),
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                            Err(mpsc::RecvTimeoutError::Disconnected) => connected = false,
                        }
                    }
                    loop {
                        match rx.try_recv() {
                            Ok(m) => absorb(m, &mut jobs, &mut picks, &mut renderer, &mut settle),
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
                            let req = if software && !FULL_FRAMES_ONLY.load(Ordering::Relaxed) { settle.moving(job, Instant::now()) } else { job.request() };
                            perf::frame_milestone(seq, "taken by the render thread");
                            if let Err(e) = renderer.submit_frame(target, seq, req) {
                                on_done(Completed::Frame { target, seq, result: Err(e) });
                            }
                        } else {
                            i += 1;
                        }
                    }
                    // A settled view drawn again at the full sample count (none waits behind a newer job).
                    if let Some(job) = settle.take_due(Instant::now(), |t| !jobs.iter().any(|j| j.target == t) && renderer.can_submit(t)) {
                        let (target, seq) = (job.target, job.seq);
                        if let Err(e) = renderer.submit_frame(target, seq, job.request()) {
                            on_done(Completed::Frame { target, seq, result: Err(e) });
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
                        if let Completed::Frame { seq, .. } = &c {
                            perf::frame_milestone(*seq, "rendered");
                        }
                        let frame = matches!(c, Completed::Frame { .. });
                        on_done(c);
                        // Render-thread CPU per delivered frame (encoding,
                        // copy-out and hand-off; the GPU's work is not included).
                        if frame
                            && perf::enabled()
                            && let Some(now) = perf::thread_cpu_time()
                            && let Some(prev) = cpu_at_frame.replace(now)
                        {
                            perf::record("w.cpu_per_frame", now.saturating_sub(prev));
                        }
                    }
                    if !delivered && renderer.has_pending() {
                        let _t = perf::scope("w.gpu_wait");
                        if renderer.frame_slot_free() {
                            // The next frame can go to the GPU before this
                            // one is done: wait in short slices, the loop
                            // takes it as soon as it arrives (blocking for the
                            // whole frame held every frame back until the
                            // previous one had finished: one frame in flight).
                            renderer.wait_oldest_for(std::time::Duration::from_millis(1));
                        } else {
                            renderer.wait_oldest();
                        }
                    }
                }
            })
            .expect("spawn render thread");
        Self { tx, thread: Some(thread), closing, uploader, adapter, software, frames_in_flight }
    }

    pub fn submit(&self, job: RenderJob) {
        let _ = self.tx.send(Msg::Render(Box::new(job)));
    }

    /// The caller shows `job`'s frame from its own copy of one drawn before: picks answer for it from now on.
    pub fn adopt(&self, job: RenderJob) {
        let _ = self.tx.send(Msg::Adopt(Box::new(job)));
    }

    /// Read a rectangle (device pixels) of `target`'s pick buffer for its
    /// newest frame; the result is reported with `token`.
    pub fn pick(&self, target: u64, token: u64, rect: PickRect) {
        let _ = self.tx.send(Msg::Pick { target, token, rect });
    }

    /// How `target`'s frames are delivered (default [`Present::Readback`]).
    pub fn set_present(&self, target: u64, present: Present) {
        let _ = self.tx.send(Msg::SetPresent(target, present));
    }

    /// Draw `overlay` into every frame of `target` from now on.
    pub fn set_overlay(&self, target: u64, overlay: Option<Arc<crate::overlay::Overlay>>) {
        let _ = self.tx.send(Msg::SetOverlay(target, overlay));
    }

    /// Build `geometry` for every frame of `target` from the frame's camera.
    pub fn set_camera_geometry(&self, target: u64, geometry: Vec<Arc<dyn crate::camera_geometry::CameraGeometry>>) {
        let _ = self.tx.send(Msg::SetCameraGeometry(target, geometry));
    }

    /// How overlay text masks are shaded.
    pub fn set_text_shading(&self, shading: crate::overlay::TextShading) {
        let _ = self.tx.send(Msg::SetTextShading(shading));
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

    /// [`Renderer::is_software`].
    pub fn is_software(&self) -> bool {
        self.software
    }

    /// Frames a target may have in flight (submitting more only queues them).
    pub fn frames_in_flight(&self) -> usize {
        self.frames_in_flight
    }
}

impl Drop for RenderWorker {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::Relaxed);
        // Disconnect the thread's channel, then wait for it (never from the render thread itself).
        drop(std::mem::replace(&mut self.tx, mpsc::channel().0));
        if let Some(thread) = self.thread.take().filter(|t| t.thread().id() != std::thread::current().id()) {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(target: u64, seq: u64, msaa: u32) -> RenderJob {
        RenderJob {
            target,
            scene: Arc::new(Scene::default()),
            camera: Camera::default(),
            width: 8,
            height: 8,
            device_pixel_ratio: 1.0,
            style: Style { msaa, ..Style::default() },
            pick: true,
            seq,
        }
    }

    #[test]
    fn a_software_view_moves_at_1x_and_settles_at_its_samples() {
        let t0 = Instant::now();
        let mut s = Settle::default();
        assert_eq!(s.moving(job(1, 1, 4), t0).style.msaa, 1, "moving: 1x");
        assert_eq!(s.wait(t0), Some(SETTLE));
        // A newer frame before it settled: the settle waits again.
        assert_eq!(s.moving(job(1, 2, 4), t0 + SETTLE / 2).style.msaa, 1);
        assert!(s.take_due(t0 + SETTLE, |_| true).is_none(), "the view changed 75 ms ago");
        assert!(s.take_due(t0 + SETTLE * 2, |_| false).is_none(), "no free frame slot: it waits");
        let settled = s.take_due(t0 + SETTLE * 2, |_| true).expect("settled");
        assert_eq!((settled.seq, settled.style.msaa, settled.pick), (2, 4, false), "the newest view at 4x, its pick buffer kept");
        assert_eq!(s.wait(t0), None, "drawn once");
        // A 1x style and a dropped target settle nothing.
        assert_eq!(s.moving(job(2, 3, 1), t0).style.msaa, 1);
        s.moving(job(3, 4, 4), t0);
        s.forget(3);
        assert_eq!(s.wait(t0), None);
    }
}
