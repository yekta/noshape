//! Offscreen wgpu renderer producing BGRA frames (for GPUI) and pick-id regions.
//!
//! Rendering is organised per *target* (one per viewport): every target owns
//! its render textures and a few readback buffers, so the render thread can
//! submit frame N+1 while frame N is still being copied back
//! ([`Renderer::submit_frame`], [`Renderer::collect`]; driven by `worker.rs`).
//! The pick-id buffer is rendered lazily, only when a pick query arrives after
//! the geometry or camera changed, and only the queried rectangles are copied
//! back ([`Renderer::submit_picks`]). [`Renderer::render`] is a synchronous
//! convenience wrapper (tests, tools).
//!
//! A target's color frames reach the screen in one of two ways
//! ([`Present`]): read back to the CPU as BGRA bytes (thumbnails, tests,
//! drawings), or left on the GPU in a small ring of textures
//! ([`PresentTexture`]) that the UI composites directly when it shares this
//! renderer's GPU device ([`Renderer::with_device`]): no readback, no copy.

use crate::camera::{Camera, Projection};
use crate::perf;
use crate::scene::*;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use wgpu::util::DeviceExt;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
const PICK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Uint;
/// Depth of the pick passes: no stencil (they use none), so the small
/// visible-depth sampling pass ([`PickRect::depth_samples`]) can copy it out.
const PICK_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Depth (and stencil, for section caps) format: float depth with an 8-bit
/// stencil where the adapter has it, else 24-bit depth + stencil. Chosen
/// once, when the first device is created.
static DEPTH_FORMAT_CHOICE: std::sync::OnceLock<wgpu::TextureFormat> = std::sync::OnceLock::new();

fn depth_format() -> wgpu::TextureFormat {
    *DEPTH_FORMAT_CHOICE.get().unwrap_or(&wgpu::TextureFormat::Depth32FloatStencil8)
}
const INST_STRIDE: u64 = 256;

/// Polygon offset of shaded faces (reverse-Z: negative pushes away from the
/// camera). The slope term pushes a face back by the depth it gains over a
/// few pixels, so edges (screen-space quads a couple of pixels wide, drawn
/// at the edge's depth) stay fully visible on silhouettes and where the
/// adjacent face rises toward the viewer, instead of fighting the face and
/// looking dotted. The constant term covers faces seen head-on.
const FACE_DEPTH_BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState { constant: -16, slope_scale: -2.0, clamp: 0.0 };
/// Pick-only meshes win ties against faces at the same depth (a sketch region
/// on a part face): a small pull toward the camera on top of the faces' push.
const PICK_PRIORITY_DEPTH_BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState { constant: 4, slope_scale: 0.0, clamp: 0.0 };

/// Upper bound of color frames one target may have in flight (rendering or
/// being read back); see [`Renderer::frames_in_flight`].
pub const MAX_FRAMES_IN_FLIGHT: usize = 2;
/// Upper bound of a [`Present::Texture`] target's textures: frames in flight,
/// the one on screen, and the ones the compositor may still be sampling.
const MAX_PRESENT_TEXTURES: usize = 6;
/// Format of [`PresentTexture`]s: plain BGRA8, so a compositor sampling it
/// gets the sRGB-encoded bytes unchanged (as from a readback). Rendering goes
/// through a [`COLOR_FORMAT`] view of it.
pub const PRESENT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
/// Targets kept alive before the least recently used one is evicted.
const MAX_TARGETS: usize = 8;
/// Target id used by the synchronous [`Renderer::render`].
const SYNC_TARGET: u64 = u64::MAX;

const MAP_PENDING: u8 = 0;
const MAP_OK: u8 = 1;
const MAP_FAILED: u8 = 2;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("no suitable GPU adapter: {0}")]
    NoAdapter(String),
    #[error("failed to create GPU device: {0}")]
    Device(String),
    #[error("GPU readback failed: {0}")]
    Readback(String),
}

pub(crate) struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    pub(crate) bounds: Option<(Vec3, Vec3)>,
}
pub(crate) struct GpuLines {
    buffer: wgpu::Buffer,
    count: u32,
    pub(crate) bounds: Option<(Vec3, Vec3)>,
}
pub(crate) struct GpuPoints {
    buffer: wgpu::Buffer,
    count: u32,
    pub(crate) bounds: Option<(Vec3, Vec3)>,
}
pub(crate) struct GpuImage {
    /// Six [`ImageVertex`]es (two triangles).
    vertices: wgpu::Buffer,
    /// Texture view and sampler (group 2 of the image pipelines); keeps the
    /// texture alive.
    bind_group: wgpu::BindGroup,
    pub(crate) bounds: Option<(Vec3, Vec3)>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ImageVertex {
    pos: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MeshVertex {
    pos: [f32; 3],
    normal: [f32; 3],
    pick: u32,
    /// sRGB RGBA8 per-vertex color (used by per-primitive color instances;
    /// alpha 0 = instance color).
    color: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    camera_fwd: [f32; 4],
    camera_up: [f32; 4],
    viewport: [f32; 4],
    clip: [f32; 4],
    flags: [u32; 4],
    clip2: [f32; 4],
    clip3: [f32; 4],
    bg_top: [f32; 4],
    bg_bottom: [f32; 4],
    hover: [f32; 4],
    select: [f32; 4],
    accent: [f32; 4],
    error: [f32; 4],
    /// rgb = specular color (linear), a = shininess 0..1.
    specular: [f32; 4],
    /// Construction line dash, gap, dash, gap (px).
    dash: [f32; 4],
    /// Zebra stripes room: center xyz and radius (around the scene).
    room: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstUniform {
    model: [[f32; 4]; 4],
    normal_m: [[f32; 4]; 4],
    color: [f32; 4],
    params: [u32; 4],
    /// x = line width scale.
    extra: [f32; 4],
    _pad: [f32; 20],
}

/// How a target's color frames are delivered (see the module docs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Present {
    /// Read back to the CPU ([`Frame::bgra`]).
    #[default]
    Readback,
    /// Left on the GPU ([`Frame::texture`]).
    Texture,
}

/// A color frame left on the GPU ([`Present::Texture`]): [`PRESENT_FORMAT`],
/// sRGB-encoded. The renderer renders into it again only once every clone of
/// the `Arc` holding it is dropped, so whoever shows it keeps it until the
/// compositor's GPU work sampling it has finished.
pub struct PresentTexture {
    pub texture: wgpu::Texture,
    /// The whole texture as [`PRESENT_FORMAT`] (what a compositor samples).
    pub view: wgpu::TextureView,
    /// The render attachment ([`COLOR_FORMAT`] view).
    target_view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

/// A rendered frame. `bgra` is tightly packed sRGB BGRA8, row-major, top row
/// first; empty when the frame stayed on the GPU (`texture`).
#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    /// The frame's texture ([`Present::Texture`] targets).
    pub texture: Option<Arc<PresentTexture>>,
    /// Pick ids of the whole frame (only from the synchronous [`Renderer::render`]
    /// with `pick: true`; the render worker answers pick queries separately).
    pub pick: Option<Arc<PickBuffer>>,
    /// When the frame finished on the render thread (for latency stats).
    pub rendered_at: std::time::Instant,
}

/// Pick ids of a rectangle of a frame (possibly the whole frame).
#[derive(Clone, Debug)]
pub struct PickBuffer {
    /// Origin of the rectangle in frame pixels.
    pub x0: u32,
    pub y0: u32,
    /// Size of the rectangle.
    pub width: u32,
    pub height: u32,
    /// Size of the frame the rectangle was read from.
    pub frame_width: u32,
    pub frame_height: u32,
    /// Row-major ids of the rectangle (`width * height`).
    pub ids: Vec<u32>,
    /// Visible-depth samples, for a [`PickRect::depth_samples`] query (then
    /// `ids` is empty and the frame size is the sampling viewport's): every
    /// sample that hit geometry.
    pub samples: Vec<DepthSample>,
    /// The same with the datum planes' depth (Onshape's second,
    /// `includePlanes` pass, used when `samples` has too few hits).
    pub samples_with_planes: Vec<DepthSample>,
    /// View-projection the samples were rendered with, to turn them back
    /// into world points.
    pub view_proj: Option<Mat4>,
}

/// One visible-depth sample: pixel position in the sampling viewport
/// ([`PickBuffer::frame_width`] x `frame_height`; x right, y down, pixel
/// centres at .5), the depth there (reverse-Z: 1 near, 0 far) and the pick id.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthSample {
    pub x: f32,
    pub y: f32,
    pub depth: f32,
    pub id: u32,
}

/// Size of the shrunk viewport Onshape's depth sampling pass renders for
/// about `n` samples of a `width` x `height` frame (`DepthSamplingPass.preDraw`).
pub fn depth_sample_size(width: u32, height: u32, n: u32) -> (u32, u32) {
    let (w, h) = (width.max(1) as f64, height.max(1) as f64);
    let s = (n as f64 / (w * h)).sqrt().min(1.0);
    let (hs, vs) = ((w * s).ceil() / w, (h * s).ceil() / h);
    (((hs * w).round() as u32).max(1), ((vs * h).round() as u32).max(1))
}

impl PickBuffer {
    /// Id at a frame pixel, `None` outside the rectangle.
    pub fn id_at(&self, x: i32, y: i32) -> Option<u32> {
        let (lx, ly) = (x - self.x0 as i32, y - self.y0 as i32);
        if lx < 0 || ly < 0 || lx >= self.width as i32 || ly >= self.height as i32 {
            return None;
        }
        Some(self.ids[(ly as u32 * self.width + lx as u32) as usize])
    }

    /// Whether the square of `radius` around a frame pixel (clipped to the
    /// frame) lies inside the rectangle, i.e. [`PickBuffer::pick`] is exact.
    pub fn covers(&self, x: i32, y: i32, radius: i32) -> bool {
        let x0 = (x - radius).max(0);
        let y0 = (y - radius).max(0);
        let x1 = (x + radius).min(self.frame_width as i32 - 1);
        let y1 = (y + radius).min(self.frame_height as i32 - 1);
        if x0 > x1 || y0 > y1 {
            // Entirely outside the frame: nothing to pick, trivially exact.
            return true;
        }
        x0 >= self.x0 as i32 && y0 >= self.y0 as i32 && x1 < (self.x0 + self.width) as i32 && y1 < (self.y0 + self.height) as i32
    }

    /// Pick id at pixel, searching a small radius for the nearest non-zero id
    /// so thin edges and points are easy to hit. Prefers lines/points over faces
    /// through the caller-supplied priority function (higher wins at equal distance).
    pub fn pick(&self, x: i32, y: i32, radius: i32, priority: impl Fn(u32) -> i32) -> Option<u32> {
        let mut best: Option<(i32, i32, u32)> = None; // (priority, -dist2, id)
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let d2 = dx * dx + dy * dy;
                if d2 > radius * radius {
                    continue;
                }
                let Some(id) = self.id_at(x + dx, y + dy) else { continue };
                if id == 0 {
                    continue;
                }
                // `i32::MIN`: never picked (filtered out by the caller).
                let key = (priority(id), -d2, id);
                if key.0 == i32::MIN {
                    continue;
                }
                if best.is_none_or(|b| (key.0, key.1) > (b.0, b.1)) {
                    best = Some(key);
                }
            }
        }
        best.map(|b| b.2)
    }

    /// Distinct non-zero ids inside a frame-pixel rectangle `[x0, x1) x [y0, y1)`
    /// (clipped to this buffer), sorted.
    pub fn ids_in(&self, x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<u32> {
        let mut v = Vec::new();
        self.for_each(|x, y, id| {
            if x >= x0 && x < x1 && y >= y0 && y < y1 {
                v.push(id);
            }
        });
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Calls `f(x, y, id)` (frame pixel coordinates) for every non-zero id.
    pub fn for_each(&self, mut f: impl FnMut(i64, i64, u32)) {
        for (row, ids) in self.ids.chunks_exact(self.width.max(1) as usize).enumerate() {
            let y = self.y0 as i64 + row as i64;
            for (col, &id) in ids.iter().enumerate() {
                if id != 0 {
                    f(self.x0 as i64 + col as i64, y, id);
                }
            }
        }
    }
}

/// A rectangle of a frame in device pixels (clipped to the frame when read).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PickRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Non-zero: instead of the rectangle's ids, read about this many
    /// visible-depth samples spread over the whole frame
    /// ([`PickBuffer::samples`]; Onshape's `DEPTH_SAMPLES_PER_VIEWPORT`).
    pub depth_samples: u32,
}

impl PickRect {
    /// Square of `2 * half + 1` pixels centred on a pixel.
    pub fn around(x: i32, y: i32, half: i32) -> Self {
        let half = half.max(0);
        Self { x: x - half, y: y - half, width: (2 * half + 1) as u32, height: (2 * half + 1) as u32, depth_samples: 0 }
    }
    /// A visible-depth sampling query of about `n` samples.
    pub fn depth_samples(n: u32) -> Self {
        Self { depth_samples: n.max(1), ..Default::default() }
    }
    /// Clip to a `width` x `height` frame: (x0, y0, w, h), possibly empty.
    fn clip(&self, width: u32, height: u32) -> (u32, u32, u32, u32) {
        let x0 = self.x.clamp(0, width as i32);
        let y0 = self.y.clamp(0, height as i32);
        let x1 = (self.x as i64 + self.width as i64).clamp(0, width as i64) as i32;
        let y1 = (self.y as i64 + self.height as i64).clamp(0, height as i64) as i32;
        (x0 as u32, y0 as u32, (x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32)
    }
}

/// Everything needed to render one frame of a target.
#[derive(Clone)]
pub struct FrameRequest {
    pub scene: Arc<Scene>,
    pub camera: Camera,
    pub width: u32,
    pub height: u32,
    pub device_pixel_ratio: f32,
    pub style: Style,
    /// Geometry or pick state changed: re-render the pick buffer before the
    /// next pick query is answered (a size or camera change always does).
    pub invalidate_pick: bool,
}

/// Result of an asynchronous frame or pick read.
pub enum Completed {
    Frame { target: u64, seq: u64, result: Result<Frame, RenderError> },
    Pick { target: u64, token: u64, result: Result<PickBuffer, RenderError> },
}

struct Textures {
    width: u32,
    height: u32,
    msaa: u32,
    color_msaa: Option<wgpu::TextureView>,
    resolve: wgpu::Texture,
    resolve_view: wgpu::TextureView,
    depth: wgpu::TextureView,
    /// The depth aspect alone (sampled by the ambient occlusion pass).
    depth_sample: wgpu::TextureView,
    pick: wgpu::Texture,
    pick_view: wgpu::TextureView,
    pick_depth: wgpu::TextureView,
    padded_row: u32,
}

impl Textures {
    fn readback_size(&self) -> u64 {
        self.padded_row as u64 * self.height as u64
    }
}

#[derive(Clone)]
struct FrameState {
    scene: Arc<Scene>,
    camera: Camera,
    device_pixel_ratio: f32,
    style: Style,
}

#[derive(Default)]
struct TargetState {
    tex: Option<Textures>,
    present: Present,
    /// Free color readback buffers of the current size.
    spare: Vec<wgpu::Buffer>,
    /// [`Present::Texture`]: the textures of the current size; one is free
    /// when the ring holds the only reference.
    ring: Vec<Arc<PresentTexture>>,
    /// Parameters of the newest submitted frame (for lazy pick rendering).
    last: Option<FrameState>,
    /// The pick texture matches `last`.
    pick_valid: bool,
    in_flight: usize,
    stamp: u64,
}

/// A visible-depth sampling query in a pick readback buffer: the sampling
/// viewport's ids, then its depths (`width` x `height`, padded rows).
struct SampleRead {
    view_proj: Mat4,
}

struct RegionRead {
    token: u64,
    /// A sampling query (`offset` then holds the sampled rows).
    samples: Option<SampleRead>,
    x0: u32,
    y0: u32,
    width: u32,
    height: u32,
    frame_width: u32,
    frame_height: u32,
    offset: u64,
    padded_row: u32,
}

enum ReadKind {
    Frame { seq: u64, width: u32, height: u32, padded_row: u32 },
    /// A [`Present::Texture`] frame: nothing to read, done when the GPU is.
    Texture { seq: u64, texture: Arc<PresentTexture> },
    Pick { regions: Vec<RegionRead> },
}

struct PendingRead {
    target: u64,
    kind: ReadKind,
    /// The readback buffer (none for [`ReadKind::Texture`]).
    buffer: Option<wgpu::Buffer>,
    submission: wgpu::SubmissionIndex,
    state: Arc<AtomicU8>,
}

/// Per-frame draw lists (reused to avoid per-frame allocation).
#[derive(Default)]
struct DrawLists {
    opaque: Vec<usize>,
    transparent: Vec<usize>,
    /// Pick pass only: invisible pick-only meshes (sketch regions, wireframe
    /// faces). Drawn after the opaque meshes and win depth ties against them.
    pick_priority: Vec<usize>,
    /// Color pass only: images with an opaque instance color (write depth
    /// for their mostly opaque texels) and faded ones (blend only).
    images: Vec<usize>,
    images_faded: Vec<usize>,
    /// Color pass only: opaque meshes mirroring an environment map.
    reflective: Vec<usize>,
    /// Color pass only: opaque meshes drawn after the transparent ones.
    over_transparent: Vec<usize>,
    lines: Vec<usize>,
    points: Vec<usize>,
    /// Color pass only: `hidden_only` lines and points.
    lines_hidden: Vec<usize>,
    points_hidden: Vec<usize>,
    culled: usize,
}

impl DrawLists {
    /// Partition the scene's visible instances by pass, sorted by layer.
    fn build(&mut self, scene: &Scene, view_proj: &Mat4, width: u32, height: u32, dpr: f32, pick: bool) {
        self.opaque.clear();
        self.transparent.clear();
        self.pick_priority.clear();
        self.images.clear();
        self.images_faded.clear();
        self.reflective.clear();
        self.over_transparent.clear();
        self.lines.clear();
        self.points.clear();
        self.lines_hidden.clear();
        self.points_hidden.clear();
        self.culled = 0;
        // Screen-space margin for wide lines, points and the pick widening.
        let margin = |px: f32| (2.0 * px * dpr / width as f32, 2.0 * px * dpr / height as f32);
        let mesh_margin = margin(2.0);
        let sprite_margin = margin(24.0);
        for (i, inst) in scene.instances.iter().enumerate() {
            if pick {
                // Images are never picked (their box lines are) and don't
                // occlude picks either.
                if matches!(inst.drawable, Drawable::Image(_)) || (!inst.pickable && !matches!(inst.drawable, Drawable::Mesh(_))) {
                    continue;
                }
            } else if inst.pick_only {
                continue;
            }
            if pick && inst.hidden_only {
                continue;
            }
            let (bounds, m) = match &inst.drawable {
                Drawable::Mesh(m) => {
                    if m.0.index_count == 0 {
                        continue;
                    }
                    (m.0.bounds, mesh_margin)
                }
                Drawable::Lines(l) => {
                    if l.0.count == 0 {
                        continue;
                    }
                    (l.0.bounds, sprite_margin)
                }
                Drawable::Points(p) => {
                    if p.0.count == 0 {
                        continue;
                    }
                    (p.0.bounds, sprite_margin)
                }
                Drawable::Image(img) => (img.0.bounds, mesh_margin),
            };
            if let Some(b) = bounds
                && outside_frustum(b, &(*view_proj * inst.transform), m)
            {
                self.culled += 1;
                continue;
            }
            match &inst.drawable {
                Drawable::Mesh(_) if pick && inst.pick_only => self.pick_priority.push(i),
                Drawable::Mesh(_) if !pick && inst.reflection.is_some() && inst.color[3] >= 0.999 => self.reflective.push(i),
                Drawable::Mesh(_) if !pick && inst.over_transparent && inst.color[3] >= 0.999 => self.over_transparent.push(i),
                Drawable::Mesh(_) if inst.color[3] >= 0.999 => self.opaque.push(i),
                Drawable::Mesh(_) => self.transparent.push(i),
                Drawable::Image(_) if inst.color[3] >= 0.999 => self.images.push(i),
                Drawable::Image(_) => self.images_faded.push(i),
                Drawable::Lines(_) if inst.hidden_only => self.lines_hidden.push(i),
                Drawable::Points(_) if inst.hidden_only => self.points_hidden.push(i),
                Drawable::Lines(_) => self.lines.push(i),
                Drawable::Points(_) => self.points.push(i),
            }
        }
        if !pick {
            let o = perf_overrides();
            if o.skip_lines {
                self.lines.clear();
            }
            if o.skip_meshes {
                self.opaque.clear();
                self.transparent.clear();
                self.reflective.clear();
            }
        }
        let inst = &scene.instances;
        for v in [&mut self.opaque, &mut self.transparent, &mut self.pick_priority, &mut self.images, &mut self.images_faded, &mut self.reflective, &mut self.over_transparent, &mut self.lines, &mut self.points, &mut self.lines_hidden, &mut self.points_hidden] {
            // Stable by index within a layer (instances are already in index order).
            v.sort_by_key(|&i| inst[i].layer);
        }
    }
}

/// Conservative frustum test of a local AABB in homogeneous clip space: true
/// when all 8 corners lie outside one of the left/right/bottom/top planes
/// (widened by the `margin` in NDC units). Exact for boxes behind the camera.
fn outside_frustum((lo, hi): (Vec3, Vec3), mvp: &Mat4, (mx, my): (f32, f32)) -> bool {
    let (mut l, mut r, mut b, mut t) = (true, true, true, true);
    for i in 0..8 {
        let c = Vec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
        let p = *mvp * Vec4::new(c.x, c.y, c.z, 1.0);
        let (wx, wy) = (p.w * (1.0 + mx), p.w * (1.0 + my));
        l &= p.x < -wx;
        r &= p.x > wx;
        b &= p.y < -wy;
        t &= p.y > wy;
        if !(l || r || b || t) {
            return false;
        }
    }
    l || r || b || t
}

/// What a [`draw_lists`] pass draws: the pick ids, the section caps, the
/// part outlines.
#[derive(Clone, Copy, Default)]
struct DrawMode {
    pick: bool,
    section: bool,
    outline: bool,
    /// The visible-depth sample pass (Onshape's `DepthSamplingPass`): faces
    /// and points with true depth (no pick depth bias), no lines.
    depth_samples: bool,
    /// …with the datum planes' depth too (`includePlanes`).
    planes_write_depth: bool,
}

fn draw_lists(pass: &mut wgpu::RenderPass, lists: &DrawLists, instances: &[Instance], pipelines: &Pipelines, bind_inst: &wgpu::BindGroup, mode: DrawMode) {
    let DrawMode { pick, section, outline, depth_samples, planes_write_depth } = mode;
    let draw = |pass: &mut wgpu::RenderPass, list: &[usize], pipeline: &wgpu::RenderPipeline| {
        if list.is_empty() {
            return;
        }
        pass.set_pipeline(pipeline);
        for &i in list {
            let inst = &instances[i];
            pass.set_bind_group(1, bind_inst, &[(i as u64 * INST_STRIDE) as u32]);
            match &inst.drawable {
                Drawable::Mesh(m) => {
                    if let Some(r) = inst.reflection.as_ref().filter(|_| !pick) {
                        pass.set_bind_group(2, &r.environment.0.bind_group, &[]);
                    }
                    pass.set_vertex_buffer(0, m.0.vertices.slice(..));
                    pass.set_index_buffer(m.0.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.0.index_count, 0, 0..1);
                }
                Drawable::Lines(l) => {
                    pass.set_vertex_buffer(0, l.0.buffer.slice(..));
                    pass.draw(0..6, 0..l.0.count);
                }
                Drawable::Points(pt) => {
                    pass.set_vertex_buffer(0, pt.0.buffer.slice(..));
                    pass.draw(0..6, 0..pt.0.count);
                }
                Drawable::Image(img) => {
                    pass.set_bind_group(2, &img.0.bind_group, &[]);
                    pass.set_vertex_buffer(0, img.0.vertices.slice(..));
                    pass.draw(0..6, 0..1);
                }
            }
        }
    };
    if pick {
        // Transparent geometry (datum planes) first and without depth
        // writes: opaque parts behind it win the pick.
        // Unlit ones (datum planes) before lit see-through faces (in-context
        // geometry, transparent parts), so a face wins over a plane at the
        // same pixel (Onshape).
        let (flat, lit): (Vec<usize>, Vec<usize>) = lists.transparent.iter().partition(|&&i| instances[i].unlit);
        if depth_samples {
            if planes_write_depth {
                draw(pass, &flat, &pipelines.mesh_sample);
            }
            draw(pass, &lists.opaque, &pipelines.mesh_sample);
            draw(pass, &lists.pick_priority, &pipelines.mesh_sample);
            draw(pass, &lists.points, &pipelines.point_pick);
            return;
        }
        draw(pass, &flat, &pipelines.mesh_pick_transparent);
        draw(pass, &lit, &pipelines.mesh_pick_transparent);
        draw(pass, &lists.opaque, &pipelines.mesh_pick);
        // Pick-only meshes (e.g. sketch regions drawn on a part face) after
        // the faces: opaque faces are pushed back by their depth bias, so a
        // region lying exactly on a face wins even though the two
        // triangulations differ; parts in front of a region still win.
        draw(pass, &lists.pick_priority, &pipelines.mesh_pick_priority);
        draw(pass, &lists.lines, &pipelines.line_pick);
        draw(pass, &lists.points, &pipelines.point_pick);
    } else {
        draw(pass, &lists.opaque, &pipelines.mesh_opaque);
        if outline {
            draw(pass, &lists.opaque, &pipelines.mesh_outline);
        }
        if section {
            // Per part: reset the stencil, count the part's kept surfaces
            // behind each pixel, then draw its caps where the count says the
            // plane point is inside it (so a part lying wholly behind the
            // cut, buried in another, gets no cap).
            pass.set_stencil_reference(0);
            for &i in &lists.opaque {
                pass.set_pipeline(&pipelines.stencil_reset);
                pass.draw(0..3, 0..1);
                draw(pass, &[i], &pipelines.section_count);
                draw(pass, &[i], &pipelines.mesh_section);
            }
        }
        draw(pass, &lists.reflective, &pipelines.mesh_reflect);
        // Images before transparent meshes: their opaque texels write depth,
        // so datum planes / region fills behind them are hidden and those
        // in front blend over them.
        draw(pass, &lists.images, &pipelines.image_opaque);
        draw(pass, &lists.images, &pipelines.image_soft);
        draw(pass, &lists.images_faded, &pipelines.image_blend);
        draw(pass, &lists.transparent, &pipelines.mesh_transparent);
        draw(pass, &lists.over_transparent, &pipelines.mesh_opaque);
        // Hidden-only geometry right after the meshes, so only faces count
        // as hiding it (not hover outlines or overlays drawn over the same
        // curves); the visible lines come after.
        draw(pass, &lists.lines_hidden, &pipelines.line_hidden);
        draw(pass, &lists.points_hidden, &pipelines.point_hidden);
        draw(pass, &lists.lines, &pipelines.line);
        draw(pass, &lists.points, &pipelines.point);
        // Outlined see-through parts, last (their depth would hide what
        // is behind them).
        if outline {
            draw(pass, &lists.transparent, &pipelines.mesh_outline_depth);
            draw(pass, &lists.transparent, &pipelines.mesh_outline);
        }
    }
}

/// Diagnostics for GPU cost analysis (`NOSHAPE_PERF_MSAA=<n>` overrides the
/// sample count, `NOSHAPE_PERF_SKIP=lines,meshes` skips draw categories in
/// the color pass). Unset in normal use.
struct PerfOverrides {
    msaa: Option<u32>,
    skip_lines: bool,
    skip_meshes: bool,
}

fn perf_overrides() -> &'static PerfOverrides {
    static O: std::sync::OnceLock<PerfOverrides> = std::sync::OnceLock::new();
    O.get_or_init(|| {
        let skip = std::env::var("NOSHAPE_PERF_SKIP").unwrap_or_default();
        PerfOverrides {
            msaa: std::env::var("NOSHAPE_PERF_MSAA").ok().and_then(|v| v.parse().ok()),
            skip_lines: skip.contains("lines"),
            skip_meshes: skip.contains("meshes"),
        }
    })
}

fn msaa_of(style: &Style) -> u32 {
    match perf_overrides().msaa.unwrap_or(style.msaa) {
        0 | 1 => 1,
        2 => 2,
        8 => 8,
        _ => 4,
    }
}

fn make_globals(scene: &Scene, camera: &Camera, width: u32, height: u32, dpr: f32, s: &Style) -> Globals {
    let bounds = scene.bounds();
    let scene_radius = bounds
        .map(|(lo, hi)| {
            let c = (lo + hi) * 0.5;
            ((hi - lo).length() as f64 * 0.5) + (c.as_dvec3() - camera.target).length()
        })
        .unwrap_or(1.0);
    let aspect = width as f64 / height as f64;
    let vp = camera.view_proj_f32(aspect, scene_radius);
    let eye = camera.eye().as_vec3();
    let fwd = camera.forward().as_vec3();
    let up = camera.up().as_vec3();
    let (clip, clip_on) = match scene.clip {
        Some(c) => ([c.normal.x, c.normal.y, c.normal.z, c.d], 1),
        None => ([0.0; 4], 0),
    };
    // Extra section planes: geometry is removed where every plane clips it.
    let extra = |i: usize| scene.clip_extra.get(i).map(|c| [c.normal.x, c.normal.y, c.normal.z, c.d]).unwrap_or([0.0; 4]);
    let clip_count = if clip_on == 1 { 1 + scene.clip_extra.len().min(2) as u32 } else { 0 };
    Globals {
        view_proj: vp.to_cols_array_2d(),
        camera_pos: [eye.x, eye.y, eye.z, (camera.projection == Projection::Perspective) as u32 as f32],
        camera_fwd: [fwd.x, fwd.y, fwd.z, 0.0],
        camera_up: [up.x, up.y, up.z, 0.0],
        viewport: [width as f32, height as f32, dpr, 0.0],
        clip,
        flags: [clip_count, 0, 0, 0],
        clip2: extra(0),
        clip3: extra(1),
        bg_top: s.background_top,
        bg_bottom: s.background_bottom,
        hover: s.hover_color,
        select: s.select_color,
        accent: s.accent_color,
        specular: s.specular,
        dash: s.construction_dash,
        room: room_of(bounds),
        error: s.error_color,
    }
}

struct Pipelines {
    background: wgpu::RenderPipeline,
    mesh_opaque: wgpu::RenderPipeline,
    /// Silhouettes of outlined parts (state OUTLINE), after the opaque faces.
    mesh_outline: wgpu::RenderPipeline,
    /// Depth only, for outlined see-through parts (Interference detection
    /// results): their own faces then mask their back faces in the outline
    /// pass like an opaque part's do. Drawn last.
    mesh_outline_depth: wgpu::RenderPipeline,
    /// Section view: counts the surfaces behind each pixel in the stencil,
    /// then draws the caps where the plane is inside the material.
    section_count: wgpu::RenderPipeline,
    mesh_section: wgpu::RenderPipeline,
    /// Full-screen stencil reset to 0 (no color, no depth).
    stencil_reset: wgpu::RenderPipeline,
    mesh_transparent: wgpu::RenderPipeline,
    /// Opaque meshes mirroring an environment map (image bind group 2).
    mesh_reflect: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
    point: wgpu::RenderPipeline,
    /// Lines and points of `hidden_only` instances: drawn where something is
    /// in front of them (depth test Less, no depth write).
    line_hidden: wgpu::RenderPipeline,
    point_hidden: wgpu::RenderPipeline,
    mesh_pick: wgpu::RenderPipeline,
    mesh_pick_transparent: wgpu::RenderPipeline,
    /// Visible-depth samples: pick ids with true depth (no bias).
    mesh_sample: wgpu::RenderPipeline,
    mesh_pick_priority: wgpu::RenderPipeline,
    line_pick: wgpu::RenderPipeline,
    point_pick: wgpu::RenderPipeline,
    /// Opaque image texels (alpha >= 0.5), writing depth.
    image_opaque: wgpu::RenderPipeline,
    /// Soft image texels (alpha < 0.5) of opaque instances, no depth write.
    image_soft: wgpu::RenderPipeline,
    /// Faded image instances, no depth write.
    image_blend: wgpu::RenderPipeline,
    msaa: u32,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_info: wgpu::AdapterInfo,
    shader: wgpu::ShaderModule,
    globals_layout: wgpu::BindGroupLayout,
    inst_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    /// Texture + sampler of an image (group 2 of the image pipelines).
    image_layout: wgpu::BindGroupLayout,
    image_pipeline_layout: wgpu::PipelineLayout,
    image_sampler: wgpu::Sampler,
    pipelines: Option<Pipelines>,
    /// Ambient occlusion pass (built for the current MSAA count when used).
    ao: Option<crate::ao::AoPass>,
    globals_color: wgpu::Buffer,
    globals_pick: wgpu::Buffer,
    /// Globals of the visible-depth sampling pass (its own, so it can share a
    /// submission with the pick pass).
    globals_sample: wgpu::Buffer,
    states: wgpu::Buffer,
    states_capacity: u64,
    inst_buffer: wgpu::Buffer,
    inst_capacity: u64,
    bind_color: Option<wgpu::BindGroup>,
    bind_pick: Option<wgpu::BindGroup>,
    bind_sample: Option<wgpu::BindGroup>,
    bind_inst: Option<wgpu::BindGroup>,
    /// GPU copies of the state / instance buffers (skip unchanged uploads).
    last_states: Vec<u32>,
    last_inst: Vec<u8>,
    inst_scratch: Vec<u8>,
    lists: DrawLists,
    targets: HashMap<u64, TargetState>,
    pending: VecDeque<PendingRead>,
    ready: Vec<Completed>,
    pick_pool: Vec<wgpu::Buffer>,
    stamp: u64,
    frames_in_flight: usize,
    /// `NOSHAPE_PERF` on a device with timestamp queries: GPU time per frame.
    timer: Option<GpuTimer>,
}

/// GPU time of color frames (render and ambient occlusion passes), from
/// timestamp queries, recorded as `r.gpu_ms`. Each frame in flight uses one
/// slot: two queries and a small readback buffer mapped when the frame is done.
struct GpuTimer {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    slots: Vec<TimerSlot>,
    /// Nanoseconds per timestamp tick.
    period: f32,
}

struct TimerSlot {
    readback: wgpu::Buffer,
    state: Arc<AtomicU8>,
    busy: bool,
}

const TIMER_SLOTS: usize = 8;

impl GpuTimer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let set = device.create_query_set(&wgpu::QuerySetDescriptor { label: Some("frame-time"), ty: wgpu::QueryType::Timestamp, count: 2 * TIMER_SLOTS as u32 });
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame-time-resolve"),
            size: wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT * TIMER_SLOTS as u64,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let slots = (0..TIMER_SLOTS)
            .map(|_| TimerSlot {
                readback: device.create_buffer(&wgpu::BufferDescriptor { label: Some("frame-time-read"), size: 16, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }),
                state: Arc::new(AtomicU8::new(MAP_PENDING)),
                busy: false,
            })
            .collect();
        Self { set, resolve, slots, period: queue.get_timestamp_period() }
    }

    /// A free slot for the next frame (none while all are in flight).
    fn free_slot(&self) -> Option<usize> {
        self.slots.iter().position(|s| !s.busy)
    }

    /// Timestamp writes of slot `i`: start of the color pass, end of the
    /// last pass of the frame.
    fn writes(&self, i: usize, begin: bool, end: bool) -> wgpu::RenderPassTimestampWrites<'_> {
        let base = 2 * i as u32;
        wgpu::RenderPassTimestampWrites { query_set: &self.set, beginning_of_pass_write_index: begin.then_some(base), end_of_pass_write_index: end.then_some(base + 1) }
    }

    /// Copy slot `i`'s timestamps out (after the frame's passes).
    fn resolve(&self, encoder: &mut wgpu::CommandEncoder, i: usize) {
        let base = 2 * i as u32;
        let offset = wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT * i as u64;
        encoder.resolve_query_set(&self.set, base..base + 2, &self.resolve, offset);
        encoder.copy_buffer_to_buffer(&self.resolve, offset, &self.slots[i].readback, 0, 16);
    }

    /// Map slot `i`'s readback once the submission is done.
    fn mapped_after_submit(&mut self, i: usize) {
        let slot = &mut self.slots[i];
        slot.busy = true;
        slot.state.store(MAP_PENDING, Ordering::Release);
        let st = slot.state.clone();
        slot.readback.slice(..).map_async(wgpu::MapMode::Read, move |r| st.store(if r.is_ok() { MAP_OK } else { MAP_FAILED }, Ordering::Release));
    }

    /// Record the GPU time of every finished slot.
    fn collect(&mut self) {
        for slot in self.slots.iter_mut().filter(|s| s.busy) {
            match slot.state.load(Ordering::Acquire) {
                MAP_PENDING => continue,
                MAP_OK => {
                    let data = slot.readback.slice(..).get_mapped_range();
                    let t = bytemuck::cast_slice::<u8, u64>(&data);
                    let ms = t[1].saturating_sub(t[0]) as f64 * self.period as f64 / 1e6;
                    drop(data);
                    slot.readback.unmap();
                    perf::record_ms("r.gpu_ms", ms as f32);
                }
                _ => {}
            }
            slot.busy = false;
        }
    }
}

pub struct RenderParams<'a> {
    pub scene: &'a Scene,
    pub camera: &'a Camera,
    pub width: u32,
    pub height: u32,
    pub device_pixel_ratio: f32,
    pub style: &'a Style,
    /// Also render and read back the whole pick buffer.
    pub pick: bool,
}

impl Renderer {
    pub fn new() -> Result<Self, RenderError> {
        pollster::block_on(Self::new_async())
    }

    /// Blocking [`Self::new_preferring`].
    pub fn new_matching(prefer: impl Fn(&wgpu::AdapterInfo) -> bool) -> Result<Self, RenderError> {
        pollster::block_on(Self::new_preferring(prefer))
    }

    pub async fn new_async() -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| RenderError::NoAdapter(e.to_string()))?;
        Self::on_adapter(adapter).await
    }

    /// A renderer on the first adapter `prefer` accepts (e.g. the GPU the UI
    /// toolkit draws with, so frames can be shared), else as [`Self::new`].
    pub async fn new_preferring(prefer: impl Fn(&wgpu::AdapterInfo) -> bool) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        match instance.enumerate_adapters(wgpu::Backends::all()).await.into_iter().find(|a| prefer(&a.get_info())) {
            Some(adapter) => Self::on_adapter(adapter).await,
            None => Self::new_async().await,
        }
    }

    async fn on_adapter(adapter: wgpu::Adapter) -> Result<Self, RenderError> {
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("noshape"),
                required_features: adapter.features() & Self::WANTED_FEATURES,
                required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| RenderError::Device(e.to_string()))?;
        Ok(Self::with_device(&adapter, device, queue))
    }

    /// Optional device features the renderer uses when the device has them
    /// (float depth with stencil, else 24-bit depth; GPU timestamps for
    /// `NOSHAPE_PERF`).
    pub const WANTED_FEATURES: wgpu::Features = wgpu::Features::DEPTH32FLOAT_STENCIL8.union(wgpu::Features::TIMESTAMP_QUERY);

    /// A renderer on an existing device, e.g. the UI toolkit's, so its
    /// [`Present::Texture`] frames can be composited without a copy. The
    /// device should have [`Self::WANTED_FEATURES`] where the adapter has
    /// them and at least downlevel limits.
    pub fn with_device(adapter: &wgpu::Adapter, device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let adapter_info = adapter.get_info();
        let frames_in_flight = frames_in_flight_for(&adapter_info);
        log::info!("noshape renderer using {:?} ({:?})", adapter_info.name, adapter_info.backend);
        let float_stencil = device.features().contains(wgpu::Features::DEPTH32FLOAT_STENCIL8);
        DEPTH_FORMAT_CHOICE.get_or_init(|| if float_stencil { wgpu::TextureFormat::Depth32FloatStencil8 } else { wgpu::TextureFormat::Depth24PlusStencil8 });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("noshape-shaders"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders.wgsl").into()),
        });

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let inst_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("instance"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<InstUniform>() as u64),
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("noshape-layout"),
            bind_group_layouts: &[Some(&globals_layout), Some(&inst_layout)],
            immediate_size: 0,
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let image_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("noshape-image-layout"),
            bind_group_layouts: &[Some(&globals_layout), Some(&inst_layout), Some(&image_layout)],
            immediate_size: 0,
        });
        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("image"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            // Ignored (1) where anisotropic filtering is unsupported.
            anisotropy_clamp: 8,
            ..Default::default()
        });

        let mk_uniform = |label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<Globals>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let globals_color = mk_uniform("globals-color");
        let globals_pick = mk_uniform("globals-pick");
        let globals_sample = mk_uniform("globals-sample");
        let states_capacity = 1024;
        let states = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("states"),
            size: states_capacity * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let inst_capacity = 64;
        let inst_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances"),
            size: inst_capacity * INST_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let timer = (perf::enabled() && device.features().contains(wgpu::Features::TIMESTAMP_QUERY)).then(|| GpuTimer::new(&device, &queue));
        Self {
            device,
            queue,
            adapter_info,
            shader,
            globals_layout,
            inst_layout,
            pipeline_layout,
            image_layout,
            image_pipeline_layout,
            image_sampler,
            pipelines: None,
            ao: None,
            globals_color,
            globals_pick,
            globals_sample,
            states,
            states_capacity,
            inst_buffer,
            inst_capacity,
            bind_color: None,
            bind_pick: None,
            bind_sample: None,
            bind_inst: None,
            last_states: Vec::new(),
            last_inst: Vec::new(),
            inst_scratch: Vec::new(),
            lists: DrawLists::default(),
            targets: HashMap::new(),
            pending: VecDeque::new(),
            ready: Vec::new(),
            pick_pool: Vec::new(),
            stamp: 0,
            frames_in_flight,
            timer,
        }
    }

    /// The GPU device the renderer draws with.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn adapter_name(&self) -> String {
        format!("{} ({:?})", self.adapter_info.name, self.adapter_info.backend)
    }

    /// A cloneable handle for uploading geometry from any thread.
    pub fn uploader(&self) -> Uploader {
        Uploader { device: self.device.clone(), queue: self.queue.clone(), image_layout: self.image_layout.clone(), image_sampler: self.image_sampler.clone() }
    }
    pub fn upload_mesh(&self, mesh: &MeshData) -> MeshHandle {
        self.uploader().upload_mesh(mesh)
    }
    pub fn upload_lines(&self, lines: &LineData) -> LinesHandle {
        self.uploader().upload_lines(lines)
    }
    pub fn upload_points(&self, points: &PointData) -> PointsHandle {
        self.uploader().upload_points(points)
    }
    pub fn upload_image(&self, image: &ImageData) -> ImageHandle {
        self.uploader().upload_image(image)
    }

    // ------------------------------------------------------------ setup

    fn ensure_pipelines(&mut self, msaa: u32) {
        if self.pipelines.as_ref().is_some_and(|p| p.msaa == msaa) {
            return;
        }
        let d = &self.device;
        let sh = &self.shader;
        let layout = &self.pipeline_layout;
        let image_layout = &self.image_pipeline_layout;

        let mesh_buffers = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MeshVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Uint32, 3 => Unorm8x4],
        }];
        let line_buffers = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineSegment>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Unorm8x4, 3 => Uint32, 4 => Float32, 5 => Uint32],
        }];
        let point_buffers = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PointSprite>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Uint32, 3 => Float32, 4 => Uint32],
        }];
        let image_buffers = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ImageVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2],
        }];

        struct P<'a> {
            label: &'a str,
            vs: &'a str,
            fs: &'a str,
            buffers: &'a [wgpu::VertexBufferLayout<'a>],
            format: wgpu::TextureFormat,
            blend: Option<wgpu::BlendState>,
            depth: Option<(bool, wgpu::CompareFunction)>,
            bias: wgpu::DepthBiasState,
            samples: u32,
        }
        // Section caps: the counting pass and the caps (see draw_lists).
        struct Extra {
            stencil: wgpu::StencilState,
            writes: wgpu::ColorWrites,
            cull: Option<wgpu::Face>,
        }
        let plain = || Extra { stencil: Default::default(), writes: wgpu::ColorWrites::ALL, cull: None };
        let make_full = |layout: &wgpu::PipelineLayout, p: P, x: Extra| {
            d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(p.label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: sh,
                    entry_point: Some(p.vs),
                    buffers: p.buffers,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: sh,
                    entry_point: Some(p.fs),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: p.format,
                        blend: p.blend,
                        write_mask: x.writes,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: x.cull,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: p.depth.map(|(write, cmp)| wgpu::DepthStencilState {
                    format: if p.format == PICK_FORMAT { PICK_DEPTH_FORMAT } else { depth_format() },
                    depth_write_enabled: Some(write),
                    depth_compare: Some(cmp),
                    stencil: x.stencil.clone(),
                    bias: p.bias,
                }),
                multisample: wgpu::MultisampleState { count: p.samples, mask: !0, alpha_to_coverage_enabled: false },
                multiview_mask: None,
                cache: None,
            })
        };
        let make_in = |layout: &wgpu::PipelineLayout, p: P| make_full(layout, p, plain());
        let make = |p: P| make_in(layout, p);
        let ge = wgpu::CompareFunction::GreaterEqual;
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        // Image texels are premultiplied.
        let premul = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let none = wgpu::DepthBiasState::default();
        let (face, priority) = (FACE_DEPTH_BIAS, PICK_PRIORITY_DEPTH_BIAS);
        let pipelines = Pipelines {
            background: make(P { label: "background", vs: "vs_background", fs: "fs_background", buffers: &[], format: COLOR_FORMAT, blend: None, depth: Some((false, wgpu::CompareFunction::Always)), bias: none, samples: msaa }),
            mesh_opaque: make(P { label: "mesh", vs: "vs_mesh", fs: "fs_mesh", buffers: &mesh_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((true, ge)), bias: face, samples: msaa }),
            mesh_outline: make(P { label: "mesh-outline", vs: "vs_outline", fs: "fs_outline", buffers: &mesh_buffers, format: COLOR_FORMAT, blend: None, depth: Some((false, ge)), bias: face, samples: msaa }),
            mesh_outline_depth: make_full(
                layout,
                P { label: "mesh-outline-depth", vs: "vs_mesh", fs: "fs_outline_depth", buffers: &mesh_buffers, format: COLOR_FORMAT, blend: None, depth: Some((true, ge)), bias: face, samples: msaa },
                Extra { stencil: Default::default(), writes: wgpu::ColorWrites::empty(), cull: None },
            ),
            // Counting: every kept surface along the ray, front faces -1 and
            // back faces +1; no color or depth.
            section_count: make_full(
                layout,
                P { label: "section-count", vs: "vs_mesh", fs: "fs_section_count", buffers: &mesh_buffers, format: COLOR_FORMAT, blend: None, depth: Some((false, wgpu::CompareFunction::Always)), bias: none, samples: msaa },
                Extra {
                    stencil: wgpu::StencilState {
                        front: wgpu::StencilFaceState { compare: wgpu::CompareFunction::Always, fail_op: wgpu::StencilOperation::Keep, depth_fail_op: wgpu::StencilOperation::Keep, pass_op: wgpu::StencilOperation::DecrementWrap },
                        back: wgpu::StencilFaceState { compare: wgpu::CompareFunction::Always, fail_op: wgpu::StencilOperation::Keep, depth_fail_op: wgpu::StencilOperation::Keep, pass_op: wgpu::StencilOperation::IncrementWrap },
                        read_mask: 0xff,
                        write_mask: 0xff,
                    },
                    writes: wgpu::ColorWrites::empty(),
                    cull: None,
                },
            ),
            stencil_reset: make_full(
                layout,
                P { label: "stencil-reset", vs: "vs_background", fs: "fs_background", buffers: &[], format: COLOR_FORMAT, blend: None, depth: Some((false, wgpu::CompareFunction::Always)), bias: none, samples: msaa },
                Extra {
                    stencil: wgpu::StencilState {
                        front: wgpu::StencilFaceState { compare: wgpu::CompareFunction::Always, fail_op: wgpu::StencilOperation::Zero, depth_fail_op: wgpu::StencilOperation::Zero, pass_op: wgpu::StencilOperation::Zero },
                        back: wgpu::StencilFaceState { compare: wgpu::CompareFunction::Always, fail_op: wgpu::StencilOperation::Zero, depth_fail_op: wgpu::StencilOperation::Zero, pass_op: wgpu::StencilOperation::Zero },
                        read_mask: 0xff,
                        write_mask: 0xff,
                    },
                    writes: wgpu::ColorWrites::empty(),
                    cull: None,
                },
            ),
            // Caps: back faces where the count is not zero (the plane point is
            // inside the material), drawn at the plane.
            mesh_section: make_full(
                layout,
                P { label: "mesh-section", vs: "vs_mesh", fs: "fs_mesh_section", buffers: &mesh_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((true, ge)), bias: face, samples: msaa },
                Extra {
                    stencil: wgpu::StencilState {
                        front: wgpu::StencilFaceState { compare: wgpu::CompareFunction::NotEqual, fail_op: wgpu::StencilOperation::Keep, depth_fail_op: wgpu::StencilOperation::Keep, pass_op: wgpu::StencilOperation::Keep },
                        back: wgpu::StencilFaceState { compare: wgpu::CompareFunction::NotEqual, fail_op: wgpu::StencilOperation::Keep, depth_fail_op: wgpu::StencilOperation::Keep, pass_op: wgpu::StencilOperation::Keep },
                        read_mask: 0xff,
                        write_mask: 0,
                    },
                    writes: wgpu::ColorWrites::ALL,
                    cull: Some(wgpu::Face::Front),
                },
            ),
            mesh_reflect: make_in(image_layout, P { label: "mesh-reflect", vs: "vs_mesh", fs: "fs_mesh_reflect", buffers: &mesh_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((true, ge)), bias: face, samples: msaa }),
            // Strictly in front: a transparent plane coplanar with a sketch
            // image (drawn just before, without bias) doesn't veil it. Opaque
            // faces carry FACE_DEPTH_BIAS, so coplanar planes still tint them.
            mesh_transparent: make(P { label: "mesh-t", vs: "vs_mesh", fs: "fs_mesh", buffers: &mesh_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((false, wgpu::CompareFunction::Greater)), bias: none, samples: msaa }),
            line: make(P { label: "line", vs: "vs_line", fs: "fs_line", buffers: &line_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((true, ge)), bias: none, samples: msaa }),
            line_hidden: make(P { label: "line-hidden", vs: "vs_line", fs: "fs_line", buffers: &line_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((false, wgpu::CompareFunction::Less)), bias: none, samples: msaa }),
            point: make(P { label: "point", vs: "vs_point", fs: "fs_point", buffers: &point_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((true, ge)), bias: none, samples: msaa }),
            point_hidden: make(P { label: "point-hidden", vs: "vs_point", fs: "fs_point", buffers: &point_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((false, wgpu::CompareFunction::Less)), bias: none, samples: msaa }),
            mesh_pick: make(P { label: "mesh-pick", vs: "vs_mesh", fs: "fs_mesh_pick", buffers: &mesh_buffers, format: PICK_FORMAT, blend: None, depth: Some((true, ge)), bias: face, samples: 1 }),
            mesh_pick_transparent: make(P { label: "mesh-pick-t", vs: "vs_mesh", fs: "fs_mesh_pick", buffers: &mesh_buffers, format: PICK_FORMAT, blend: None, depth: Some((false, ge)), bias: none, samples: 1 }),
            mesh_sample: make(P { label: "mesh-sample", vs: "vs_mesh", fs: "fs_mesh_pick", buffers: &mesh_buffers, format: PICK_FORMAT, blend: None, depth: Some((true, ge)), bias: none, samples: 1 }),
            mesh_pick_priority: make(P { label: "mesh-pick-p", vs: "vs_mesh", fs: "fs_mesh_pick", buffers: &mesh_buffers, format: PICK_FORMAT, blend: None, depth: Some((true, ge)), bias: priority, samples: 1 }),
            line_pick: make(P { label: "line-pick", vs: "vs_line", fs: "fs_line_pick", buffers: &line_buffers, format: PICK_FORMAT, blend: None, depth: Some((true, ge)), bias: none, samples: 1 }),
            point_pick: make(P { label: "point-pick", vs: "vs_point", fs: "fs_point_pick", buffers: &point_buffers, format: PICK_FORMAT, blend: None, depth: Some((true, ge)), bias: none, samples: 1 }),
            image_opaque: make_in(image_layout, P { label: "image", vs: "vs_image", fs: "fs_image_opaque", buffers: &image_buffers, format: COLOR_FORMAT, blend: premul, depth: Some((true, ge)), bias: none, samples: msaa }),
            image_soft: make_in(image_layout, P { label: "image-soft", vs: "vs_image", fs: "fs_image_soft", buffers: &image_buffers, format: COLOR_FORMAT, blend: premul, depth: Some((false, ge)), bias: none, samples: msaa }),
            image_blend: make_in(image_layout, P { label: "image-blend", vs: "vs_image", fs: "fs_image_blend", buffers: &image_buffers, format: COLOR_FORMAT, blend: premul, depth: Some((false, ge)), bias: none, samples: msaa }),
            msaa,
        };
        self.pipelines = Some(pipelines);
    }

    fn rebuild_bind_groups(&mut self) {
        let d = &self.device;
        let mk = |globals: &wgpu::Buffer| {
            d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("globals"),
                layout: &self.globals_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.states.as_entire_binding() },
                ],
            })
        };
        self.bind_color = Some(mk(&self.globals_color));
        self.bind_pick = Some(mk(&self.globals_pick));
        self.bind_sample = Some(mk(&self.globals_sample));
        self.bind_inst = Some(d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("instances"),
            layout: &self.inst_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &self.inst_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<InstUniform>() as u64),
                }),
            }],
        }));
    }

    /// Upload per-pick-id state flags (only when they changed).
    fn upload_states(&mut self, states: &[u32]) {
        let needed = (states.len() as u64).max(1);
        if needed > self.states_capacity {
            self.states_capacity = needed.next_power_of_two();
            // New buffers are zero-initialised.
            self.states = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("states"),
                size: self.states_capacity * 4,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.bind_color = None;
            self.last_states.clear();
        }
        if self.last_states.as_slice() != states {
            // Entries beyond `states.len()` must read as 0: clear what the
            // previous upload left there.
            if !states.is_empty() {
                self.queue.write_buffer(&self.states, 0, bytemuck::cast_slice(states));
            }
            if self.last_states.len() > states.len() {
                let zeros = vec![0u32; self.last_states.len() - states.len()];
                self.queue.write_buffer(&self.states, states.len() as u64 * 4, bytemuck::cast_slice(&zeros));
            }
            self.last_states.clear();
            self.last_states.extend_from_slice(states);
        }
    }

    /// Upload per-instance uniforms (only when they changed).
    fn upload_instances(&mut self, scene: &Scene) {
        let n = scene.instances.len().max(1) as u64;
        if n > self.inst_capacity {
            self.inst_capacity = n.next_power_of_two();
            self.inst_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instances"),
                size: self.inst_capacity * INST_STRIDE,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.bind_color = None;
            self.last_inst.clear();
        }
        let bytes = &mut self.inst_scratch;
        bytes.clear();
        bytes.resize((scene.instances.len() as u64 * INST_STRIDE) as usize, 0);
        for (i, inst) in scene.instances.iter().enumerate() {
            let normal_m = inst.transform.inverse().transpose();
            let u = InstUniform {
                model: inst.transform.to_cols_array_2d(),
                normal_m: normal_m.to_cols_array_2d(),
                color: inst.color,
                params: [(inst.color_mode == ColorMode::PerPrimitive) as u32, inst.ignore_clip as u32 | ((inst.unlit as u32) << 1) | ((inst.preview as u32) << 4) | zebra_bits(inst.zebra), inst.pickable as u32, inst.pick_base],
                extra: match &inst.reflection {
                    Some(r) => [inst.line_scale, r.rotation, if r.blur { 3.0 } else { 0.0 }, r.lock as u32 as f32],
                    None => [inst.line_scale, 0.0, 0.0, 0.0],
                },
                _pad: [0.0; 20],
            };
            let off = i * INST_STRIDE as usize;
            bytes[off..off + std::mem::size_of::<InstUniform>()].copy_from_slice(bytemuck::bytes_of(&u));
        }
        if !bytes.is_empty() && *bytes != self.last_inst {
            self.queue.write_buffer(&self.inst_buffer, 0, bytes);
            std::mem::swap(&mut self.last_inst, &mut self.inst_scratch);
        }
    }

    /// Bump the LRU clock, evict an idle target if over budget, and return
    /// the state of `target` with textures for the given size.
    fn prepare_target(&mut self, target: u64, width: u32, height: u32, msaa: u32) -> &mut TargetState {
        self.stamp += 1;
        if !self.targets.contains_key(&target) && self.targets.len() >= MAX_TARGETS {
            let victim = self.targets.iter().filter(|(_, t)| t.in_flight == 0).min_by_key(|(_, t)| t.stamp).map(|(k, _)| *k);
            if let Some(k) = victim {
                self.targets.remove(&k);
            }
        }
        let device = &self.device;
        let ts = self.targets.entry(target).or_default();
        ts.stamp = self.stamp;
        if !ts.tex.as_ref().is_some_and(|t| t.width == width && t.height == height && t.msaa == msaa) {
            ts.tex = Some(create_textures(device, width, height, msaa));
            ts.spare.clear();
            ts.pick_valid = false;
        }
        ts.ring.retain(|t| t.width == width && t.height == height);
        ts
    }

    /// How `target`'s color frames are delivered from now on (default
    /// [`Present::Readback`]).
    pub fn set_present(&mut self, target: u64, present: Present) {
        let ts = self.targets.entry(target).or_default();
        ts.present = present;
        if present == Present::Readback {
            ts.ring.clear();
        }
    }

    /// Create the render pipelines for `msaa` ahead of the first frame
    /// (shader compilation otherwise delays it).
    pub fn prewarm(&mut self, msaa: u32) {
        let _t = perf::scope("r.prewarm_pipelines");
        self.ensure_pipelines(msaa_of(&Style { msaa, ..Style::default() }));
    }

    /// Forget a target (its textures are released once in-flight reads finish).
    pub fn drop_target(&mut self, target: u64) {
        self.targets.remove(&target);
    }

    /// Frames a target may have in flight: 2 on real GPUs (frame N+1 renders
    /// while frame N is read back), 1 on CPU rasterizers (llvmpipe/WARP),
    /// where a second frame only competes with the UI thread for the same
    /// cores. `NOSHAPE_FRAMES_IN_FLIGHT` overrides.
    pub fn frames_in_flight(&self) -> usize {
        self.frames_in_flight
    }

    /// Whether `target` can accept another frame now (see [`Self::frames_in_flight`]).
    /// A [`Present::Texture`] target also needs a texture nobody holds any
    /// more (they are released by dropping the delivered frames).
    pub fn can_submit(&self, target: u64) -> bool {
        self.targets.get(&target).is_none_or(|t| {
            t.in_flight < self.frames_in_flight && (t.present == Present::Readback || t.ring.len() < MAX_PRESENT_TEXTURES || t.ring.iter().any(|x| Arc::strong_count(x) == 1))
        })
    }

    /// Whether reads are outstanding (call [`Self::wait_oldest`] / [`Self::collect`]).
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty() || !self.ready.is_empty()
    }

    // ------------------------------------------------------------ render

    /// Encode and submit a color frame for `target`; its pixels are delivered
    /// by [`Self::collect`] once the GPU finished. Never waits for the GPU.
    pub fn submit_frame(&mut self, target: u64, seq: u64, req: FrameRequest) -> Result<(), RenderError> {
        let t_prep = perf::scope("r.prep");
        let width = req.width.clamp(1, 8192);
        let height = req.height.clamp(1, 8192);
        let msaa = msaa_of(&req.style);
        self.ensure_pipelines(msaa);
        self.upload_states(&req.scene.states);
        self.upload_instances(&req.scene);
        if self.bind_color.is_none() {
            self.rebuild_bind_groups();
        }
        let dpr = req.device_pixel_ratio;
        let globals = make_globals(&req.scene, &req.camera, width, height, dpr, &req.style);
        self.queue.write_buffer(&self.globals_color, 0, bytemuck::bytes_of(&globals));
        let mut lists = std::mem::take(&mut self.lists);
        lists.build(&req.scene, &Mat4::from_cols_array_2d(&globals.view_proj), width, height, dpr, false);
        if perf::enabled() {
            perf::record_ms("r.culled_instances", lists.culled as f32);
        }
        self.prepare_target(target, width, height, msaa);
        let ao_strength = req.style.ambient_occlusion;
        if ao_strength > 0.001 && self.ao.as_ref().is_none_or(|a| a.msaa != msaa) {
            self.ao = Some(crate::ao::AoPass::new(&self.device, msaa, COLOR_FORMAT));
        }
        drop(t_prep);

        let t_encode = perf::scope("r.encode_submit");
        let Self { device, queue, pipelines, targets, bind_color, bind_inst, pending, ao, timer, .. } = self;
        let ao_on = ao_strength > 0.001 && ao.is_some();
        let timer_slot = timer.as_ref().and_then(|t| t.free_slot());
        let pipelines = pipelines.as_ref().expect("pipelines");
        let ts = targets.get_mut(&target).expect("target");
        if req.invalidate_pick {
            ts.pick_valid = false;
        }
        let tex = ts.tex.as_ref().expect("textures");
        // Texture targets render into a free texture of their ring (the
        // readback path into the target's own color texture).
        let present_texture = (ts.present == Present::Texture).then(|| match ts.ring.iter().find(|t| Arc::strong_count(t) == 1) {
            Some(t) => t.clone(),
            None => {
                let t = Arc::new(create_present_texture(device, width, height));
                ts.ring.push(t.clone());
                t
            }
        });
        let readback = present_texture.is_none().then(|| {
            ts.spare.pop().unwrap_or_else(|| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("color-readback"),
                    size: tex.readback_size(),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                })
            })
        });
        let color_view = present_texture.as_ref().map_or(&tex.resolve_view, |t| &t.target_view);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let (view, resolve) = match &tex.color_msaa {
                Some(v) => (v, Some(color_view)),
                None => (color_view, None),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("color"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: resolve,
                    // The MSAA buffer is only needed until it is resolved.
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: if resolve.is_some() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store } },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &tex.depth,
                    // Kept for the ambient occlusion pass.
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: if ao_strength > 0.001 { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard } }),
                    // Section caps count surfaces in the stencil.
                    stencil_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0), store: wgpu::StoreOp::Discard }),
                }),
                timestamp_writes: timer.as_ref().zip(timer_slot).map(|(t, i)| t.writes(i, true, !ao_on)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let bind_inst = bind_inst.as_ref().expect("bind groups");
            pass.set_bind_group(0, bind_color.as_ref().expect("bind groups"), &[]);
            pass.set_bind_group(1, bind_inst, &[0]);
            pass.set_pipeline(&pipelines.background);
            pass.draw(0..3, 0..1);
            let outline = req.scene.states.iter().any(|s| s & crate::scene::state::OUTLINE != 0);
            draw_lists(&mut pass, &lists, &req.scene.instances, pipelines, bind_inst, DrawMode { section: req.scene.clip.is_some(), outline, ..Default::default() });
        }
        if ao_strength > 0.001
            && let Some(ao) = ao.as_ref()
        {
            let timestamp_writes = timer.as_ref().zip(timer_slot).map(|(t, i)| t.writes(i, false, true));
            let frame = crate::ao::AoFrame { view_proj: Mat4::from_cols_array_2d(&globals.view_proj), width, height, dpr, strength: ao_strength, timestamp_writes };
            ao.encode(device, queue, &mut encoder, &tex.depth_sample, color_view, frame);
        }
        if let (Some(t), Some(i)) = (timer.as_ref(), timer_slot) {
            t.resolve(&mut encoder, i);
        }
        if let Some(readback) = &readback {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture: &tex.resolve, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo { buffer: readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(tex.padded_row), rows_per_image: Some(height) } },
                wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            );
        }
        let submission = queue.submit(Some(encoder.finish()));
        if let (Some(t), Some(i)) = (timer.as_mut(), timer_slot) {
            t.mapped_after_submit(i);
        }
        let state = Arc::new(AtomicU8::new(MAP_PENDING));
        let st = state.clone();
        let kind = match (&readback, present_texture) {
            (Some(readback), _) => {
                readback.slice(..).map_async(wgpu::MapMode::Read, move |r| st.store(if r.is_ok() { MAP_OK } else { MAP_FAILED }, Ordering::Release));
                ReadKind::Frame { seq, width, height, padded_row: tex.padded_row }
            }
            (None, Some(texture)) => {
                queue.on_submitted_work_done(move || st.store(MAP_OK, Ordering::Release));
                ReadKind::Texture { seq, texture }
            }
            (None, None) => unreachable!("a frame either reads back or keeps its texture"),
        };
        pending.push_back(PendingRead { target, kind, buffer: readback, submission, state });
        ts.in_flight += 1;
        ts.last = Some(FrameState { scene: req.scene, camera: req.camera, device_pixel_ratio: dpr, style: req.style });
        self.lists = lists;
        drop(t_encode);
        Ok(())
    }

    /// Read rectangles of `target`'s pick buffer for its newest frame. The
    /// pick pass is rendered first if the frame invalidated it. Results come
    /// from [`Self::collect`] as [`Completed::Pick`] with the given tokens.
    pub fn submit_picks(&mut self, target: u64, queries: &[(u64, PickRect)]) {
        if queries.is_empty() {
            return;
        }
        let _t = perf::scope("r.pick_submit");
        let Some(last) = self.targets.get(&target).and_then(|t| t.last.clone()) else {
            for &(token, _) in queries {
                self.ready.push(Completed::Pick { target, token, result: Err(RenderError::Readback("no frame rendered for this target yet".into())) });
            }
            return;
        };
        let (width, height, msaa) = {
            let tex = self.targets[&target].tex.as_ref().expect("textures");
            (tex.width, tex.height, tex.msaa)
        };
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pick") });
        if !self.targets[&target].pick_valid {
            self.ensure_pipelines(msaa);
            self.upload_states(&last.scene.states);
            self.upload_instances(&last.scene);
            if self.bind_color.is_none() {
                self.rebuild_bind_groups();
            }
            let mut globals = make_globals(&last.scene, &last.camera, width, height, last.device_pixel_ratio, &last.style);
            globals.viewport[3] = 8.0 * last.device_pixel_ratio;
            self.queue.write_buffer(&self.globals_pick, 0, bytemuck::bytes_of(&globals));
            let mut lists = std::mem::take(&mut self.lists);
            lists.build(&last.scene, &Mat4::from_cols_array_2d(&globals.view_proj), width, height, last.device_pixel_ratio, true);
            let tex = self.targets[&target].tex.as_ref().expect("textures");
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("pick"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &tex.pick_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &tex.pick_depth,
                        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_bind_group(0, self.bind_pick.as_ref().expect("bind groups"), &[]);
                draw_lists(&mut pass, &lists, &last.scene.instances, self.pipelines.as_ref().expect("pipelines"), self.bind_inst.as_ref().expect("bind groups"), DrawMode { pick: true, ..Default::default() });
            }
            self.lists = lists;
            self.targets.get_mut(&target).expect("target").pick_valid = true;
        }
        // Visible-depth sampling queries render their own tiny pass.
        let samples: Vec<(u64, SampleRead, Vec<wgpu::Texture>)> = queries
            .iter()
            .filter(|(_, r)| r.depth_samples > 0)
            .map(|&(token, r)| {
                let (read, textures) = self.render_depth_samples(&mut encoder, &last, width, height, r.depth_samples, msaa);
                (token, read, textures)
            })
            .collect();
        // Lay the rectangles out in one readback buffer.
        let mut regions = Vec::with_capacity(queries.len());
        let mut size = 0u64;
        for &(token, rect) in queries {
            if rect.depth_samples > 0 {
                let (sw, sh) = depth_sample_size(width, height, rect.depth_samples);
                let padded_row = (sw * 4).div_ceil(256).max(1) * 256;
                let read = samples.iter().find(|s| s.0 == token).map(|s| SampleRead { view_proj: s.1.view_proj });
                regions.push(RegionRead { token, samples: read, x0: 0, y0: 0, width: sw, height: sh, frame_width: sw, frame_height: sh, offset: size, padded_row });
                size += padded_row as u64 * sh as u64 * 4;
                continue;
            }
            let (x0, y0, w, h) = rect.clip(width, height);
            let padded_row = (w * 4).div_ceil(256).max(1) * 256;
            regions.push(RegionRead { token, samples: None, x0, y0, width: w, height: h, frame_width: width, frame_height: height, offset: size, padded_row });
            if w > 0 && h > 0 {
                size += padded_row as u64 * h as u64;
            }
        }
        if size == 0 {
            for r in regions {
                self.ready.push(Completed::Pick { target, token: r.token, result: Ok(PickBuffer { x0: r.x0, y0: r.y0, width: 0, height: 0, frame_width: width, frame_height: height, ids: Vec::new(), samples: Vec::new(), samples_with_planes: Vec::new(), view_proj: None }) });
            }
            // The pick pass (if any) still has to run so later queries can reuse it.
            self.queue.submit(Some(encoder.finish()));
            return;
        }
        let buffer = match self.pick_pool.iter().position(|b| b.size() >= size) {
            Some(i) => self.pick_pool.swap_remove(i),
            None => self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pick-readback"),
                size: size.next_power_of_two().max(64 * 1024),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
        };
        let tex = self.targets[&target].tex.as_ref().expect("textures");
        for r in regions.iter().filter(|r| r.width > 0 && r.height > 0) {
            if r.samples.is_some() {
                let Some((_, _, textures)) = samples.iter().find(|s| s.0 == r.token) else { continue };
                let block = r.padded_row as u64 * r.height as u64;
                for (k, texture) in textures.iter().enumerate() {
                    let aspect = if k % 2 == 0 { wgpu::TextureAspect::All } else { wgpu::TextureAspect::DepthOnly };
                    let offset = r.offset + block * k as u64;
                    encoder.copy_texture_to_buffer(
                        wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect },
                        wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset, bytes_per_row: Some(r.padded_row), rows_per_image: Some(r.height) } },
                        wgpu::Extent3d { width: r.width, height: r.height, depth_or_array_layers: 1 },
                    );
                }
                continue;
            }
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture: &tex.pick, mip_level: 0, origin: wgpu::Origin3d { x: r.x0, y: r.y0, z: 0 }, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: r.offset, bytes_per_row: Some(r.padded_row), rows_per_image: Some(r.height) } },
                wgpu::Extent3d { width: r.width, height: r.height, depth_or_array_layers: 1 },
            );
        }
        let submission = self.queue.submit(Some(encoder.finish()));
        let state = Arc::new(AtomicU8::new(MAP_PENDING));
        let st = state.clone();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| st.store(if r.is_ok() { MAP_OK } else { MAP_FAILED }, Ordering::Release));
        self.pending.push_back(PendingRead { target, kind: ReadKind::Pick { regions }, buffer: Some(buffer), submission, state });
    }

    /// Onshape's `DepthSamplingPass`: the pick pass again, into a viewport
    /// shrunk to about `n` pixels (cheap), whose ids and depth are read back
    /// whole, twice (without and with plane depth). Returns the read
    /// description and the textures to copy: ids, depth, ids, depth.
    fn render_depth_samples(&mut self, encoder: &mut wgpu::CommandEncoder, last: &FrameState, width: u32, height: u32, n: u32, msaa: u32) -> (SampleRead, Vec<wgpu::Texture>) {
        let (sw, sh) = depth_sample_size(width, height, n);
        self.ensure_pipelines(msaa);
        self.upload_states(&last.scene.states);
        self.upload_instances(&last.scene);
        if self.bind_color.is_none() {
            self.rebuild_bind_groups();
        }
        // Line and point widths stay in device pixels of the full frame.
        let dpr = last.device_pixel_ratio * sw as f32 / width.max(1) as f32;
        let mut globals = make_globals(&last.scene, &last.camera, sw, sh, dpr, &last.style);
        globals.viewport[3] = 8.0 * dpr;
        self.queue.write_buffer(&self.globals_sample, 0, bytemuck::bytes_of(&globals));
        let view_proj = Mat4::from_cols_array_2d(&globals.view_proj);
        let mut lists = DrawLists::default();
        lists.build(&last.scene, &view_proj, sw, sh, dpr, true);
        let tex = |label, format| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: sw, height: sh, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        // Without plane depth, then with it (Onshape retrieves the samples
        // with `includePlanes` false, then true when too few hit).
        let mut textures = Vec::with_capacity(4);
        for planes_write_depth in [false, true] {
            let ids = tex("depth-sample-ids", PICK_FORMAT);
            let depth = tex("depth-sample-depth", PICK_DEPTH_FORMAT);
            {
                let (ids_view, depth_view) = (ids.create_view(&Default::default()), depth.create_view(&Default::default()));
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("depth-samples"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &ids_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_view,
                        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_bind_group(0, self.bind_sample.as_ref().expect("bind groups"), &[]);
                draw_lists(&mut pass, &lists, &last.scene.instances, self.pipelines.as_ref().expect("pipelines"), self.bind_inst.as_ref().expect("bind groups"), DrawMode { pick: true, depth_samples: true, planes_write_depth, ..Default::default() });
            }
            textures.push(ids);
            textures.push(depth);
        }
        (SampleRead { view_proj }, textures)
    }

    /// Block until the oldest outstanding read is ready (no-op if none).
    pub fn wait_oldest(&mut self) {
        let Some(front) = self.pending.front() else { return };
        if front.state.load(Ordering::Acquire) != MAP_PENDING {
            return;
        }
        let r = self.device.poll(wgpu::PollType::Wait { submission_index: Some(front.submission.clone()), timeout: Some(std::time::Duration::from_secs(10)) });
        if let Err(e) = r {
            // Give up on this read; report it as failed.
            let front = self.pending.pop_front().expect("front");
            log::error!("GPU wait failed: {e}");
            front.state.store(MAP_FAILED, Ordering::Release);
            self.finish(front);
        }
    }

    /// Deliver every completed read (in submission order) without blocking.
    pub fn collect(&mut self) -> Vec<Completed> {
        if !self.pending.is_empty() {
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
        while self.pending.front().is_some_and(|r| r.state.load(Ordering::Acquire) != MAP_PENDING) {
            let r = self.pending.pop_front().expect("front");
            self.finish(r);
        }
        if let Some(t) = &mut self.timer {
            t.collect();
        }
        std::mem::take(&mut self.ready)
    }

    fn finish(&mut self, r: PendingRead) {
        let ok = r.state.load(Ordering::Acquire) == MAP_OK;
        match r.kind {
            ReadKind::Texture { seq, texture } => {
                let result = if ok {
                    Ok(Frame { width: texture.width, height: texture.height, bgra: Vec::new(), texture: Some(texture), pick: None, rendered_at: std::time::Instant::now() })
                } else {
                    Err(RenderError::Readback("the frame failed on the GPU".into()))
                };
                if let Some(ts) = self.targets.get_mut(&r.target) {
                    ts.in_flight = ts.in_flight.saturating_sub(1);
                }
                self.ready.push(Completed::Frame { target: r.target, seq, result });
            }
            ReadKind::Frame { seq, width, height, padded_row } => {
                let buffer = r.buffer.expect("frame readback buffer");
                let result = if ok {
                    let _t = perf::scope("r.readback_copy");
                    let row = (width * 4) as usize;
                    let data = buffer.slice(..).get_mapped_range();
                    let mut bgra = Vec::with_capacity(row * height as usize);
                    if padded_row as usize == row {
                        bgra.extend_from_slice(&data[..row * height as usize]);
                    } else {
                        for line in data.chunks_exact(padded_row as usize).take(height as usize) {
                            bgra.extend_from_slice(&line[..row]);
                        }
                    }
                    drop(data);
                    buffer.unmap();
                    perf::record_ms("r.readback_copy_mb", (row * height as usize) as f32 / 1e6);
                    Ok(Frame { width, height, bgra, texture: None, pick: None, rendered_at: std::time::Instant::now() })
                } else {
                    Err(RenderError::Readback("mapping the frame buffer failed".into()))
                };
                if let Some(ts) = self.targets.get_mut(&r.target) {
                    ts.in_flight = ts.in_flight.saturating_sub(1);
                    if ok && ts.tex.as_ref().is_some_and(|t| t.readback_size() == buffer.size()) && ts.spare.len() < MAX_FRAMES_IN_FLIGHT {
                        ts.spare.push(buffer);
                    }
                }
                self.ready.push(Completed::Frame { target: r.target, seq, result });
            }
            ReadKind::Pick { regions } => {
                let buffer = r.buffer.expect("pick readback buffer");
                if ok {
                    let data = buffer.slice(..).get_mapped_range();
                    for g in regions {
                        if let Some(sr) = &g.samples {
                            let block = g.padded_row as usize * g.height as usize;
                            let read = |first: usize| {
                                let mut samples = Vec::new();
                                for y in 0..g.height as usize {
                                    let ids_at = g.offset as usize + block * first + y * g.padded_row as usize;
                                    let depth_at = ids_at + block;
                                    let ids = bytemuck::cast_slice::<u8, u32>(&data[ids_at..ids_at + g.width as usize * 4]);
                                    let depths = bytemuck::cast_slice::<u8, f32>(&data[depth_at..depth_at + g.width as usize * 4]);
                                    for (x, (&id, &depth)) in ids.iter().zip(depths).enumerate() {
                                        // Reverse-Z: cleared to 0 where nothing was drawn.
                                        if depth > 0.0 {
                                            samples.push(DepthSample { x: x as f32 + 0.5, y: y as f32 + 0.5, depth, id });
                                        }
                                    }
                                }
                                samples
                            };
                            let (samples, samples_with_planes) = (read(0), read(2));
                            let result = Ok(PickBuffer { x0: 0, y0: 0, width: 0, height: 0, frame_width: g.frame_width, frame_height: g.frame_height, ids: Vec::new(), samples, samples_with_planes, view_proj: Some(sr.view_proj) });
                            self.ready.push(Completed::Pick { target: r.target, token: g.token, result });
                            continue;
                        }
                        let mut ids = Vec::with_capacity((g.width * g.height) as usize);
                        for y in 0..g.height as usize {
                            let start = g.offset as usize + y * g.padded_row as usize;
                            ids.extend_from_slice(bytemuck::cast_slice::<u8, u32>(&data[start..start + g.width as usize * 4]));
                        }
                        let result = Ok(PickBuffer { x0: g.x0, y0: g.y0, width: g.width, height: g.height, frame_width: g.frame_width, frame_height: g.frame_height, ids, samples: Vec::new(), samples_with_planes: Vec::new(), view_proj: None });
                        self.ready.push(Completed::Pick { target: r.target, token: g.token, result });
                    }
                    drop(data);
                    buffer.unmap();
                    if self.pick_pool.len() < 4 {
                        self.pick_pool.push(buffer);
                    }
                } else {
                    for g in regions {
                        self.ready.push(Completed::Pick { target: r.target, token: g.token, result: Err(RenderError::Readback("mapping the pick buffer failed".into())) });
                    }
                }
            }
        }
    }

    /// Render one frame synchronously (and the whole pick buffer if asked).
    /// Must not be mixed with the asynchronous API on the same renderer while
    /// reads are outstanding.
    pub fn render(&mut self, p: RenderParams) -> Result<Frame, RenderError> {
        let req = FrameRequest {
            scene: Arc::new(p.scene.clone()),
            camera: *p.camera,
            width: p.width,
            height: p.height,
            device_pixel_ratio: p.device_pixel_ratio,
            style: *p.style,
            invalidate_pick: true,
        };
        self.submit_frame(SYNC_TARGET, 0, req)?;
        if p.pick {
            self.submit_picks(SYNC_TARGET, &[(0, PickRect { x: 0, y: 0, width: p.width, height: p.height, depth_samples: 0 })]);
        }
        let (mut frame, mut pick) = (None, None);
        while frame.is_none() || (p.pick && pick.is_none()) {
            if !self.has_pending() {
                break;
            }
            self.wait_oldest();
            for c in self.collect() {
                match c {
                    Completed::Frame { target: SYNC_TARGET, result, .. } => frame = Some(result),
                    Completed::Pick { target: SYNC_TARGET, result, .. } => pick = Some(result),
                    _ => {}
                }
            }
        }
        let mut f = frame.ok_or_else(|| RenderError::Readback("frame lost".into()))??;
        if let Some(pk) = pick {
            f.pick = Some(Arc::new(pk?));
        }
        Ok(f)
    }
}

fn frames_in_flight_for(info: &wgpu::AdapterInfo) -> usize {
    let default = if info.device_type == wgpu::DeviceType::Cpu { 1 } else { MAX_FRAMES_IN_FLIGHT };
    std::env::var("NOSHAPE_FRAMES_IN_FLIGHT").ok().and_then(|v| v.parse::<usize>().ok()).map_or(default, |n| n.clamp(1, MAX_FRAMES_IN_FLIGHT))
}

fn create_present_texture(d: &wgpu::Device, width: u32, height: u32) -> PresentTexture {
    let texture = d.create_texture(&wgpu::TextureDescriptor {
        label: Some("present"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: PRESENT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[COLOR_FORMAT],
    });
    let view = texture.create_view(&Default::default());
    let target_view = texture.create_view(&wgpu::TextureViewDescriptor { format: Some(COLOR_FORMAT), ..Default::default() });
    PresentTexture { texture, view, target_view, width, height }
}

fn create_textures(d: &wgpu::Device, width: u32, height: u32, msaa: u32) -> Textures {
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let tex = |label, format, samples, usage| {
        d.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let att = wgpu::TextureUsages::RENDER_ATTACHMENT;
    let color_msaa = (msaa > 1).then(|| tex("color-msaa", COLOR_FORMAT, msaa, att).create_view(&Default::default()));
    let resolve = tex("color", COLOR_FORMAT, 1, att | wgpu::TextureUsages::COPY_SRC);
    let resolve_view = resolve.create_view(&Default::default());
    // Also sampled by the ambient occlusion pass.
    let depth_tex = tex("depth", depth_format(), msaa, att | wgpu::TextureUsages::TEXTURE_BINDING);
    let depth = depth_tex.create_view(&Default::default());
    let depth_sample = depth_tex.create_view(&wgpu::TextureViewDescriptor { aspect: wgpu::TextureAspect::DepthOnly, ..Default::default() });
    let pick = tex("pick", PICK_FORMAT, 1, att | wgpu::TextureUsages::COPY_SRC);
    let pick_view = pick.create_view(&Default::default());
    let pick_depth = tex("pick-depth", PICK_DEPTH_FORMAT, 1, att).create_view(&Default::default());
    let padded_row = (width * 4).div_ceil(256) * 256;
    Textures { width, height, msaa, color_msaa, resolve, resolve_view, depth, depth_sample, pick, pick_view, pick_depth, padded_row }
}

const SRGB_STEPS: usize = 4096;

/// sRGB byte -> linear.
fn srgb_to_linear_lut() -> &'static [f32; 256] {
    static LUT: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    LUT.get_or_init(|| {
        std::array::from_fn(|i| {
            let c = i as f32 / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        })
    })
}

/// Linear (clamped to 0..1) -> sRGB byte.
fn linear_to_srgb(l: f32) -> u8 {
    static LUT: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    let lut = LUT.get_or_init(|| {
        (0..SRGB_STEPS)
            .map(|i| {
                let l = i as f32 / (SRGB_STEPS - 1) as f32;
                let c = if l <= 0.003_130_8 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
                (c * 255.0).round().clamp(0.0, 255.0) as u8
            })
            .collect()
    });
    lut[(l.clamp(0.0, 1.0) * (SRGB_STEPS - 1) as f32).round() as usize]
}

/// Straight-alpha sRGB RGBA8 -> alpha premultiplied in linear space (still
/// sRGB encoded), so bilinear filtering and mip averaging don't bleed the
/// color of transparent texels into the visible ones.
fn premultiply_rgba(src: &[u8]) -> Vec<u8> {
    let to_linear = srgb_to_linear_lut();
    let mut out = src.to_vec();
    for p in out.as_chunks_mut::<4>().0 {
        if p[3] != 255 {
            let a = p[3] as f32 / 255.0;
            for c in &mut p[..3] {
                *c = linear_to_srgb(to_linear[*c as usize] * a);
            }
        }
    }
    out
}

/// Halve a premultiplied sRGB RGBA8 image (each side to at least 1; for odd
/// sizes the last row/column joins its neighbours' block), averaging in
/// linear space.
fn downsample_rgba(src: &[u8], w: u32, h: u32) -> (Vec<u8>, u32, u32) {
    let to_linear = srgb_to_linear_lut();
    let (w, h) = (w as usize, h as usize);
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    let mut out = vec![0u8; nw * nh * 4];
    for (y, row) in out.chunks_exact_mut(nw * 4).enumerate() {
        let (y0, y1) = (y * h / nh, (y + 1) * h / nh);
        for (x, o) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x0, x1) = (x * w / nw, (x + 1) * w / nw);
            let mut sum = [0f32; 4];
            for sy in y0..y1 {
                for p in src[(sy * w + x0) * 4..(sy * w + x1) * 4].as_chunks::<4>().0 {
                    for (acc, (c, &v)) in sum.iter_mut().zip(p.iter().enumerate()) {
                        *acc += if c < 3 { to_linear[v as usize] } else { v as f32 / 255.0 };
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as f32;
            for (c, (dst, acc)) in o.iter_mut().zip(sum).enumerate() {
                *dst = if c < 3 { linear_to_srgb(acc / n) } else { (acc / n * 255.0).round() as u8 };
            }
        }
    }
    (out, nw as u32, nh as u32)
}

/// Helper to build a transform from a position and rotation (f64 inputs).
pub fn transform(translation: glam::DVec3, rotation: glam::DQuat) -> Mat4 {
    glam::DMat4::from_rotation_translation(rotation, translation).as_mat4()
}

/// Uploads geometry to the GPU. Cheap to clone and usable from any thread.
#[derive(Clone)]
pub struct Uploader {
    device: wgpu::Device,
    /// For texture uploads (staged by wgpu, executed before the render
    /// thread's next submission).
    queue: wgpu::Queue,
    image_layout: wgpu::BindGroupLayout,
    image_sampler: wgpu::Sampler,
}

impl Uploader {
    /// Upload an image as an sRGB texture (alpha premultiplied in linear
    /// space) with a full mip chain built on the CPU. Images larger than the
    /// device's texture limit are downscaled; invalid data (zero size or
    /// short `rgba`) uploads a transparent texel.
    pub fn upload_image(&self, image: &ImageData) -> ImageHandle {
        let (mut w, mut h) = (image.width, image.height);
        let mut level: std::borrow::Cow<[u8]> = if w == 0 || h == 0 || image.rgba.len() < w as usize * h as usize * 4 {
            log::warn!("invalid image data: {}x{} with {} bytes", w, h, image.rgba.len());
            (w, h) = (1, 1);
            vec![0u8; 4].into()
        } else {
            premultiply_rgba(&image.rgba[..w as usize * h as usize * 4]).into()
        };
        let max = self.device.limits().max_texture_dimension_2d.max(1);
        while w > max || h > max {
            let (next, nw, nh) = downsample_rgba(&level, w, h);
            (level, w, h) = (next.into(), nw, nh);
        }
        let mip_level_count = 32 - w.max(h).leading_zeros();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for mip in 0..mip_level_count {
            if mip > 0 {
                let (next, nw, nh) = downsample_rgba(&level, w, h);
                (level, w, h) = (next.into(), nw, nh);
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: mip, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &level,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
        }
        let view = texture.create_view(&Default::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("image"),
            layout: &self.image_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.image_sampler) },
            ],
        });
        // Texture rows run top to bottom: v = 0 is the upper edge.
        let [ll, lr, ur, ul] = image.corners;
        let v = |pos, uv| ImageVertex { pos, uv };
        let quad = [v(ll, [0.0, 1.0]), v(lr, [1.0, 1.0]), v(ur, [1.0, 0.0]), v(ll, [0.0, 1.0]), v(ur, [1.0, 0.0]), v(ul, [0.0, 0.0])];
        let vertices = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("image-quad"),
            contents: bytemuck::cast_slice(&quad),
            usage: wgpu::BufferUsages::VERTEX,
        });
        ImageHandle(Arc::new(GpuImage { vertices, bind_group, bounds: image.bounds() }))
    }

    pub fn upload_mesh(&self, mesh: &MeshData) -> MeshHandle {
        self.upload_mesh_parts(&mesh.positions, &mesh.normals, &mesh.pick_ids, &mesh.indices)
    }

    /// [`Self::upload_mesh`] from borrowed attribute slices (no intermediate
    /// copies): vertices are interleaved straight into the mapped GPU buffer.
    /// Missing normals default to +Z, missing pick ids to 0.
    pub fn upload_mesh_parts(&self, positions: &[[f32; 3]], normals: &[[f32; 3]], pick_ids: &[u32], indices: &[u32]) -> MeshHandle {
        self.upload_mesh_colored(positions, normals, pick_ids, &[], indices)
    }

    /// [`Self::upload_mesh_parts`] with per-vertex sRGB colors, shown by
    /// instances in per-primitive color mode (analysis maps, face colors).
    pub fn upload_mesh_colored(&self, positions: &[[f32; 3]], normals: &[[f32; 3]], pick_ids: &[u32], colors: &[[u8; 4]], indices: &[u32]) -> MeshHandle {
        const STRIDE: usize = std::mem::size_of::<MeshVertex>();
        let n = positions.len();
        let vertices = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh-vertices"),
            size: (n.max(1) * STRIDE) as u64,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: true,
        });
        {
            let mut view = vertices.slice(..).get_mapped_range_mut();
            // Interleave in cache-sized chunks.
            let mut chunk: Vec<MeshVertex> = Vec::with_capacity(4096.min(n));
            let mut start = 0;
            while start < n {
                let end = (start + 4096).min(n);
                chunk.clear();
                chunk.extend((start..end).map(|i| MeshVertex {
                    pos: positions[i],
                    normal: normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]),
                    pick: pick_ids.get(i).copied().unwrap_or(0),
                    color: colors.get(i).copied().unwrap_or([0; 4]),
                }));
                view.slice(start * STRIDE..end * STRIDE).copy_from_slice(bytemuck::cast_slice(&chunk));
                start = end;
            }
        }
        vertices.unmap();
        let indices_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-indices"),
            contents: if indices.is_empty() { &[0u8; 4] } else { bytemuck::cast_slice(indices) },
            usage: wgpu::BufferUsages::INDEX,
        });
        let bounds = crate::scene::bounds_of(positions.iter().copied());
        MeshHandle(Arc::new(GpuMesh { vertices, indices: indices_buf, index_count: indices.len() as u32, bounds }))
    }

    pub fn upload_lines(&self, lines: &LineData) -> LinesHandle {
        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lines"),
            contents: if lines.segments.is_empty() {
                &[0u8; std::mem::size_of::<LineSegment>()]
            } else {
                bytemuck::cast_slice(&lines.segments)
            },
            usage: wgpu::BufferUsages::VERTEX,
        });
        LinesHandle(Arc::new(GpuLines { buffer, count: lines.segments.len() as u32, bounds: lines.bounds() }))
    }

    pub fn upload_points(&self, points: &PointData) -> PointsHandle {
        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("points"),
            contents: if points.points.is_empty() {
                &[0u8; std::mem::size_of::<PointSprite>()]
            } else {
                bytemuck::cast_slice(&points.points)
            },
            usage: wgpu::BufferUsages::VERTEX,
        });
        let bounds = {
            let mut lo = Vec3::splat(f32::INFINITY);
            let mut hi = Vec3::splat(f32::NEG_INFINITY);
            for p in &points.points {
                lo = lo.min(p.position.into());
                hi = hi.max(p.position.into());
            }
            (!points.points.is_empty()).then_some((lo, hi))
        };
        PointsHandle(Arc::new(GpuPoints { buffer, count: points.points.len() as u32, bounds }))
    }
}


/// Instance flag bits of zebra stripes: bit 2 on, bit 3 flipped, bits 8..
/// the stripe count.
fn zebra_bits(z: Option<crate::scene::Zebra>) -> u32 {
    z.map_or(0, |z| 4 | ((z.flip as u32) << 3) | (z.stripes.min(0xff_ffff) << 8))
}

/// The striped room zebra stripes reflect: centered on the scene, sampled
/// a third of its size along each reflected ray.
fn room_of(bounds: Option<(glam::Vec3, glam::Vec3)>) -> [f32; 4] {
    match bounds {
        Some((lo, hi)) => {
            let c = (lo + hi) * 0.5;
            [c.x, c.y, c.z, ((hi - lo).length() * 0.35).max(1e-6)]
        }
        None => [0.0, 0.0, 0.0, 1.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiply_and_downsample_images() {
        // Transparent texels lose their color; opaque ones keep it exactly.
        assert_eq!(premultiply_rgba(&[10, 200, 30, 255, 0, 0, 255, 0]), vec![10, 200, 30, 255, 0, 0, 0, 0]);
        // Half-covered white -> linear 0.5 -> sRGB 188.
        assert_eq!(premultiply_rgba(&[255, 255, 255, 128])[..3], [188, 188, 188]);
        // 3x1 -> 1x1: the odd column joins the block; red averaged with two
        // transparent texels in linear space.
        let (px, w, h) = downsample_rgba(&[255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0], 3, 1);
        assert_eq!((w, h), (1, 1));
        assert_eq!(px[3], 85);
        assert_eq!(px[0], linear_to_srgb(1.0 / 3.0));
        // 5x4 -> 2x2 -> 1x1 -> 1x1, uniform color stays exact.
        let mut img: Vec<u8> = [90u8, 160, 220, 255].repeat(20);
        let (mut w, mut h) = (5, 4);
        for expect in [(2, 2), (1, 1), (1, 1)] {
            (img, w, h) = downsample_rgba(&img, w, h);
            assert_eq!((w, h), expect);
            assert!(img.as_chunks::<4>().0.iter().all(|p| *p == [90, 160, 220, 255]), "{img:?}");
        }
    }
}
