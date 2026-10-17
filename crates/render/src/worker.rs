//! Background render thread: the UI thread submits jobs and receives frames
//! without ever blocking on the GPU. Pending jobs are coalesced so only the
//! newest one renders.

use crate::camera::Camera;
use crate::renderer::{Frame, RenderError, RenderParams, Renderer, Uploader};
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
    pub pick: bool,
    /// Monotonic sequence number echoed back with the frame.
    pub seq: u64,
}

pub struct RenderWorker {
    tx: mpsc::Sender<RenderJob>,
    uploader: Uploader,
    adapter: String,
}

impl RenderWorker {
    /// Spawns the render thread. `on_frame` is called on the render thread for
    /// every completed frame.
    pub fn spawn(
        on_frame: impl Fn(u64, u64, Result<Frame, RenderError>) + Send + 'static,
    ) -> Result<Self, RenderError> {
        let mut renderer = Renderer::new()?;
        let uploader = renderer.uploader();
        let adapter = renderer.adapter_name();
        let (tx, rx) = mpsc::channel::<RenderJob>();
        std::thread::Builder::new()
            .name("noshape-render".into())
            .spawn(move || {
                while let Ok(first) = rx.recv() {
                    // Coalesce: keep only the newest pending job per target, but
                    // never drop a pick request.
                    let mut pending: Vec<RenderJob> = vec![first];
                    while let Ok(next) = rx.try_recv() {
                        if let Some(slot) = pending.iter_mut().find(|j| j.target == next.target) {
                            let pick = slot.pick || next.pick;
                            *slot = next;
                            slot.pick = pick;
                        } else {
                            pending.push(next);
                        }
                    }
                    for job in pending {
                        let result = renderer.render(RenderParams {
                            scene: &job.scene,
                            camera: &job.camera,
                            width: job.width,
                            height: job.height,
                            device_pixel_ratio: job.device_pixel_ratio,
                            style: &job.style,
                            pick: job.pick,
                        });
                        on_frame(job.target, job.seq, result);
                    }
                }
            })
            .expect("spawn render thread");
        Ok(Self { tx, uploader, adapter })
    }

    pub fn submit(&self, job: RenderJob) {
        let _ = self.tx.send(job);
    }

    pub fn uploader(&self) -> &Uploader {
        &self.uploader
    }

    pub fn adapter_name(&self) -> &str {
        &self.adapter
    }
}
