//! Process-wide GPU access: one render worker thread and device shared by all
//! viewports, so uploaded meshes can be reused across tabs and windows.

use gpui_kit::RenderImage;
use noshape_render::{Camera, Completed, Frame, PickBuffer, PickRect, Scene, Style, worker::{RenderJob, RenderWorker}, renderer::Uploader};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub type FrameResult = (u64, Result<(Frame, Arc<RenderImage>), String>, f32);
/// Answer to [`Gpu::pick`]: (token, pick ids of the requested rectangle).
pub type PickResult = (u64, Result<Arc<PickBuffer>, String>);

struct Routes {
    senders: HashMap<u64, async_channel::Sender<FrameResult>>,
    pick_senders: HashMap<u64, async_channel::Sender<PickResult>>,
    /// Submission times per target, oldest first (coalesced jobs never
    /// complete, so a frame retires every older entry too).
    submitted: HashMap<u64, VecDeque<(u64, Instant)>>,
    /// One-off offscreen renders (thumbnails): raw frame back to the caller.
    oneshots: HashMap<u64, async_channel::Sender<Result<Frame, String>>>,
}

/// Size of document thumbnails (stored as PNG in the document metadata).
pub const THUMBNAIL_SIZE: (u32, u32) = (256, 192);

#[derive(Clone)]
pub struct Gpu {
    /// Created on a background thread at startup (adapter/device creation and
    /// pipeline compilation take 0.1-2 s) while GPUI opens the window; the
    /// first use waits for it.
    worker: Arc<OnceLock<Result<RenderWorker, String>>>,
    routes: Arc<Mutex<Routes>>,
    next_target: Arc<AtomicU64>,
}

impl Gpu {
    pub fn new() -> anyhow::Result<Self> {
        let routes = Arc::new(Mutex::new(Routes { senders: HashMap::new(), pick_senders: HashMap::new(), submitted: HashMap::new(), oneshots: HashMap::new() }));
        let r2 = routes.clone();
        let cell: Arc<OnceLock<Result<RenderWorker, String>>> = Arc::new(OnceLock::new());
        let cell2 = cell.clone();
        let on_done = move |done: Completed| {
            let (target, seq, result) = match done {
                Completed::Frame { target, seq, result } => (target, seq, result),
                Completed::Pick { target, token, result } => {
                    let sender = r2.lock().pick_senders.get(&target).cloned();
                    if let Some(tx) = sender {
                        let _ = tx.try_send((token, result.map(Arc::new).map_err(|e| e.to_string())));
                    }
                    return;
                }
            };
            let oneshot = r2.lock().oneshots.remove(&target);
            if let Some(tx) = oneshot {
                let _ = tx.try_send(result.map_err(|e| e.to_string()));
                return;
            }
            let started = {
                let mut routes = r2.lock();
                let q = routes.submitted.entry(target).or_default();
                let mut started = None;
                while q.front().is_some_and(|(s, _)| *s <= seq) {
                    let (s, t) = q.pop_front().expect("front");
                    if s == seq {
                        started = Some(t);
                    }
                }
                started
            };
            let ms = started.map(|t| t.elapsed().as_secs_f32() * 1000.0).unwrap_or(0.0);
            noshape_render::perf::record_ms("g.submit_to_frame", ms);
            let _t_img = noshape_render::perf::scope("g.image");
            let result = result.map_err(|e| e.to_string()).and_then(|mut frame| {
                let bgra = std::mem::take(&mut frame.bgra);
                let buf = image::RgbaImage::from_raw(frame.width, frame.height, bgra)
                    .ok_or_else(|| "frame size mismatch".to_string())?;
                let image = Arc::new(RenderImage::new(vec![image::Frame::new(buf)]));
                Ok((frame, image))
            });
            drop(_t_img);
            let sender = r2.lock().senders.get(&target).cloned();
            if let Some(tx) = sender {
                let _ = tx.try_send((seq, result, ms));
            }
        };
        std::thread::Builder::new().name("noshape-gpu-init".into()).spawn(move || {
            let worker = RenderWorker::spawn(on_done).map_err(|e| e.to_string());
            match &worker {
                Ok(w) => log::info!("GPU: {}", w.adapter_name()),
                Err(e) => log::error!("GPU initialization failed: {e}"),
            }
            noshape_render::perf::milestone("GPU initialized");
            let _ = cell2.set(worker);
        })?;
        Ok(Self { worker: cell, routes, next_target: Arc::new(AtomicU64::new(1)) })
    }

    /// The render worker, waiting for its initialization if needed.
    fn worker(&self) -> &RenderWorker {
        match self.worker.wait() {
            Ok(w) => w,
            Err(e) => panic!("noshape needs a GPU (Vulkan, Metal or DirectX 12): {e}"),
        }
    }

    /// Register a new render target; frames for it arrive on the receiver.
    pub fn register_target(&self) -> (u64, async_channel::Receiver<FrameResult>) {
        let id = self.next_target.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = async_channel::unbounded();
        self.routes.lock().senders.insert(id, tx);
        (id, rx)
    }

    /// Register a render target that also answers pick queries ([`Gpu::pick`]).
    pub fn register_target_with_picks(&self) -> (u64, async_channel::Receiver<FrameResult>, async_channel::Receiver<PickResult>) {
        let (id, rx) = self.register_target();
        let (tx, prx) = async_channel::unbounded();
        self.routes.lock().pick_senders.insert(id, tx);
        (id, rx, prx)
    }

    pub fn unregister_target(&self, id: u64) {
        let mut r = self.routes.lock();
        r.senders.remove(&id);
        r.pick_senders.remove(&id);
        r.submitted.remove(&id);
        drop(r);
        self.worker().drop_target(id);
    }

    pub fn submit(&self, job: RenderJob) {
        self.routes.lock().submitted.entry(job.target).or_default().push_back((job.seq, Instant::now()));
        self.worker().submit(job);
    }

    /// Read a rectangle (device pixels) of the target's pick buffer for its
    /// newest frame; the answer arrives on the target's pick channel.
    pub fn pick(&self, target: u64, token: u64, rect: PickRect) {
        self.worker().pick(target, token, rect);
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
        self.worker().submit(RenderJob {
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
        let frame = rx.recv().await.map_err(|_| anyhow::anyhow!("render worker stopped"));
        self.worker().drop_target(id);
        let frame = frame?.map_err(|e| anyhow::anyhow!(e))?;
        let mut px = frame.bgra;
        for c in px.as_chunks_mut::<4>().0 {
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
        self.worker().uploader()
    }

    /// Frames a viewport should keep in flight.
    pub fn frames_in_flight(&self) -> usize {
        self.worker().frames_in_flight()
    }
}

/// Global handle so any window can open new document windows.
pub struct GpuGlobal(pub Gpu);
impl gpui_kit::Global for GpuGlobal {}
