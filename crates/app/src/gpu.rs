//! Process-wide GPU access: one render worker thread and device shared by all
//! viewports, so uploaded meshes can be reused across tabs and windows.

use gpui_kit::RenderImage;
use noshape_render::{Camera, Frame, Scene, Style, worker::{RenderJob, RenderWorker}, renderer::Uploader};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub type FrameResult = (u64, Result<(Frame, Arc<RenderImage>), String>, f32);

struct Routes {
    senders: HashMap<u64, async_channel::Sender<FrameResult>>,
    submitted: HashMap<(u64, u64), Instant>,
    /// One-off offscreen renders (thumbnails): raw frame back to the caller.
    oneshots: HashMap<u64, async_channel::Sender<Result<Frame, String>>>,
}

/// Size of document thumbnails (stored as PNG in the document metadata).
pub const THUMBNAIL_SIZE: (u32, u32) = (256, 192);

#[derive(Clone)]
pub struct Gpu {
    worker: Arc<RenderWorker>,
    routes: Arc<Mutex<Routes>>,
    next_target: Arc<AtomicU64>,
}

impl Gpu {
    pub fn new() -> anyhow::Result<Self> {
        let routes = Arc::new(Mutex::new(Routes { senders: HashMap::new(), submitted: HashMap::new(), oneshots: HashMap::new() }));
        let r2 = routes.clone();
        let worker = RenderWorker::spawn(move |target, seq, result| {
            let oneshot = r2.lock().oneshots.remove(&target);
            if let Some(tx) = oneshot {
                let _ = tx.try_send(result.map_err(|e| e.to_string()));
                return;
            }
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

    /// Render `scene` offscreen (independent of any viewport) and return the
    /// frame as PNG bytes of `width` x `height` pixels. Renders at twice the
    /// size and downsamples for smooth edges. Does not block the caller's
    /// thread on the GPU, but PNG encoding runs where the future is polled,
    /// so await it on a background executor.
    pub async fn render_png(&self, scene: Arc<Scene>, camera: Camera, style: Style, width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
        const SUPERSAMPLE: u32 = 2;
        let id = self.next_target.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = async_channel::bounded(1);
        self.routes.lock().oneshots.insert(id, tx);
        self.worker.submit(RenderJob {
            target: id,
            scene,
            camera,
            width: width * SUPERSAMPLE,
            height: height * SUPERSAMPLE,
            device_pixel_ratio: SUPERSAMPLE as f32,
            style,
            pick: false,
            seq: 0,
        });
        let frame = rx.recv().await.map_err(|_| anyhow::anyhow!("render worker stopped"))?.map_err(|e| anyhow::anyhow!(e))?;
        let mut px = frame.bgra;
        for c in px.chunks_exact_mut(4) {
            c.swap(0, 2);
            c[3] = 255;
        }
        let full = image::RgbaImage::from_raw(frame.width, frame.height, px).ok_or_else(|| anyhow::anyhow!("frame size mismatch"))?;
        let img = if frame.width == width && frame.height == height {
            full
        } else {
            image::imageops::resize(&full, width, height, image::imageops::FilterType::Triangle)
        };
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)?;
        Ok(png)
    }

    /// Render a document thumbnail ([`THUMBNAIL_SIZE`]) as PNG bytes.
    pub async fn render_thumbnail(&self, scene: Arc<Scene>, camera: Camera, style: Style) -> anyhow::Result<Vec<u8>> {
        self.render_png(scene, camera, style, THUMBNAIL_SIZE.0, THUMBNAIL_SIZE.1).await
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
