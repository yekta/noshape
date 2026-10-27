//! A hovered whole part's outline drawn on the part side of its image's boundary (see `hover_outline.wgsl`): the
//! color pass's depth, a mask pass of the hovered part's visible faces (`fs_hover_mask`), then a band over the
//! resolved color where the mask meets uncovered pixels.

use bytemuck::{Pod, Zeroable};

/// The mask's format: coverage only.
pub(crate) const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BandUniform {
    color: [f32; 4],
    params: [f32; 4],
}

/// A target's mask (multisampled like the color pass, and resolved).
pub(crate) struct HoverMask {
    size: (u32, u32, u32),
    /// The color pass's sample count > 1: drawn into this, resolved into `resolved`.
    pub msaa: Option<wgpu::TextureView>,
    pub resolved: wgpu::TextureView,
}

pub(crate) struct HoverOutline {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
}

impl HoverOutline {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("hover-outline"), source: wgpu::ShaderSource::Wgsl(include_str!("hover_outline.wgsl").into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hover-outline"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("hover-outline"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hover-outline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_hover_outline"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_hover_outline"),
                targets: &[Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::COLOR })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor { label: Some("hover-outline"), size: std::mem::size_of::<BandUniform>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        Self { layout, pipeline, uniform }
    }

    /// The mask textures for a `width` × `height` target with `msaa` samples (`prev` when it still fits).
    pub fn mask(device: &wgpu::Device, prev: Option<HoverMask>, width: u32, height: u32, msaa: u32) -> HoverMask {
        if let Some(m) = prev.filter(|m| m.size == (width, height, msaa)) {
            return m;
        }
        let tex = |label, samples, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: MASK_FORMAT,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let msaa_view = (msaa > 1).then(|| tex("hover-mask-msaa", msaa, wgpu::TextureUsages::RENDER_ATTACHMENT));
        let resolved = tex("hover-mask", 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
        HoverMask { size: (width, height, msaa), msaa: msaa_view, resolved }
    }

    /// The band over `target` (the resolved color): `color` sRGB, `radius` device px.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(&self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, mask: &HoverMask, target: &wgpu::TextureView, color: [f32; 3], radius: f32) {
        let (width, height, _) = mask.size;
        let u = BandUniform { color: [color[0], color[1], color[2], 1.0], params: [width as f32, height as f32, radius, 0.0] };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hover-outline"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&mask.resolved) }],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hover-outline"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.draw(0..3, 0..1);
    }
}
