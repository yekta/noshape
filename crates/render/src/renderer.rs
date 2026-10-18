//! Offscreen wgpu renderer producing BGRA frames (for GPUI) and a pick buffer.

use crate::camera::{Camera, Projection};
use crate::scene::*;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::sync::Arc;
use wgpu::util::DeviceExt;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
const PICK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Uint;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const INST_STRIDE: u64 = 256;

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
    /// Pick ids, one per pixel (same layout), when requested.
    pub pick: Option<Arc<PickBuffer>>,
}

#[derive(Clone)]
pub struct PickBuffer {
    pub width: u32,
    pub height: u32,
    pub ids: Vec<u32>,
}

impl PickBuffer {
    /// Pick id at pixel, searching a small radius for the nearest non-zero id
    /// so thin edges and points are easy to hit. Prefers lines/points over faces
    /// through the caller-supplied priority function (higher wins at equal distance).
    pub fn pick(&self, x: i32, y: i32, radius: i32, priority: impl Fn(u32) -> i32) -> Option<u32> {
        let mut best: Option<(i32, i32, u32)> = None; // (priority, -dist2, id)
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let (px, py) = (x + dx, y + dy);
                if px < 0 || py < 0 || px >= self.width as i32 || py >= self.height as i32 {
                    continue;
                }
                let d2 = dx * dx + dy * dy;
                if d2 > radius * radius {
                    continue;
                }
                let id = self.ids[(py as u32 * self.width + px as u32) as usize];
                if id == 0 {
                    continue;
                }
                let pr = priority(id);
                // Exact hit on the center pixel only competes by priority if within radius.
                let key = (pr, -d2, id);
                if best.is_none_or(|b| (key.0, key.1) > (b.0, b.1)) {
                    best = Some(key);
                }
            }
        }
        best.map(|b| b.2)
    }
}

struct Targets {
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
    color_readback: wgpu::Buffer,
    pick_readback: wgpu::Buffer,
    padded_row: u32,
}

struct Pipelines {
    background: wgpu::RenderPipeline,
    mesh_opaque: wgpu::RenderPipeline,
    mesh_transparent: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
    point: wgpu::RenderPipeline,
    mesh_pick: wgpu::RenderPipeline,
    mesh_pick_transparent: wgpu::RenderPipeline,
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
    targets: Option<Targets>,
    globals_color: wgpu::Buffer,
    globals_pick: wgpu::Buffer,
    states: wgpu::Buffer,
    states_capacity: u64,
    inst_buffer: wgpu::Buffer,
    inst_capacity: u64,
    bind_color: Option<wgpu::BindGroup>,
    bind_pick: Option<wgpu::BindGroup>,
    bind_inst: Option<wgpu::BindGroup>,
    last_states: Vec<u32>,
}

pub struct RenderParams<'a> {
    pub scene: &'a Scene,
    pub camera: &'a Camera,
    pub width: u32,
    pub height: u32,
    pub device_pixel_ratio: f32,
    pub style: &'a Style,
    /// Also render and read back the pick buffer.
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
            targets: None,
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
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Uint32],
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
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState { count: p.samples, mask: !0, alpha_to_coverage_enabled: false },
                multiview_mask: None,
                cache: None,
            })
        };
        let ge = wgpu::CompareFunction::GreaterEqual;
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        let pipelines = Pipelines {
            background: make(P { label: "background", vs: "vs_background", fs: "fs_background", buffers: &[], format: COLOR_FORMAT, blend: None, depth: Some((false, wgpu::CompareFunction::Always)), samples: msaa }),
            mesh_opaque: make(P { label: "mesh", vs: "vs_mesh", fs: "fs_mesh", buffers: &mesh_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((true, ge)), samples: msaa }),
            mesh_transparent: make(P { label: "mesh-t", vs: "vs_mesh", fs: "fs_mesh", buffers: &mesh_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((false, ge)), samples: msaa }),
            line: make(P { label: "line", vs: "vs_line", fs: "fs_line", buffers: &line_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((true, ge)), samples: msaa }),
            point: make(P { label: "point", vs: "vs_point", fs: "fs_point", buffers: &point_buffers, format: COLOR_FORMAT, blend: alpha, depth: Some((true, ge)), samples: msaa }),
            mesh_pick: make(P { label: "mesh-pick", vs: "vs_mesh", fs: "fs_mesh_pick", buffers: &mesh_buffers, format: PICK_FORMAT, blend: None, depth: Some((true, ge)), samples: 1 }),
            mesh_pick_transparent: make(P { label: "mesh-pick-t", vs: "vs_mesh", fs: "fs_mesh_pick", buffers: &mesh_buffers, format: PICK_FORMAT, blend: None, depth: Some((false, ge)), samples: 1 }),
            line_pick: make(P { label: "line-pick", vs: "vs_line", fs: "fs_line_pick", buffers: &line_buffers, format: PICK_FORMAT, blend: None, depth: Some((true, ge)), samples: 1 }),
            point_pick: make(P { label: "point-pick", vs: "vs_point", fs: "fs_point_pick", buffers: &point_buffers, format: PICK_FORMAT, blend: None, depth: Some((true, ge)), samples: 1 }),
            msaa,
        };
        self.pipelines = Some(pipelines);
    }

    fn ensure_targets(&mut self, width: u32, height: u32, msaa: u32) {
        if self.targets.as_ref().is_some_and(|t| t.width == width && t.height == height && t.msaa == msaa) {
            return;
        }
        let d = &self.device;
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
        let rb = |label| {
            d.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: padded_row as u64 * height as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        };
        self.targets = Some(Targets {
            width,
            height,
            msaa,
            color_msaa,
            resolve,
            resolve_view,
            depth,
            pick,
            pick_view,
            pick_depth,
            color_readback: rb("color-readback"),
            pick_readback: rb("pick-readback"),
            padded_row,
        });
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

    fn upload_states(&mut self, states: &[u32]) {
        let needed = (states.len() as u64).max(1);
        if needed > self.states_capacity {
            self.states_capacity = needed.next_power_of_two();
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
            // Always write the full capacity so stale entries beyond `states.len()` read as 0.
            let mut data = states.to_vec();
            data.resize(self.states_capacity as usize, 0);
            self.queue.write_buffer(&self.states, 0, bytemuck::cast_slice(&data));
            self.last_states = states.to_vec();
        }
    }

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
        }
        let mut bytes = vec![0u8; (scene.instances.len() as u64 * INST_STRIDE) as usize];
        for (i, inst) in scene.instances.iter().enumerate() {
            let normal_m = inst.transform.inverse().transpose();
            let u = InstUniform {
                model: inst.transform.to_cols_array_2d(),
                normal_m: normal_m.to_cols_array_2d(),
                color: inst.color,
                params: [
                    (inst.color_mode == ColorMode::PerPrimitive) as u32,
                    inst.ignore_clip as u32,
                    inst.pickable as u32,
                    inst.pick_base,
                ],
                _pad: [0.0; 24],
            };
            let off = i * INST_STRIDE as usize;
            bytes[off..off + std::mem::size_of::<InstUniform>()].copy_from_slice(bytemuck::bytes_of(&u));
        }
        if !bytes.is_empty() {
            self.queue.write_buffer(&self.inst_buffer, 0, &bytes);
        }
    }

    // ------------------------------------------------------------ render

    pub fn render(&mut self, p: RenderParams) -> Result<Frame, RenderError> {
        let width = p.width.clamp(1, 8192);
        let height = p.height.clamp(1, 8192);
        let msaa = match p.style.msaa {
            0 | 1 => 1,
            2 => 2,
            8 => 8,
            _ => 4,
        };
        self.ensure_pipelines(msaa);
        self.ensure_targets(width, height, msaa);
        self.upload_states(&p.scene.states);
        self.upload_instances(p.scene);
        if self.bind_color.is_none() {
            self.rebuild_bind_groups();
        }

        let scene_radius = p
            .scene
            .bounds()
            .map(|(lo, hi)| {
                let c = (lo + hi) * 0.5;
                ((hi - lo).length() as f64 * 0.5) + (c.as_dvec3() - p.camera.target).length()
            })
            .unwrap_or(1.0);
        let aspect = width as f64 / height as f64;
        let vp = p.camera.view_proj_f32(aspect, scene_radius);
        let eye = p.camera.eye().as_vec3();
        let fwd = p.camera.forward().as_vec3();
        let up = p.camera.up().as_vec3();
        let s = p.style;
        let (clip, clip_on) = match p.scene.clip {
            Some(c) => ([c.normal.x, c.normal.y, c.normal.z, c.d], 1),
            None => ([0.0; 4], 0),
        };
        let mut globals = Globals {
            view_proj: vp.to_cols_array_2d(),
            camera_pos: [eye.x, eye.y, eye.z, (p.camera.projection == Projection::Perspective) as u32 as f32],
            camera_fwd: [fwd.x, fwd.y, fwd.z, 0.0],
            camera_up: [up.x, up.y, up.z, 0.0],
            viewport: [width as f32, height as f32, p.device_pixel_ratio, 0.0],
            clip,
            flags: [clip_on, 0, 0, 0],
            bg_top: s.background_top,
            bg_bottom: s.background_bottom,
            hover: s.hover_color,
            select: s.select_color,
            accent: s.accent_color,
            error: s.error_color,
        };
        self.queue.write_buffer(&self.globals_color, 0, bytemuck::bytes_of(&globals));
        globals.viewport[3] = 8.0 * p.device_pixel_ratio;
        self.queue.write_buffer(&self.globals_pick, 0, bytemuck::bytes_of(&globals));

        // Partition instances into draw lists.
        let mut opaque = Vec::new();
        let mut transparent = Vec::new();
        let mut lines = Vec::new();
        let mut points = Vec::new();
        for (i, inst) in p.scene.instances.iter().enumerate() {
            match &inst.drawable {
                Drawable::Mesh(_) if inst.color[3] >= 0.999 => opaque.push(i),
                Drawable::Mesh(_) => transparent.push(i),
                Drawable::Lines(_) => lines.push(i),
                Drawable::Points(_) => points.push(i),
            }
        }
        let by_layer = |v: &mut Vec<usize>| v.sort_by_key(|&i| (p.scene.instances[i].layer, i));
        by_layer(&mut opaque);
        by_layer(&mut transparent);
        by_layer(&mut lines);
        by_layer(&mut points);

        let pipelines = self.pipelines.as_ref().unwrap();
        let targets = self.targets.as_ref().unwrap();
        let bind_color = self.bind_color.as_ref().unwrap();
        let bind_pick = self.bind_pick.as_ref().unwrap();
        let bind_inst = self.bind_inst.as_ref().unwrap();
        let instances = &p.scene.instances;

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        let draw_lists = |pass: &mut wgpu::RenderPass, pick: bool| {
            let draw = |pass: &mut wgpu::RenderPass, list: &[usize], pipeline: &wgpu::RenderPipeline| {
                pass.set_pipeline(pipeline);
                for &i in list {
                    let inst = &instances[i];
                    if pick && !inst.pickable && !matches!(inst.drawable, Drawable::Mesh(_)) {
                        continue;
                    }
                    if !pick && inst.pick_only {
                        continue;
                    }
                    pass.set_bind_group(1, bind_inst, &[(i as u64 * INST_STRIDE) as u32]);
                    match &inst.drawable {
                        Drawable::Mesh(m) => {
                            if m.0.index_count == 0 {
                                continue;
                            }
                            pass.set_vertex_buffer(0, m.0.vertices.slice(..));
                            pass.set_index_buffer(m.0.indices.slice(..), wgpu::IndexFormat::Uint32);
                            pass.draw_indexed(0..m.0.index_count, 0, 0..1);
                        }
                        Drawable::Lines(l) => {
                            if l.0.count == 0 {
                                continue;
                            }
                            pass.set_vertex_buffer(0, l.0.buffer.slice(..));
                            pass.draw(0..6, 0..l.0.count);
                        }
                        Drawable::Points(pt) => {
                            if pt.0.count == 0 {
                                continue;
                            }
                            pass.set_vertex_buffer(0, pt.0.buffer.slice(..));
                            pass.draw(0..6, 0..pt.0.count);
                        }
                    }
                }
            };
            if pick {
                // Transparent geometry (datum planes, sketch regions) first and
                // without depth writes: opaque parts behind it win the pick.
                draw(pass, &transparent, &pipelines.mesh_pick_transparent);
                draw(pass, &opaque, &pipelines.mesh_pick);
                draw(pass, &lines, &pipelines.line_pick);
                draw(pass, &points, &pipelines.point_pick);
            } else {
                draw(pass, &opaque, &pipelines.mesh_opaque);
                draw(pass, &transparent, &pipelines.mesh_transparent);
                draw(pass, &lines, &pipelines.line);
                draw(pass, &points, &pipelines.point);
            }
        };

        {
            let (view, resolve) = match &targets.color_msaa {
                Some(v) => (v, Some(&targets.resolve_view)),
                None => (&targets.resolve_view, None),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("color"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, bind_color, &[]);
            pass.set_bind_group(1, bind_inst, &[0]);
            pass.set_pipeline(&pipelines.background);
            pass.draw(0..3, 0..1);
            draw_lists(&mut pass, false);
        }
        if p.pick {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pick"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.pick_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.pick_depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, bind_pick, &[]);
            draw_lists(&mut pass, true);
        }

        let copy = |encoder: &mut wgpu::CommandEncoder, tex: &wgpu::Texture, buf: &wgpu::Buffer| {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture: tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo {
                    buffer: buf,
                    layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(targets.padded_row), rows_per_image: Some(height) },
                },
                wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            );
        };
        copy(&mut encoder, &targets.resolve, &targets.color_readback);
        if p.pick {
            copy(&mut encoder, &targets.pick, &targets.pick_readback);
        }
        let submission = self.queue.submit(Some(encoder.finish()));

        let (tx, rx) = std::sync::mpsc::channel();
        let tx2 = tx.clone();
        targets.color_readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        if p.pick {
            targets.pick_readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx2.send(r);
            });
        } else {
            drop(tx2);
        }
        self.device
            .poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: Some(std::time::Duration::from_secs(10)) })
            .map_err(|e| RenderError::Readback(e.to_string()))?;
        for _ in 0..(1 + p.pick as usize) {
            rx.recv()
                .map_err(|e| RenderError::Readback(e.to_string()))?
                .map_err(|e| RenderError::Readback(e.to_string()))?;
        }

        let row = (width * 4) as usize;
        let padded = targets.padded_row as usize;
        let bgra = {
            let data = targets.color_readback.slice(..).get_mapped_range();
            let mut out = Vec::with_capacity(row * height as usize);
            for r in data.chunks_exact(padded).take(height as usize) {
                out.extend_from_slice(&r[..row]);
            }
            out
        };
        targets.color_readback.unmap();
        let pick = if p.pick {
            let data = targets.pick_readback.slice(..).get_mapped_range();
            let mut ids = Vec::with_capacity((width * height) as usize);
            for r in data.chunks_exact(padded).take(height as usize) {
                ids.extend_from_slice(bytemuck::cast_slice::<u8, u32>(&r[..row]));
            }
            drop(data);
            targets.pick_readback.unmap();
            Some(Arc::new(PickBuffer { width, height, ids }))
        } else {
            None
        };
        Ok(Frame { width, height, bgra, pick })
    }
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
        let n = mesh.positions.len();
        let verts: Vec<MeshVertex> = (0..n)
            .map(|i| MeshVertex {
                pos: mesh.positions[i],
                normal: mesh.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]),
                pick: mesh.pick_ids.get(i).copied().unwrap_or(0),
            })
            .collect();
        let vertices = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-vertices"),
            contents: if verts.is_empty() { &[0u8; 28] } else { bytemuck::cast_slice(&verts) },
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-indices"),
            contents: if mesh.indices.is_empty() { &[0u8; 4] } else { bytemuck::cast_slice(&mesh.indices) },
            usage: wgpu::BufferUsages::INDEX,
        });
        MeshHandle(Arc::new(GpuMesh { vertices, indices, index_count: mesh.indices.len() as u32, bounds: mesh.bounds() }))
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
