//! Process-wide GPU access: one render worker thread and device shared by all
//! viewports, so uploaded meshes can be reused across tabs and windows.
//!
//! Where GPUI renders with wgpu on a real GPU API (Linux: Vulkan), the
//! renderer runs on GPUI's own device and 3D viewport frames stay on the GPU
//! ([`FrameImage::Texture`]): GPUI composites them directly
//! (`Window::paint_external_texture`, a vendored GPUI patch), with no readback
//! and no upload. Elsewhere (or with `NOSHAPE_PRESENT=readback`) frames are
//! read back and shown as images.

use gpui_kit::RenderImage;
use noshape_render::{Camera, Completed, Frame, PickBuffer, PickRect, Present, PresentTexture, Scene, Style, worker::{RenderJob, RenderWorker}, renderer::Uploader};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub type FrameResult = (u64, Result<(Frame, Arc<RenderImage>), String>, f32);
/// A 3D viewport's frame ([`Gpu::register_target_with_picks`]).
pub type ViewportFrameResult = (u64, Result<(Frame, FrameImage), String>, f32);

/// How a viewport frame is shown.
pub enum FrameImage {
    /// Read back to the CPU; GPUI uploads the image into its atlas.
    Image(Arc<RenderImage>),
    /// Left on the GPU; GPUI samples the texture (see the module docs).
    Texture(Arc<PresentTexture>),
}
/// Answer to [`Gpu::pick`]: (token, pick ids of the requested rectangle).
pub type PickResult = (u64, Result<Arc<PickBuffer>, String>);

struct Routes {
    senders: HashMap<u64, async_channel::Sender<FrameResult>>,
    viewport_senders: HashMap<u64, async_channel::Sender<ViewportFrameResult>>,
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
    worker: Arc<OnceLock<Result<(RenderWorker, Present), String>>>,
    routes: Arc<Mutex<Routes>>,
    next_target: Arc<AtomicU64>,
}

impl Gpu {
    pub fn new() -> anyhow::Result<Self> {
        let routes = Arc::new(Mutex::new(Routes { senders: HashMap::new(), viewport_senders: HashMap::new(), pick_senders: HashMap::new(), submitted: HashMap::new(), oneshots: HashMap::new() }));
        let r2 = routes.clone();
        let cell: Arc<OnceLock<Result<(RenderWorker, Present), String>>> = Arc::new(OnceLock::new());
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
            let viewport = r2.lock().viewport_senders.get(&target).cloned();
            if let Some(tx) = viewport {
                let result = result.map_err(|e| e.to_string()).and_then(|mut frame| {
                    let image = match frame.texture.take() {
                        Some(texture) => FrameImage::Texture(texture),
                        None => FrameImage::Image(frame_image(&mut frame)?),
                    };
                    Ok((frame, image))
                });
                let _ = tx.try_send((seq, result, ms));
                return;
            }
            let result = result.map_err(|e| e.to_string()).and_then(|mut frame| {
                let image = frame_image(&mut frame)?;
                Ok((frame, image))
            });
            let sender = r2.lock().senders.get(&target).cloned();
            if let Some(tx) = sender {
                let _ = tx.try_send((seq, result, ms));
            }
        };
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        gpui_wgpu::request_device_features(noshape_render::Renderer::WANTED_FEATURES);
        if noshape_render::perf::enabled() {
            // GPUI's draw phases (render + layout requests, prepaint, paint, present).
            gpui_kit::set_draw_stats_hook(noshape_render::perf::record);
        }
        std::thread::Builder::new().name("noshape-gpu-init".into()).spawn(move || {
            let worker = spawn_worker(on_done);
            match &worker {
                Ok((w, present)) => log::info!("GPU: {} (viewport frames: {present:?})", w.adapter_name()),
                Err(e) => log::error!("GPU initialization failed: {e}"),
            }
            noshape_render::perf::milestone("GPU initialized");
            let _ = cell2.set(worker);
        })?;
        Ok(Self { worker: cell, routes, next_target: Arc::new(AtomicU64::new(1)) })
    }

    /// The GPU adapter ("name (Backend)") once the renderer is up.
    pub fn adapter_name(&self) -> Option<String> {
        self.worker.get().and_then(|w| w.as_ref().ok()).map(|w| w.0.adapter_name().to_string())
    }

    /// The render worker, waiting for its initialization if needed.
    fn worker(&self) -> &RenderWorker {
        match self.worker.wait() {
            Ok((w, _)) => w,
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

    /// Register a 3D viewport's target: it also answers pick queries
    /// ([`Gpu::pick`]), and its frames stay on the GPU where GPUI can
    /// composite them ([`FrameImage::Texture`]).
    pub fn register_target_with_picks(&self) -> (u64, async_channel::Receiver<ViewportFrameResult>, async_channel::Receiver<PickResult>) {
        let id = self.next_target.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = async_channel::unbounded();
        let (ptx, prx) = async_channel::unbounded();
        {
            let mut routes = self.routes.lock();
            routes.viewport_senders.insert(id, tx);
            routes.pick_senders.insert(id, ptx);
        }
        if let Ok((worker, Present::Texture)) = self.worker.wait()
            && compositor_shares_gpu()
        {
            worker.set_present(id, Present::Texture);
        }
        (id, rx, prx)
    }

    /// What GPUI's renderer samples for a [`FrameImage::Texture`] frame
    /// (`gpui::set_external_texture`): the texture's view on GPUI's device.
    pub fn compositor_texture(texture: &PresentTexture) -> Arc<dyn std::any::Any + Send + Sync> {
        #[cfg(target_os = "macos")]
        {
            metal::texture(texture)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Arc::new(texture.view.clone())
        }
    }

    /// Draw `overlay` (view cube, plane names) into the target's frames.
    pub fn set_overlay(&self, target: u64, overlay: Option<Arc<noshape_render::overlay::Overlay>>) {
        self.worker().set_overlay(target, overlay);
    }

    /// Geometry the render thread builds for every frame from its camera.
    pub fn set_camera_geometry(&self, target: u64, geometry: Vec<Arc<dyn noshape_render::camera_geometry::CameraGeometry>>) {
        self.worker().set_camera_geometry(target, geometry);
    }

    /// Shade overlay text like GPUI's text (its renderer's parameters).
    pub fn set_text_shading(&self, shading: noshape_render::overlay::TextShading) {
        self.worker().set_text_shading(shading);
    }

    pub fn unregister_target(&self, id: u64) {
        let mut r = self.routes.lock();
        r.senders.remove(&id);
        r.viewport_senders.remove(&id);
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

/// A read-back frame as a GPUI image (the pixels move, no copy).
fn frame_image(frame: &mut Frame) -> Result<Arc<RenderImage>, String> {
    let _t = noshape_render::perf::scope("g.image");
    let bgra = std::mem::take(&mut frame.bgra);
    let buf = image::RgbaImage::from_raw(frame.width, frame.height, bgra).ok_or_else(|| "frame size mismatch".to_string())?;
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(buf)])))
}

/// Start the render thread: on GPUI's own device with frames left on the
/// GPU where GPUI can composite them, else on a device of its own with
/// frames read back (see the module docs).
fn spawn_worker(on_done: impl Fn(Completed) + Send + 'static) -> Result<(RenderWorker, Present), String> {
    // `NOSHAPE_PRESENT=readback|texture` overrides the choice (benchmarks).
    let forced = std::env::var("NOSHAPE_PRESENT").ok();
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    if forced.as_deref() != Some("readback") {
        // GPUI creates its context with the first window, right after start.
        // On a CPU rasterizer (llvmpipe) sharing GPUI's queue serializes our
        // frames behind GPUI's own software rendering, so frames are read
        // back there unless texture presentation is forced.
        match gpui_wgpu::wait_for_shared_context(std::time::Duration::from_secs(20)) {
            Some(ctx)
                if ctx.adapter.get_info().backend == gpui_wgpu::wgpu::Backend::Vulkan
                    && (ctx.adapter.get_info().device_type != gpui_wgpu::wgpu::DeviceType::Cpu || forced.as_deref() == Some("texture")) =>
            {
                let renderer = noshape_render::Renderer::with_device(&ctx.adapter, (*ctx.device).clone(), (*ctx.queue).clone());
                return Ok((RenderWorker::spawn_with(renderer, on_done), Present::Texture));
            }
            Some(ctx) => log::info!("GPUI renders with {:?} on a {:?} adapter: viewport frames are read back", ctx.adapter.get_info().backend, ctx.adapter.get_info().device_type),
            None => log::warn!("GPUI's GPU context did not appear: viewport frames are read back"),
        }
    }
    #[cfg(target_os = "macos")]
    if forced.as_deref() != Some("readback") {
        // GPUI (Metal) samples the renderer's textures directly when both
        // draw with the same GPU (`compositor_shares_gpu`). Apple silicon has
        // one; on Intel Macs with two, GPUI draws with the integrated one, so
        // render on the GPU GPUI reports when its first window opens.
        // With one GPU that is GPUI's: no need to wait for its first window
        // (the renderer and its shaders get going while the window opens).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let gpui_gpu = metal::only_device().or_else(|| loop {
            match gpui_kit::renderer_gpus().first() {
                Some(id) => break Some(*id),
                None if std::time::Instant::now() >= deadline => break None,
                None => std::thread::sleep(std::time::Duration::from_millis(5)),
            }
        });
        noshape_render::perf::milestone("GPU: GPUI's window GPU known");
        let renderer = match gpui_gpu.and_then(metal::device_name) {
            Some(name) => noshape_render::Renderer::new_matching(|info| info.backend == noshape_render::wgpu::Backend::Metal && info.name == name),
            None => noshape_render::Renderer::new(),
        }
        .map_err(|e| e.to_string())?;
        if let Some(id) = metal::registry_id(renderer.device()) {
            let _ = metal::RENDERER_GPU.set(id);
            return Ok((RenderWorker::spawn_with(renderer, on_done), Present::Texture));
        }
        return Ok((RenderWorker::spawn_with(renderer, on_done), Present::Readback));
    }
    let _ = forced;
    RenderWorker::spawn(on_done).map(|w| (w, Present::Readback)).map_err(|e| e.to_string())
}

/// Whether GPUI's window renderer draws with the GPU the viewport renders on,
/// so it can sample the viewport's textures.
fn compositor_shares_gpu() -> bool {
    #[cfg(target_os = "macos")]
    {
        metal::RENDERER_GPU.get().is_some_and(|id| gpui_kit::renderer_gpus().contains(id))
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Metal access for zero-copy presentation (macOS).
#[cfg(target_os = "macos")]
mod metal {
    use noshape_render::{PresentTexture, wgpu};
    use objc2::Message as _;
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2_metal::{MTLDevice, MTLTexture};
    use std::ffi::c_void;
    use std::ptr::NonNull;
    use std::sync::{Arc, OnceLock};

    /// `registryID` of the renderer's Metal device.
    pub static RENDERER_GPU: OnceLock<u64> = OnceLock::new();

    /// The name of the Metal device with `registryID` `id` (what wgpu calls
    /// its adapter).
    /// The Mac's GPU when it has just one (every Apple silicon Mac).
    pub fn only_device() -> Option<u64> {
        let all = objc2_metal::MTLCopyAllDevices();
        let mut devices = all.iter();
        match (devices.next(), devices.next()) {
            (Some(d), None) => Some(d.registryID()),
            _ => None,
        }
    }

    pub fn device_name(id: u64) -> Option<String> {
        objc2_metal::MTLCopyAllDevices().iter().find(|d| d.registryID() == id).map(|d| d.name().to_string())
    }

    pub fn registry_id(device: &wgpu::Device) -> Option<u64> {
        // SAFETY: only reads the device's identity.
        let hal = unsafe { device.as_hal::<wgpu::hal::api::Metal>() }?;
        Some(hal.raw_device().registryID())
    }

    /// The frame's `MTLTexture`, what gpui's Metal renderer samples.
    pub fn texture(texture: &PresentTexture) -> Arc<dyn std::any::Any + Send + Sync> {
        // SAFETY: the raw texture is only retained, not used here.
        let hal = unsafe { texture.texture.as_hal::<wgpu::hal::api::Metal>() }.expect("a Metal texture");
        let raw: Retained<ProtocolObject<dyn MTLTexture>> = hal.raw_handle().retain();
        unsafe fn release(ptr: NonNull<c_void>) {
            // SAFETY: `ptr` came from `Retained::into_raw` below.
            drop(unsafe { Retained::from_raw(ptr.as_ptr().cast::<ProtocolObject<dyn MTLTexture>>()) });
        }
        let ptr = NonNull::new(Retained::into_raw(raw).cast::<c_void>()).expect("a retained texture");
        // SAFETY: the handle takes over the reference `into_raw` gave up.
        Arc::new(unsafe { gpui_kit::NativeTextureHandle::new(ptr, release) })
    }
}

/// Global handle so any window can open new document windows.
pub struct GpuGlobal(pub Gpu);
impl gpui_kit::Global for GpuGlobal {}
