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
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
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
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstUniform {
    model: [[f32; 4]; 4],
    normal_m: [[f32; 4]; 4],
    color: [f32; 4],
    params: [u32; 4],
    _pad: [f32; 24],
}

/// A rendered frame. `bgra` is tightly packed sRGB BGRA8, row-major, top row first.
#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
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
                let key = (priority(id), -d2, id);
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl PickRect {
    /// Square of `2 * half + 1` pixels centred on a pixel.
    pub fn around(x: i32, y: i32, half: i32) -> Self {
        let half = half.max(0);
        Self { x: x - half, y: y - half, width: (2 * half + 1) as u32, height: (2 * half + 1) as u32 }
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
    /// Free color readback buffers of the current size.
    spare: Vec<wgpu::Buffer>,
    /// Parameters of the newest submitted frame (for lazy pick rendering).
    last: Option<FrameState>,
    /// The pick texture matches `last`.
    pick_valid: bool,
    in_flight: usize,
    stamp: u64,
}

struct RegionRead {
    token: u64,
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
    Pick { regions: Vec<RegionRead> },
}

struct PendingRead {
    target: u64,
    kind: ReadKind,
    buffer: wgpu::Buffer,
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
    lines: Vec<usize>,
    points: Vec<usize>,
    culled: usize,
}

impl DrawLists {
    /// Partition the scene's visible instances by pass, sorted by layer.
    fn build(&mut self, scene: &Scene, view_proj: &Mat4, width: u32, height: u32, dpr: f32, pick: bool) {
        self.opaque.clear();
        self.transparent.clear();
        self.pick_priority.clear();
        self.lines.clear();
        self.points.clear();
        self.culled = 0;
        // Screen-space margin for wide lines, points and the pick widening.
        let margin = |px: f32| (2.0 * px * dpr / width as f32, 2.0 * px * dpr / height as f32);
        let mesh_margin = margin(2.0);
        let sprite_margin = margin(24.0);
        for (i, inst) in scene.instances.iter().enumerate() {
            if pick {
                if !inst.pickable && !matches!(inst.drawable, Drawable::Mesh(_)) {
                    continue;
                }
            } else if inst.pick_only {
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
            };
            if let Some(b) = bounds
                && outside_frustum(b, &(*view_proj * inst.transform), m)
            {
                self.culled += 1;
                continue;
            }
            match &inst.drawable {
                Drawable::Mesh(_) if pick && inst.pick_only => self.pick_priority.push(i),
                Drawable::Mesh(_) if inst.color[3] >= 0.999 => self.opaque.push(i),
                Drawable::Mesh(_) => self.transparent.push(i),
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
            }
        }
        let inst = &scene.instances;
        for v in [&mut self.opaque, &mut self.transparent, &mut self.pick_priority, &mut self.lines, &mut self.points] {
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

fn draw_lists(pass: &mut wgpu::RenderPass, lists: &DrawLists, instances: &[Instance], pipelines: &Pipelines, bind_inst: &wgpu::BindGroup, pick: bool, section: bool) {
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
            }
        }
    };
    if pick {
        // Transparent geometry (datum planes) first and without depth
        // writes: opaque parts behind it win the pick.
        draw(pass, &lists.transparent, &pipelines.mesh_pick_transparent);
        draw(pass, &lists.opaque, &pipelines.mesh_pick);
        // Pick-only meshes (e.g. sketch regions drawn on a part face) after
        // the faces: opaque faces are pushed back by their depth bias, so a
        // region lying exactly on a face wins even though the two
        // triangulations differ; parts in front of a region still win.
        draw(pass, &lists.pick_priority, &pipelines.mesh_pick_priority);
        draw(pass, &lists.lines, &pipelines.line_pick);
        draw(pass, &lists.points, &pipelines.point_pick);
    } else {
        draw(pass, &lists.opaque, if section { &pipelines.mesh_section } else { &pipelines.mesh_opaque });
        draw(pass, &lists.transparent, &pipelines.mesh_transparent);
        draw(pass, &lists.lines, &pipelines.line);
        draw(pass, &lists.points, &pipelines.point);
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
    let scene_radius = scene
        .bounds()
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
        error: s.error_color,
    }
}

struct Pipelines {
    background: wgpu::RenderPipeline,
    mesh_opaque: wgpu::RenderPipeline,
    /// Opaque meshes while a section view cuts: draws the caps.
    mesh_section: wgpu::RenderPipeline,
    mesh_transparent: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
    point: wgpu::RenderPipeline,
    mesh_pick: wgpu::RenderPipeline,
    mesh_pick_transparent: wgpu::RenderPipeline,
    mesh_pick_priority: wgpu::RenderPipeline,
    line_pick: wgpu::RenderPipeline,
    point_pick: wgpu::RenderPipeline,
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
    pipelines: Option<Pipelines>,
    globals_color: wgpu::Buffer,
    globals_pick: wgpu::Buffer,
    states: wgpu::Buffer,
    states_capacity: u64,
    inst_buffer: wgpu::Buffer,
    inst_capacity: u64,
    bind_color: Option<wgpu::BindGroup>,
    bind_pick: Option<wgpu::BindGroup>,
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
        let adapter_info = adapter.get_info();
        let frames_in_flight = frames_in_flight_for(&adapter_info);
        log::info!("noshape renderer using {:?} ({:?})", adapter_info.name, adapter_info.backend);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("noshape"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| RenderError::Device(e.to_string()))?;

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

        Ok(Self {
            device,
            queue,
            adapter_info,
            shader,
            globals_layout,
            inst_layout,
            pipeline_layout,
            pipelines: None,
            globals_color,
            globals_pick,
            states,
            states_capacity,
            inst_buffer,
            inst_capacity,
            bind_color: None,
            bind_pick: None,
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
        })
    }

    pub fn adapter_name(&self) -> String {
        format!("{} ({:?})", self.adapter_info.name, self.adapter_info.backend)
    }

    /// A cloneable handle for uploading geometry from any thread.
    pub fn uploader(&self) -> Uploader {
        Uploader { device: self.device.clone() }
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

    // ------------------------------------------------------------ setup

    fn ensure_pipelines(&mut self, msaa: u32) {
        if self.pipelines.as_ref().is_some_and(|p| p.msaa == msaa) {
            return;
        }
        let d = &self.device;
        let sh = &self.shader;
        let layout = &self.pipeline_layout;

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
        let make = |p: P| {
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
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: p.depth.map(|(write, cmp)| wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(cmp),
                    stencil: Default::default(),
                    bias: p.bias,
                }),
                multisample: wgpu::MultisampleState { count: p.samples, mask: !0, alpha_to_coverage_enabled: false },
                multiview_mask: None,
                cache: None,
            })
        };
        let ge = wgpu::CompareFunction::GreaterEqual;
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        let none = wgpu::DepthBiasState::default();
        let (face, priority) = (FACE_DEPTH_BIAS, PICK_PRIORITY_DEPTH_BIAS);
        let pipelines = Pipelines {
            background: make(P { label: "background", vs: "vs_background", fs: "fs_background", buffers: &[], format: COLOR_FORMAT, blend: None, depth: Some((false, wgpu::CompareFunction::Always)), bias: none, samples: msaa }),
            mesh_opaque: make(P { label: "mesh", vs: "vs_mesh", fs: "fs_mesh", buffers: &mesh_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((true, ge)), bias: face, samples: msaa }),
            mesh_section: make(P { label: "mesh-section", vs: "vs_mesh", fs: "fs_mesh_section", buffers: &mesh_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((true, ge)), bias: face, samples: msaa }),
            mesh_transparent: make(P { label: "mesh-t", vs: "vs_mesh", fs: "fs_mesh", buffers: &mesh_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((false, ge)), bias: none, samples: msaa }),
            line: make(P { label: "line", vs: "vs_line", fs: "fs_line", buffers: &line_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((true, ge)), bias: none, samples: msaa }),
            point: make(P { label: "point", vs: "vs_point", fs: "fs_point", buffers: &point_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((true, ge)), bias: none, samples: msaa }),
            mesh_pick: make(P { label: "mesh-pick", vs: "vs_mesh", fs: "fs_mesh_pick", buffers: &mesh_buffers, format: PICK_FORMAT, blend: None, depth: Some((true, ge)), bias: face, samples: 1 }),
            mesh_pick_transparent: make(P { label: "mesh-pick-t", vs: "vs_mesh", fs: "fs_mesh_pick", buffers: &mesh_buffers, format: PICK_FORMAT, blend: None, depth: Some((false, ge)), bias: none, samples: 1 }),
            mesh_pick_priority: make(P { label: "mesh-pick-p", vs: "vs_mesh", fs: "fs_mesh_pick", buffers: &mesh_buffers, format: PICK_FORMAT, blend: None, depth: Some((true, ge)), bias: priority, samples: 1 }),
            line_pick: make(P { label: "line-pick", vs: "vs_line", fs: "fs_line_pick", buffers: &line_buffers, format: PICK_FORMAT, blend: None, depth: Some((true, ge)), bias: none, samples: 1 }),
            point_pick: make(P { label: "point-pick", vs: "vs_point", fs: "fs_point_pick", buffers: &point_buffers, format: PICK_FORMAT, blend: None, depth: Some((true, ge)), bias: none, samples: 1 }),
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
                params: [(inst.color_mode == ColorMode::PerPrimitive) as u32, inst.ignore_clip as u32 | ((inst.unlit as u32) << 1), inst.pickable as u32, inst.pick_base],
                _pad: [0.0; 24],
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
        ts
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
    pub fn can_submit(&self, target: u64) -> bool {
        self.targets.get(&target).is_none_or(|t| t.in_flight < self.frames_in_flight)
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
        drop(t_prep);

        let t_encode = perf::scope("r.encode_submit");
        let Self { device, queue, pipelines, targets, bind_color, bind_inst, pending, .. } = self;
        let pipelines = pipelines.as_ref().expect("pipelines");
        let ts = targets.get_mut(&target).expect("target");
        if req.invalidate_pick {
            ts.pick_valid = false;
        }
        let tex = ts.tex.as_ref().expect("textures");
        let readback = ts.spare.pop().unwrap_or_else(|| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("color-readback"),
                size: tex.readback_size(),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let (view, resolve) = match &tex.color_msaa {
                Some(v) => (v, Some(&tex.resolve_view)),
                None => (&tex.resolve_view, None),
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
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let bind_inst = bind_inst.as_ref().expect("bind groups");
            pass.set_bind_group(0, bind_color.as_ref().expect("bind groups"), &[]);
            pass.set_bind_group(1, bind_inst, &[0]);
            pass.set_pipeline(&pipelines.background);
            pass.draw(0..3, 0..1);
            draw_lists(&mut pass, &lists, &req.scene.instances, pipelines, bind_inst, false, req.scene.clip.is_some());
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &tex.resolve, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(tex.padded_row), rows_per_image: Some(height) } },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        let submission = queue.submit(Some(encoder.finish()));
        let state = Arc::new(AtomicU8::new(MAP_PENDING));
        let st = state.clone();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |r| st.store(if r.is_ok() { MAP_OK } else { MAP_FAILED }, Ordering::Release));
        pending.push_back(PendingRead { target, kind: ReadKind::Frame { seq, width, height, padded_row: tex.padded_row }, buffer: readback, submission, state });
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
                draw_lists(&mut pass, &lists, &last.scene.instances, self.pipelines.as_ref().expect("pipelines"), self.bind_inst.as_ref().expect("bind groups"), true, false);
            }
            self.lists = lists;
            self.targets.get_mut(&target).expect("target").pick_valid = true;
        }
        // Lay the rectangles out in one readback buffer.
        let mut regions = Vec::with_capacity(queries.len());
        let mut size = 0u64;
        for &(token, rect) in queries {
            let (x0, y0, w, h) = rect.clip(width, height);
            let padded_row = (w * 4).div_ceil(256).max(1) * 256;
            regions.push(RegionRead { token, x0, y0, width: w, height: h, frame_width: width, frame_height: height, offset: size, padded_row });
            if w > 0 && h > 0 {
                size += padded_row as u64 * h as u64;
            }
        }
        if size == 0 {
            for r in regions {
                self.ready.push(Completed::Pick { target, token: r.token, result: Ok(PickBuffer { x0: r.x0, y0: r.y0, width: 0, height: 0, frame_width: width, frame_height: height, ids: Vec::new() }) });
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
        self.pending.push_back(PendingRead { target, kind: ReadKind::Pick { regions }, buffer, submission, state });
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
        std::mem::take(&mut self.ready)
    }

    fn finish(&mut self, r: PendingRead) {
        let ok = r.state.load(Ordering::Acquire) == MAP_OK;
        match r.kind {
            ReadKind::Frame { seq, width, height, padded_row } => {
                let result = if ok {
                    let _t = perf::scope("r.readback_copy");
                    let row = (width * 4) as usize;
                    let data = r.buffer.slice(..).get_mapped_range();
                    let mut bgra = Vec::with_capacity(row * height as usize);
                    if padded_row as usize == row {
                        bgra.extend_from_slice(&data[..row * height as usize]);
                    } else {
                        for line in data.chunks_exact(padded_row as usize).take(height as usize) {
                            bgra.extend_from_slice(&line[..row]);
                        }
                    }
                    drop(data);
                    r.buffer.unmap();
                    Ok(Frame { width, height, bgra, pick: None, rendered_at: std::time::Instant::now() })
                } else {
                    Err(RenderError::Readback("mapping the frame buffer failed".into()))
                };
                if let Some(ts) = self.targets.get_mut(&r.target) {
                    ts.in_flight = ts.in_flight.saturating_sub(1);
                    if ok && ts.tex.as_ref().is_some_and(|t| t.readback_size() == r.buffer.size()) && ts.spare.len() < MAX_FRAMES_IN_FLIGHT {
                        ts.spare.push(r.buffer);
                    }
                }
                self.ready.push(Completed::Frame { target: r.target, seq, result });
            }
            ReadKind::Pick { regions } => {
                if ok {
                    let data = r.buffer.slice(..).get_mapped_range();
                    for g in regions {
                        let mut ids = Vec::with_capacity((g.width * g.height) as usize);
                        for y in 0..g.height as usize {
                            let start = g.offset as usize + y * g.padded_row as usize;
                            ids.extend_from_slice(bytemuck::cast_slice::<u8, u32>(&data[start..start + g.width as usize * 4]));
                        }
                        let result = Ok(PickBuffer { x0: g.x0, y0: g.y0, width: g.width, height: g.height, frame_width: g.frame_width, frame_height: g.frame_height, ids });
                        self.ready.push(Completed::Pick { target: r.target, token: g.token, result });
                    }
                    drop(data);
                    r.buffer.unmap();
                    if self.pick_pool.len() < 4 {
                        self.pick_pool.push(r.buffer);
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
            self.submit_picks(SYNC_TARGET, &[(0, PickRect { x: 0, y: 0, width: p.width, height: p.height })]);
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
    let depth = tex("depth", DEPTH_FORMAT, msaa, att).create_view(&Default::default());
    let pick = tex("pick", PICK_FORMAT, 1, att | wgpu::TextureUsages::COPY_SRC);
    let pick_view = pick.create_view(&Default::default());
    let pick_depth = tex("pick-depth", DEPTH_FORMAT, 1, att).create_view(&Default::default());
    let padded_row = (width * 4).div_ceil(256) * 256;
    Textures { width, height, msaa, color_msaa, resolve, resolve_view, depth, pick, pick_view, pick_depth, padded_row }
}

/// Helper to build a transform from a position and rotation (f64 inputs).
pub fn transform(translation: glam::DVec3, rotation: glam::DQuat) -> Mat4 {
    glam::DMat4::from_rotation_translation(rotation, translation).as_mat4()
}

/// Uploads geometry to the GPU. Cheap to clone and usable from any thread.
#[derive(Clone)]
pub struct Uploader {
    device: wgpu::Device,
}

impl Uploader {

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
