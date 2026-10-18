//! Screen-space ambient occlusion pass (see `ao.wgsl`), drawn over the
//! resolved color image when the style asks for it.

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AoUniform {
    inv_view_proj: [[f32; 4]; 4],
    params: [f32; 4],
}

/// The frame an AO pass darkens.
pub(crate) struct AoFrame {
    pub view_proj: Mat4,
    pub width: u32,
    pub height: u32,
    pub dpr: f32,
    /// Graphics preferences "Ambient occlusion", 0..1.
    pub strength: f32,
}

pub(crate) struct AoPass {
    pub msaa: u32,
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
}

impl AoPass {
    pub fn new(device: &wgpu::Device, msaa: u32, color_format: wgpu::TextureFormat) -> Self {
        let src = include_str!("ao.wgsl").replace("DEPTH_TYPE", if msaa > 1 { "texture_depth_multisampled_2d" } else { "texture_depth_2d" });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("ao"), source: wgpu::ShaderSource::Wgsl(src.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ao"),
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
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: msaa > 1 },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("ao"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        // Multiply the image by the occlusion: dst * src.
        let multiply = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::Src, operation: wgpu::BlendOperation::Add };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ao"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_ao"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_ao"),
                targets: &[Some(wgpu::ColorTargetState { format: color_format, blend: Some(wgpu::BlendState { color: multiply, alpha: wgpu::BlendComponent::OVER }), write_mask: wgpu::ColorWrites::COLOR })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor { label: Some("ao"), size: std::mem::size_of::<AoUniform>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        Self { msaa, layout, pipeline, uniform }
    }

    /// Darken `target` (the resolved color) by the occlusion read from `depth`.
    pub fn encode(&self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, depth: &wgpu::TextureView, target: &wgpu::TextureView, f: AoFrame) {
        let u = AoUniform { inv_view_proj: f.view_proj.inverse().to_cols_array_2d(), params: [f.width as f32, f.height as f32, f.dpr, f.strength] };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ao"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth) }],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ao"),
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
