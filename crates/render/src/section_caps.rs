//! Section caps the way Onshape draws them (its webgl bundle; kern G40):
//!
//! 1. Mask: every kept surface of a cut part adds its part color (sRGB
//!    values, nudged by the part id) to a float texture with additive
//!    blending, front faces + and back faces -, alpha + 16 per part. Where
//!    the plane cuts material the sum is that part's color and count; where
//!    two parts' material overlaps the count is 2 (drawn red).
//! 2. Edges: pixels whose mask differs from 2..6 of their 8 neighbours,
//!    blurred by a 5-tap filter in x then y (8-bit buffers).
//! 3. Caps: a quad on each plane facing away from the viewer, drawn in the
//!    color pass after the opaque faces: the median of three mask samples
//!    in a row picks the color, hatched in the plane's frame
//!    (`fs_section_cap` in shaders.wgsl), with the edges in black.

use bytemuck::{Pod, Zeroable};

/// One plane's cap uniforms (`CapPlane` in shaders.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct CapPlane {
    /// xyz plane point, w half size of the quad.
    pub center: [f32; 4],
    /// xyz tangent, w hatch pattern scale (world units).
    pub u: [f32; 4],
    pub v: [f32; 4],
    pub selector: [f32; 4],
    pub index: [u32; 4],
}

/// Uniform stride of [`CapPlane`]s (the largest offset alignment allowed).
pub(crate) const PLANE_STRIDE: u64 = 256;
const MAX_PLANES: u64 = 3;

/// Onshape's per-plane selector colors (SECTION_PLANE_SELECTOR_COLOR_n),
/// mixed 10% into the cap.
pub(crate) const SELECTOR_COLORS: [[f32; 4]; 3] = [[0.0, 0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]];

pub(crate) struct SectionCaps {
    pub mask_format: wgpu::TextureFormat,
    pub cap_layout: wgpu::BindGroupLayout,
    edge_layout: wgpu::BindGroupLayout,
    edges: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    /// Blur directions x and y.
    params: [wgpu::Buffer; 2],
    planes: wgpu::Buffer,
}

/// A target's mask and edge textures, and the bind groups reading them.
pub(crate) struct CapTextures {
    size: (u32, u32),
    pub mask: wgpu::TextureView,
    edge_a: wgpu::TextureView,
    edge_b: wgpu::TextureView,
    edges_from_mask: wgpu::BindGroup,
    blur_x: wgpu::BindGroup,
    blur_y: wgpu::BindGroup,
    pub cap: wgpu::BindGroup,
}

const EDGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

impl SectionCaps {
    /// Mask in 32-bit floats where they blend (Onshape's), else 16-bit.
    pub fn new(device: &wgpu::Device) -> Self {
        let mask_format = if device.features().contains(wgpu::Features::FLOAT32_BLENDABLE) { wgpu::TextureFormat::Rgba32Float } else { wgpu::TextureFormat::Rgba16Float };
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let uniform = |binding, dynamic| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: dynamic, min_binding_size: None },
            count: None,
        };
        let cap_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("section-cap"), entries: &[uniform(0, true), texture(1), texture(2)] });
        let edge_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("section-cap-edges"), entries: &[texture(0), uniform(1, false)] });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("section-caps"),
            source: wgpu::ShaderSource::Wgsl(include_str!("section_caps.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("section-cap-edges"), bind_group_layouts: &[Some(&edge_layout)], immediate_size: 0 });
        let pipeline = |label, fs| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_full"), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    targets: &[Some(wgpu::ColorTargetState { format: EDGE_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let edges = pipeline("section-cap-edges", "fs_edges");
        let blur = pipeline("section-cap-blur", "fs_blur");
        let params = [[1.0f32, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]].map(|p| {
            use wgpu::util::DeviceExt as _;
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("section-cap-blur"), contents: bytemuck::bytes_of(&p), usage: wgpu::BufferUsages::UNIFORM })
        });
        let planes = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("section-cap-planes"),
            size: PLANE_STRIDE * MAX_PLANES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { mask_format, cap_layout, edge_layout, edges, blur, params, planes }
    }

    /// `current` if it has this size, else new textures.
    pub fn textures(&self, device: &wgpu::Device, current: Option<CapTextures>, width: u32, height: u32) -> CapTextures {
        if let Some(t) = current.filter(|t| t.size == (width, height)) {
            return t;
        }
        let view = |label, format| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let mask = view("section-cap-mask", self.mask_format);
        let edge_a = view("section-cap-edges-a", EDGE_FORMAT);
        let edge_b = view("section-cap-edges-b", EDGE_FORMAT);
        let edge_group = |source: &wgpu::TextureView, params: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("section-cap-edges"),
                layout: &self.edge_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(source) }, wgpu::BindGroupEntry { binding: 1, resource: params.as_entire_binding() }],
            })
        };
        let edges_from_mask = edge_group(&mask, &self.params[0]);
        let blur_x = edge_group(&edge_a, &self.params[0]);
        let blur_y = edge_group(&edge_b, &self.params[1]);
        let cap = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("section-cap"),
            layout: &self.cap_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &self.planes, offset: 0, size: wgpu::BufferSize::new(std::mem::size_of::<CapPlane>() as u64) }),
                },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&mask) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&edge_a) },
            ],
        });
        CapTextures { size: (width, height), mask, edge_a, edge_b, edges_from_mask, blur_x, blur_y, cap }
    }

    pub fn write_planes(&self, queue: &wgpu::Queue, planes: &[CapPlane]) {
        for (i, p) in planes.iter().take(MAX_PLANES as usize).enumerate() {
            queue.write_buffer(&self.planes, i as u64 * PLANE_STRIDE, bytemuck::bytes_of(p));
        }
    }

    /// The outline passes, after the mask pass: edges into A, blur x into
    /// B, blur y back into A (which the caps read).
    pub fn encode_edges(&self, encoder: &mut wgpu::CommandEncoder, t: &CapTextures) {
        for (pipeline, group, target) in [(&self.edges, &t.edges_from_mask, &t.edge_a), (&self.blur, &t.blur_x, &t.edge_b), (&self.blur, &t.blur_y, &t.edge_a)] {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("section-cap-edges"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
