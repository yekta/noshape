//! GPU side of the screen overlays ([`crate::overlay`]): an MSAA UNORM layer
//! the size of the overlay's bounds, composited over the frame.

use crate::overlay::{Mask, OverlayDraw, OverlayVertex, TextShading};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use std::sync::Arc;

/// Format of the layer and of the frame view it is composited into: plain
/// 8-bit, so blending happens on sRGB-encoded values like GPUI's.
const LAYER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
/// GPUI rasterizes paths with 4x MSAA.
const LAYER_SAMPLES: u32 = 4;
/// Layer textures grow in steps of this many pixels (fewer reallocations).
const LAYER_STEP: u32 = 128;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    origin: [f32; 2],
    size: [f32; 2],
    shading: [f32; 4],
    gamma_ratios: [f32; 4],
    flags: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaskVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

/// A target's overlay layer.
pub(crate) struct OverlayLayer {
    width: u32,
    height: u32,
    msaa: wgpu::TextureView,
    resolve: wgpu::TextureView,
    composite: wgpu::BindGroup,
}

/// A growable vertex or index buffer, rewritten every frame.
struct Growable {
    buffer: Option<wgpu::Buffer>,
    usage: wgpu::BufferUsages,
}

impl Growable {
    fn new(usage: wgpu::BufferUsages) -> Self {
        Self { buffer: None, usage: usage | wgpu::BufferUsages::COPY_DST }
    }

    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) -> &wgpu::Buffer {
        let need = (data.len() as u64).max(256).next_power_of_two();
        if self.buffer.as_ref().is_none_or(|b| b.size() < need) {
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor { label: Some("overlay"), size: need, usage: self.usage, mapped_at_creation: false }));
        }
        let buffer = self.buffer.as_ref().expect("buffer");
        queue.write_buffer(buffer, 0, data);
        buffer
    }
}

/// Where an overlay goes: the target's layer and the frame.
pub(crate) struct LayerTarget<'a> {
    pub layer: &'a mut Option<OverlayLayer>,
    /// A `Bgra8Unorm` view of the frame's color.
    pub frame: &'a wgpu::TextureView,
    /// The frame's size in device pixels.
    pub size: (u32, u32),
}

pub(crate) struct OverlayGpu {
    mask_layout: wgpu::BindGroupLayout,
    layer_layout: wgpu::BindGroupLayout,
    fill: wgpu::RenderPipeline,
    mask: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    layer_globals: wgpu::Buffer,
    composite_globals: wgpu::Buffer,
    layer_bind: wgpu::BindGroup,
    composite_bind: wgpu::BindGroup,
    vertices: Growable,
    indices: Growable,
    mask_vertices: Growable,
    /// Uploaded masks by their pixels' address, with the frame they were last used.
    masks: HashMap<usize, (Arc<Vec<u8>>, wgpu::BindGroup, u64)>,
    frame: u64,
}

impl OverlayGpu {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("overlay"), source: wgpu::ShaderSource::Wgsl(include_str!("overlay.wgsl").into()) });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("overlay-globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let mask_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("overlay-mask"),
            entries: &[texture_entry(0), wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None }],
        });
        let layer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("overlay-layer"), entries: &[texture_entry(0)] });
        let layout = |groups: &[Option<&wgpu::BindGroupLayout>]| device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("overlay"), bind_group_layouts: groups, immediate_size: 0 });
        let fill_layout = layout(&[Some(&globals_layout)]);
        let mask_pipeline_layout = layout(&[Some(&globals_layout), Some(&mask_layout)]);
        let composite_layout = layout(&[Some(&globals_layout), Some(&layer_layout)]);
        let premul = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let target = [Some(wgpu::ColorTargetState { format: LAYER_FORMAT, blend: premul, write_mask: wgpu::ColorWrites::ALL })];
        let pipeline = |label, layout: &wgpu::PipelineLayout, vs, fs, buffers: &[wgpu::VertexBufferLayout], topology, samples| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some(vs), compilation_options: Default::default(), buffers },
                primitive: wgpu::PrimitiveState { topology, ..Default::default() },
                depth_stencil: None,
                multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some(fs), compilation_options: Default::default(), targets: &target }),
                multiview_mask: None,
                cache: None,
            })
        };
        let fill_attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4, 2 => Float32x4];
        let fill_buffers = [wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<OverlayVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &fill_attrs }];
        let mask_attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];
        let mask_buffers = [wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<MaskVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &mask_attrs }];
        let fill = pipeline("overlay-fill", &fill_layout, "vs_fill", "fs_fill", &fill_buffers, wgpu::PrimitiveTopology::TriangleList, LAYER_SAMPLES);
        let mask = pipeline("overlay-mask", &mask_pipeline_layout, "vs_mask", "fs_mask", &mask_buffers, wgpu::PrimitiveTopology::TriangleStrip, LAYER_SAMPLES);
        let composite = pipeline("overlay-composite", &composite_layout, "vs_composite", "fs_composite", &[], wgpu::PrimitiveTopology::TriangleStrip, 1);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("overlay-mask"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let uniform = |label| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: std::mem::size_of::<Globals>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let layer_globals = uniform("overlay-layer-globals");
        let composite_globals = uniform("overlay-composite-globals");
        let bind = |buffer: &wgpu::Buffer| device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("overlay-globals"), layout: &globals_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }] });
        let layer_bind = bind(&layer_globals);
        let composite_bind = bind(&composite_globals);
        Self {
            mask_layout,
            layer_layout,
            fill,
            mask,
            composite,
            sampler,
            layer_globals,
            composite_globals,
            layer_bind,
            composite_bind,
            vertices: Growable::new(wgpu::BufferUsages::VERTEX),
            indices: Growable::new(wgpu::BufferUsages::INDEX),
            mask_vertices: Growable::new(wgpu::BufferUsages::VERTEX),
            masks: HashMap::new(),
            frame: 0,
        }
    }

    fn mask_bind_group(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, mask: &Mask) -> wgpu::BindGroup {
        let key = Arc::as_ptr(&mask.alpha) as usize;
        if let Some(entry) = self.masks.get_mut(&key) {
            entry.2 = self.frame;
            return entry.1.clone();
        }
        let size = wgpu::Extent3d { width: mask.width.max(1), height: mask.height.max(1), depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("overlay-mask"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        if mask.width > 0 && mask.height > 0 {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &mask.alpha,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(mask.width), rows_per_image: Some(mask.height) },
                size,
            );
        }
        let view = texture.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("overlay-mask"),
            layout: &self.mask_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) }],
        });
        self.masks.insert(key, (mask.alpha.clone(), bind.clone(), self.frame));
        bind
    }

    fn layer<'a>(&self, device: &wgpu::Device, layer: &'a mut Option<OverlayLayer>, width: u32, height: u32) -> &'a OverlayLayer {
        if layer.as_ref().is_none_or(|l| l.width < width || l.height < height) {
            let (w, h) = (width.div_ceil(LAYER_STEP) * LAYER_STEP, height.div_ceil(LAYER_STEP) * LAYER_STEP);
            let tex = |label, samples, usage| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: LAYER_FORMAT,
                    usage,
                    view_formats: &[],
                })
            };
            let msaa = tex("overlay-msaa", LAYER_SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default());
            let resolve = tex("overlay-layer", 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING).create_view(&Default::default());
            let composite = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("overlay-layer"), layout: &self.layer_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&resolve) }] });
            *layer = Some(OverlayLayer { width: w, height: h, msaa, resolve, composite });
        }
        layer.as_ref().expect("layer")
    }

    /// Draw `draw` and composite it over the target's frame.
    pub(crate) fn encode(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, target: LayerTarget, draw: &OverlayDraw, shading: TextShading) {
        let LayerTarget { layer, frame, size: frame_size } = target;
        let Some(b) = draw.bounds else { return };
        // The layer covers the drawing's bounds (+1 px for antialiasing) within the frame.
        let x0 = (b[0].floor() - 1.0).max(0.0) as u32;
        let y0 = (b[1].floor() - 1.0).max(0.0) as u32;
        let x1 = ((b[2].ceil() + 1.0) as u32).min(frame_size.0);
        let y1 = ((b[3].ceil() + 1.0) as u32).min(frame_size.1);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (w, h) = (x1 - x0, y1 - y0);
        self.frame += 1;
        let frame_no = self.frame;
        // Masks not drawn for a while are released.
        self.masks.retain(|_, m| frame_no - m.2 < 120);
        let mask_binds: Vec<wgpu::BindGroup> = draw.masks.iter().map(|(m, _, _)| self.mask_bind_group(device, queue, m)).collect();
        let layer_size = {
            let l = self.layer(device, layer, w, h);
            (l.width, l.height)
        };
        let shading_v = [if shading.correct { 1.0 } else { 0.0 }, shading.grayscale_enhanced_contrast, w as f32, h as f32];
        let flags = [if shading.clip_path_pixels { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0];
        queue.write_buffer(&self.layer_globals, 0, bytemuck::bytes_of(&Globals { origin: [x0 as f32, y0 as f32], size: [layer_size.0 as f32, layer_size.1 as f32], shading: shading_v, gamma_ratios: shading.gamma_ratios, flags }));
        queue.write_buffer(&self.composite_globals, 0, bytemuck::bytes_of(&Globals { origin: [x0 as f32, y0 as f32], size: [frame_size.0 as f32, frame_size.1 as f32], shading: shading_v, gamma_ratios: shading.gamma_ratios, flags }));
        let vertices = (!draw.indices.is_empty()).then(|| self.vertices.write(device, queue, bytemuck::cast_slice(&draw.vertices)).clone());
        let indices = (!draw.indices.is_empty()).then(|| self.indices.write(device, queue, bytemuck::cast_slice(&draw.indices)).clone());
        let quads: Vec<MaskVertex> = draw
            .masks
            .iter()
            .flat_map(|(m, at, color)| {
                let (x, y) = (at[0] as f32, at[1] as f32);
                let (mw, mh) = (m.width as f32, m.height as f32);
                [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]].map(|[u, v]| MaskVertex { position: [x + u * mw, y + v * mh], uv: [u, v], color: *color })
            })
            .collect();
        let mask_vertices = (!quads.is_empty()).then(|| self.mask_vertices.write(device, queue, bytemuck::cast_slice(&quads)).clone());
        let l = layer.as_ref().expect("layer");
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &l.msaa, depth_slice: None, resolve_target: Some(&l.resolve), ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Discard } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.layer_bind, &[]);
            if let (Some(v), Some(i)) = (&vertices, &indices) {
                pass.set_pipeline(&self.fill);
                pass.set_vertex_buffer(0, v.slice(..));
                pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..draw.indices.len() as u32, 0, 0..1);
            }
            if let Some(v) = &mask_vertices {
                pass.set_pipeline(&self.mask);
                pass.set_vertex_buffer(0, v.slice(..));
                for (k, bind) in mask_binds.iter().enumerate() {
                    pass.set_bind_group(1, bind, &[]);
                    let first = 4 * k as u32;
                    pass.draw(first..first + 4, 0..1);
                }
            }
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("overlay-composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: frame, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.composite);
        pass.set_bind_group(0, &self.composite_bind, &[]);
        pass.set_bind_group(1, &l.composite, &[]);
        pass.draw(0..4, 0..1);
    }
}
