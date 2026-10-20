//! NOSHAPE PATCH (agent perf): textures the application renders itself
//! (noshape's 3D viewport), composited by the platform renderer straight from
//! GPU memory instead of being read back to the CPU and uploaded into the
//! sprite atlas every frame.
//!
//! The application registers a texture under an [`ExternalTextureId`]
//! ([`set_external_texture`]) and paints the id with
//! [`crate::Window::paint_external_texture`]. The painted sprite refers to the
//! id, not to the texture: the renderer looks the texture up when it draws, so
//! replacing the registered texture shows the new pixels on the next present,
//! also when a cached view replays its old paint or when the window presents
//! its last frame again without a redraw ([`crate::Window::request_present`]).
//!
//! The texture object is the platform renderer's own type (the wgpu renderer
//! expects a `wgpu::TextureView` of an 8-bit BGRA/RGBA texture on its device;
//! the Metal renderer an `IOSurface`), behind `Arc<dyn Any>`. Renderers that
//! don't support external textures draw nothing for them.
//!
//! A texture may only be reused for new pixels once no GPU frame that sampled
//! it is still running: [`gpu_frames_submitted`] and
//! [`gpu_frames_completed_through`] report the renderers' progress.

use crate::{AtlasTextureId, AtlasTextureKind, AtlasTile, Bounds, DevicePixels, Size, TileId, point};
use std::collections::{BTreeSet, HashMap};
use parking_lot::Mutex;
use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Set in [`AtlasTextureId::index`] for external textures; the other bits are
/// the [`ExternalTextureId`].
pub const EXTERNAL_TEXTURE_INDEX_BIT: u32 = 1 << 31;

/// Identifies an application texture across frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExternalTextureId(u32);

impl ExternalTextureId {
    /// A new, process-unique id.
    pub fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed) & !EXTERNAL_TEXTURE_INDEX_BIT)
    }

    /// The atlas texture id renderers see for this texture.
    pub fn atlas_texture_id(self) -> AtlasTextureId {
        AtlasTextureId { index: self.0 | EXTERNAL_TEXTURE_INDEX_BIT, kind: AtlasTextureKind::Polychrome }
    }

    /// The external texture an atlas texture id refers to, if it is one.
    pub fn from_atlas_texture_id(id: AtlasTextureId) -> Option<Self> {
        (id.index & EXTERNAL_TEXTURE_INDEX_BIT != 0).then_some(Self(id.index & !EXTERNAL_TEXTURE_INDEX_BIT))
    }
}

impl Default for ExternalTextureId {
    fn default() -> Self {
        Self::new()
    }
}

/// A registered texture.
#[derive(Clone)]
pub struct ExternalTexture {
    /// Size in device pixels; sprites sample the whole texture.
    pub size: Size<DevicePixels>,
    /// The renderer's texture object (see the module docs).
    pub texture: Arc<dyn Any + Send + Sync>,
    /// Changes whenever a different texture is registered under the id
    /// (renderers cache per-texture state by it).
    pub generation: u64,
}

fn registry() -> &'static Mutex<HashMap<ExternalTextureId, ExternalTexture>> {
    static REGISTRY: std::sync::OnceLock<Mutex<HashMap<ExternalTextureId, ExternalTexture>>> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}

/// Show `texture` (of `size` device pixels) wherever `id` is painted, or
/// nothing (`None`).
pub fn set_external_texture(id: ExternalTextureId, texture: Option<(Arc<dyn Any + Send + Sync>, Size<DevicePixels>)>) {
    static GENERATION: AtomicU64 = AtomicU64::new(1);
    let mut registry = registry().lock();
    match texture {
        Some((texture, size)) => {
            let generation = GENERATION.fetch_add(1, Ordering::Relaxed);
            registry.insert(id, ExternalTexture { size, texture, generation });
        }
        None => {
            registry.remove(&id);
        }
    }
}

/// The texture registered under `id`.
pub fn external_texture(id: ExternalTextureId) -> Option<ExternalTexture> {
    registry().lock().get(&id).cloned()
}

/// The texture an atlas texture id refers to, if it is a registered external
/// texture (for renderers).
pub fn external_texture_for_atlas_id(id: AtlasTextureId) -> Option<ExternalTexture> {
    external_texture(ExternalTextureId::from_atlas_texture_id(id)?)
}

/// The sprite tile covering the whole texture registered under `id`.
pub(crate) fn external_texture_tile(id: ExternalTextureId) -> Option<AtlasTile> {
    let size = registry().lock().get(&id)?.size;
    Some(AtlasTile { texture_id: id.atlas_texture_id(), tile_id: TileId(0), padding: 0, bounds: Bounds { origin: point(DevicePixels(0), DevicePixels(0)), size } })
}

struct GpuFrames {
    submitted: u64,
    running: BTreeSet<u64>,
}

static GPU_FRAMES: Mutex<GpuFrames> = Mutex::new(GpuFrames { submitted: 0, running: BTreeSet::new() });

/// Renderers: a window frame was submitted to the GPU; returns its number,
/// to be passed to [`gpu_frame_completed`] when the GPU finished it.
pub fn gpu_frame_submitted() -> u64 {
    let mut frames = GPU_FRAMES.lock();
    frames.submitted += 1;
    let n = frames.submitted;
    frames.running.insert(n);
    n
}

/// Renderers: the GPU finished frame `n` (see [`gpu_frame_submitted`]).
pub fn gpu_frame_completed(n: u64) {
    GPU_FRAMES.lock().running.remove(&n);
}

/// The number of window frames submitted to the GPU so far (all windows).
pub fn gpu_frames_submitted() -> u64 {
    GPU_FRAMES.lock().submitted
}

/// Whether every window frame up to number `n` has finished on the GPU, so
/// a texture they could sample may be written again.
pub fn gpu_frames_completed_through(n: u64) -> bool {
    GPU_FRAMES.lock().running.first().is_none_or(|&first| first > n)
}

static RENDERER_GPUS: Mutex<Vec<u64>> = Mutex::new(Vec::new());

/// Renderers: the GPU they draw with (Metal: the device's `registryID`).
/// An application texture must live on one of these GPUs.
pub fn note_renderer_gpu(id: u64) {
    let mut gpus = RENDERER_GPUS.lock();
    if !gpus.contains(&id) {
        gpus.push(id);
    }
}

/// The GPUs window renderers draw with (see [`note_renderer_gpu`]).
pub fn renderer_gpus() -> Vec<u64> {
    RENDERER_GPUS.lock().clone()
}

/// A retained native GPU object for [`set_external_texture`] (the Metal
/// renderer expects an `id<MTLTexture>`), released with `release` on drop.
/// Metal objects may be used from any thread, so the handle is `Send + Sync`.
pub struct NativeTextureHandle {
    ptr: std::ptr::NonNull<std::ffi::c_void>,
    release: unsafe fn(std::ptr::NonNull<std::ffi::c_void>),
}

// SAFETY: the object is only retained/released and handed to the renderer,
// which uses it on its own thread; the native APIs it is used with (Metal)
// are thread-safe.
unsafe impl Send for NativeTextureHandle {}
// SAFETY: see `Send`.
unsafe impl Sync for NativeTextureHandle {}

impl NativeTextureHandle {
    /// Take over one reference to the object at `ptr`.
    ///
    /// # Safety
    /// `ptr` must point to a live object the caller owns one reference to,
    /// and `release` must give that reference up.
    pub unsafe fn new(ptr: std::ptr::NonNull<std::ffi::c_void>, release: unsafe fn(std::ptr::NonNull<std::ffi::c_void>)) -> Self {
        Self { ptr, release }
    }

    /// The object (still owned by the handle).
    pub fn as_ptr(&self) -> std::ptr::NonNull<std::ffi::c_void> {
        self.ptr
    }
}

impl Drop for NativeTextureHandle {
    fn drop(&mut self) {
        // SAFETY: `new`'s contract.
        unsafe { (self.release)(self.ptr) }
    }
}
