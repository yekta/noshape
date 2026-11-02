//! Screen-space ambient occlusion (see `ao.wgsl`), drawn over the resolved color image when the style asks for it:
//! Onshape's occlusion into a half-size buffer (AMBIENT_OCCLUSION_VIEWPORT_RATIO 0.5), its filter, then the image
//! multiplied by `pow(1 - occlusion, factor)`.

use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use std::sync::Mutex;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AoUniform {
    inv_view_proj: [[f32; 4]; 4],
    params: [f32; 4],
    more: [f32; 4],
}

/// The frame an AO pass darkens.
pub(crate) struct AoFrame<'a> {
    pub view_proj: Mat4,
    pub width: u32,
    pub height: u32,
    /// Graphics preferences "Ambient occlusion", 0..1 (live's 37.5 % is AMBIENT_OCCLUSION_EFFECT_FACTOR 1.5).
    pub strength: f32,
    /// Onshape's depth extent of the scene (its far minus near distance).
    pub extent: f32,
    /// GPU timing of the frame (`NOSHAPE_PERF`).
    pub timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'a>>,
}

/// The half-size occlusion buffers (raw, filtered) for one frame size.
struct Buffers {
    size: (u32, u32),
    raw: wgpu::TextureView,
    filtered: wgpu::TextureView,
}

pub(crate) struct AoPass {
    pub msaa: u32,
    layout: wgpu::BindGroupLayout,
    occlusion: wgpu::RenderPipeline,
    filter: wgpu::RenderPipeline,
    apply: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    buffers: Mutex<Option<Buffers>>,
}

const OCCLUSION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

impl AoPass {
    pub fn new(device: &wgpu::Device, msaa: u32, color_format: wgpu::TextureFormat) -> Self {
        let src = include_str!("ao.wgsl").replace("DEPTH_TYPE", if msaa > 1 { "texture_depth_multisampled_2d" } else { "texture_depth_2d" });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("ao"), source: wgpu::ShaderSource::Wgsl(src.into()) });
        let frag = wgpu::ShaderStages::FRAGMENT;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ao"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: frag, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: frag, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: msaa > 1 }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: frag, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: frag, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("ao"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = |entry: &str, target: wgpu::ColorTargetState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ao"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_ao"), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(entry), targets: &[Some(target)], compilation_options: Default::default() }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let buffer = wgpu::ColorTargetState { format: OCCLUSION_FORMAT, blend: None, write_mask: wgpu::ColorWrites::RED };
        // Multiply the image by the occlusion: dst * src.
        let multiply = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::Src, operation: wgpu::BlendOperation::Add };
        let image = wgpu::ColorTargetState { format: color_format, blend: Some(wgpu::BlendState { color: multiply, alpha: wgpu::BlendComponent::OVER }), write_mask: wgpu::ColorWrites::COLOR };
        let (occlusion, filter, apply) = (pipeline("fs_occlusion", buffer.clone()), pipeline("fs_filter", buffer), pipeline("fs_apply", image));
        let uniform = device.create_buffer(&wgpu::BufferDescriptor { label: Some("ao"), size: std::mem::size_of::<AoUniform>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("ao"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        Self { msaa, layout, occlusion, filter, apply, uniform, sampler, buffers: Mutex::new(None) }
    }

    /// Darken `target` (the resolved color) by the occlusion read from `depth`.
    pub fn encode(&self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, depth: &wgpu::TextureView, target: &wgpu::TextureView, f: AoFrame) {
        let half = (f.width.div_ceil(2).max(1), f.height.div_ceil(2).max(1));
        // The rings' radius: 1 % of the buffer's diagonal, at least 2 px (AMBIENT_OCCLUSION_RADIUS_NEAR / _FAR).
        let radius = (0.01 * (half.0 as f32).hypot(half.1 as f32)).max(2.0);
        let u = AoUniform {
            inv_view_proj: f.view_proj.inverse().to_cols_array_2d(),
            params: [f.width as f32, f.height as f32, half.0 as f32, half.1 as f32],
            more: [radius, f.extent.max(1e-9), 4.0 * f.strength, 0.0],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let mut buffers = self.buffers.lock().unwrap_or_else(|e| e.into_inner());
        if buffers.as_ref().is_none_or(|b| b.size != half) {
            let texture = |label| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d { width: half.0, height: half.1, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: OCCLUSION_FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            *buffers = Some(Buffers { size: half, raw: texture("ao raw"), filtered: texture("ao filtered") });
        }
        let Some(b) = buffers.as_ref() else { return };
        let bind = |occ: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ao"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(occ) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            })
        };
        let mut run = |pipeline: &wgpu::RenderPipeline, input: &wgpu::TextureView, out: &wgpu::TextureView, load: wgpu::LoadOp<wgpu::Color>, timestamp_writes| {
            let group = bind(input);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ao"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: out, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        };
        let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        run(&self.occlusion, &b.filtered, &b.raw, clear, None);
        run(&self.filter, &b.raw, &b.filtered, clear, None);
        run(&self.apply, &b.filtered, target, wgpu::LoadOp::Load, f.timestamp_writes);
    }
}
