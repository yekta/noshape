//! Process-wide GPU access: one render worker thread and device shared by all
//! viewports, so uploaded meshes can be reused across tabs and windows.

use gpui_kit::RenderImage;
use noshape_render::{Frame, worker::{RenderJob, RenderWorker}, renderer::Uploader};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub type FrameResult = (u64, Result<(Frame, Arc<RenderImage>), String>, f32);

struct Routes {
    senders: HashMap<u64, async_channel::Sender<FrameResult>>,
    submitted: HashMap<(u64, u64), Instant>,
}

#[derive(Clone)]
pub struct Gpu {
    worker: Arc<RenderWorker>,
    routes: Arc<Mutex<Routes>>,
    next_target: Arc<AtomicU64>,
}

impl Gpu {
    pub fn new() -> anyhow::Result<Self> {
        let routes = Arc::new(Mutex::new(Routes { senders: HashMap::new(), submitted: HashMap::new() }));
        let r2 = routes.clone();
        let worker = RenderWorker::spawn(move |target, seq, result| {
            let started = r2.lock().submitted.remove(&(target, seq));
            let ms = started.map(|t| t.elapsed().as_secs_f32() * 1000.0).unwrap_or(0.0);
            let result = result.map_err(|e| e.to_string()).and_then(|mut frame| {
                let bgra = std::mem::take(&mut frame.bgra);
                let buf = image::RgbaImage::from_raw(frame.width, frame.height, bgra)
                    .ok_or_else(|| "frame size mismatch".to_string())?;
                let image = Arc::new(RenderImage::new(vec![image::Frame::new(buf)]));
                Ok((frame, image))
            });
            let sender = r2.lock().senders.get(&target).cloned();
            if let Some(tx) = sender {
                let _ = tx.try_send((seq, result, ms));
            }
        })?;
        log::info!("GPU: {}", worker.adapter_name());
        Ok(Self { worker: Arc::new(worker), routes, next_target: Arc::new(AtomicU64::new(1)) })
    }

    /// Register a new render target; frames for it arrive on the receiver.
    pub fn register_target(&self) -> (u64, async_channel::Receiver<FrameResult>) {
        let id = self.next_target.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = async_channel::unbounded();
        self.routes.lock().senders.insert(id, tx);
        (id, rx)
    }

    pub fn unregister_target(&self, id: u64) {
        self.routes.lock().senders.remove(&id);
    }

    pub fn submit(&self, job: RenderJob) {
        self.routes.lock().submitted.insert((job.target, job.seq), Instant::now());
        self.worker.submit(job);
    }

    pub fn uploader(&self) -> &Uploader {
        self.worker.uploader()
    }

    pub fn adapter_name(&self) -> &str {
        self.worker.adapter_name()
    }
}

/// Global handle so any window can open new document windows.
pub struct GpuGlobal(pub Gpu);
impl gpui_kit::Global for GpuGlobal {}
