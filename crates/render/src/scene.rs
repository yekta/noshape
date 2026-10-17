//! CPU-side scene description handed to the renderer.
//!
//! Geometry is uploaded once into GPU buffers (`MeshHandle`, `LinesHandle`,
//! `PointsHandle`) and referenced by lightweight per-frame `Instance`s. Every
//! primitive carries a `pick_id`: a dense `u32` chosen by the application
//! (0 = not pickable). Hover/selection/visibility are per-pick-id flags stored
//! in a GPU storage buffer, so changing highlight state never re-uploads
//! geometry.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::sync::Arc;

/// Triangle mesh. Vertices are not shared across B-rep faces so every vertex
/// carries the pick id of its face.
#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub pick_ids: Vec<u32>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        bounds_of(self.positions.iter().copied())
    }
}

/// A set of line segments drawn as screen-space quads of constant pixel width.
#[derive(Clone, Debug, Default)]
pub struct LineData {
    pub segments: Vec<LineSegment>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct LineSegment {
    pub a: [f32; 3],
    pub b: [f32; 3],
    /// RGBA8 color (only used when the instance's color mode is per-primitive).
    pub color: [u8; 4],
    pub pick_id: u32,
    /// Width in logical pixels.
    pub width: f32,
    /// Bit 0: dashed. Bit 1: always on top (ignore depth).
    pub style: u32,
}

impl LineSegment {
    pub const DASHED: u32 = 1;
    pub const ON_TOP: u32 = 2;
}

impl LineData {
    /// Append a polyline as consecutive segments.
    pub fn push_polyline(&mut self, pts: &[[f32; 3]], color: [u8; 4], pick_id: u32, width: f32, style: u32) {
        for w in pts.windows(2) {
            self.segments.push(LineSegment { a: w[0], b: w[1], color, pick_id, width, style });
        }
    }
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        bounds_of(self.segments.iter().flat_map(|s| [s.a, s.b]))
    }
}

/// Screen-space round points (vertices, sketch points).
#[derive(Clone, Debug, Default)]
pub struct PointData {
    pub points: Vec<PointSprite>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct PointSprite {
    pub position: [f32; 3],
    pub color: [u8; 4],
    pub pick_id: u32,
    /// Diameter in logical pixels.
    pub size: f32,
    /// Bit 1: always on top.
    pub style: u32,
    pub _pad: u32,
}

fn bounds_of(it: impl Iterator<Item = [f32; 3]>) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut any = false;
    for p in it {
        let p = Vec3::from(p);
        min = min.min(p);
        max = max.max(p);
        any = true;
    }
    any.then_some((min, max))
}

/// Opaque handle to geometry uploaded to the GPU. Cheap to clone; GPU memory is
/// released when the last handle is dropped.
#[derive(Clone)]
pub struct MeshHandle(pub(crate) Arc<crate::renderer::GpuMesh>);
#[derive(Clone)]
pub struct LinesHandle(pub(crate) Arc<crate::renderer::GpuLines>);
#[derive(Clone)]
pub struct PointsHandle(pub(crate) Arc<crate::renderer::GpuPoints>);

impl MeshHandle {
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        self.0.bounds
    }
}
impl LinesHandle {
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        self.0.bounds
    }
}
impl PointsHandle {
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        self.0.bounds
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorMode {
    /// Use the instance color for everything.
    Instance,
    /// Use per-primitive colors (lines/points only).
    PerPrimitive,
}

#[derive(Clone)]
pub enum Drawable {
    Mesh(MeshHandle),
    Lines(LinesHandle),
    Points(PointsHandle),
}

#[derive(Clone)]
pub struct Instance {
    pub drawable: Drawable,
    pub transform: Mat4,
    /// Linear-space RGBA; alpha < 1 renders in the transparent pass.
    pub color: [f32; 4],
    pub color_mode: ColorMode,
    /// Participates in the pick buffer.
    pub pickable: bool,
    /// Excluded from section-view clipping (e.g. sketch overlays, planes).
    pub ignore_clip: bool,
    /// Draw order within its pass (lower first). Overlays use larger values.
    pub layer: i32,
}

impl Instance {
    pub fn new(drawable: Drawable) -> Self {
        Self {
            drawable,
            transform: Mat4::IDENTITY,
            color: [0.7, 0.72, 0.76, 1.0],
            color_mode: ColorMode::Instance,
            pickable: true,
            ignore_clip: false,
            layer: 0,
        }
    }
    pub fn with_color(mut self, c: [f32; 4]) -> Self {
        self.color = c;
        self
    }
    pub fn with_transform(mut self, t: Mat4) -> Self {
        self.transform = t;
        self
    }
    pub fn per_primitive_color(mut self) -> Self {
        self.color_mode = ColorMode::PerPrimitive;
        self
    }
    pub fn layer(mut self, layer: i32) -> Self {
        self.layer = layer;
        self
    }
    pub fn unpickable(mut self) -> Self {
        self.pickable = false;
        self
    }
    pub fn ignore_clip(mut self) -> Self {
        self.ignore_clip = true;
        self
    }
}

/// Per-pick-id state flags (bitfield written to the GPU state buffer).
pub mod state {
    pub const HOVER: u32 = 1;
    pub const SELECTED: u32 = 2;
    pub const HIDDEN: u32 = 4;
    /// Secondary highlight, e.g. "related to hovered feature" or preview.
    pub const ACCENT: u32 = 8;
    /// Error highlight (failed references).
    pub const ERROR: u32 = 16;
    /// Dimmed (e.g. other parts while editing in context / rollback).
    pub const DIM: u32 = 32;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub background_top: [f32; 4],
    pub background_bottom: [f32; 4],
    pub hover_color: [f32; 4],
    pub select_color: [f32; 4],
    pub accent_color: [f32; 4],
    pub error_color: [f32; 4],
    pub edge_color: [f32; 4],
    /// Draw B-rep edges on shaded bodies.
    pub show_edges: bool,
    /// 1, 2, 4 or 8.
    pub msaa: u32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background_top: srgb(0x2b, 0x30, 0x3a),
            background_bottom: srgb(0x16, 0x19, 0x1f),
            hover_color: srgb(0x4c, 0xc9, 0xf0),
            select_color: srgb(0xff, 0xb0, 0x20),
            accent_color: srgb(0x9d, 0x7b, 0xff),
            error_color: srgb(0xff, 0x45, 0x45),
            edge_color: srgb(0x0e, 0x10, 0x14),
            show_edges: true,
            msaa: 4,
        }
    }
}

/// Convert sRGB bytes to linear RGBA.
pub fn srgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    fn lin(c: u8) -> f32 {
        let c = c as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    }
    [lin(r), lin(g), lin(b), 1.0]
}

/// Section view clipping plane: geometry with dot(n, p) > d is removed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipPlane {
    pub normal: Vec3,
    pub d: f32,
}

#[derive(Clone, Default)]
pub struct Scene {
    pub instances: Vec<Instance>,
    /// Indexed by pick id; see [`state`].
    pub states: Vec<u32>,
    pub clip: Option<ClipPlane>,
}

impl Scene {
    pub fn set_state(&mut self, pick_id: u32, flags: u32) {
        let i = pick_id as usize;
        if self.states.len() <= i {
            self.states.resize(i + 1, 0);
        }
        self.states[i] = flags;
    }
    pub fn add_flag(&mut self, pick_id: u32, flag: u32) {
        let i = pick_id as usize;
        if self.states.len() <= i {
            self.states.resize(i + 1, 0);
        }
        self.states[i] |= flag;
    }
    pub fn clear_flag_all(&mut self, flag: u32) {
        for s in &mut self.states {
            *s &= !flag;
        }
    }

    /// World-space bounds of all instances.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut acc: Option<(Vec3, Vec3)> = None;
        for inst in &self.instances {
            let b = match &inst.drawable {
                Drawable::Mesh(m) => m.bounds(),
                Drawable::Lines(l) => l.bounds(),
                Drawable::Points(p) => p.bounds(),
            };
            if let Some((lo, hi)) = b {
                for i in 0..8 {
                    let c = Vec3::new(
                        if i & 1 == 0 { lo.x } else { hi.x },
                        if i & 2 == 0 { lo.y } else { hi.y },
                        if i & 4 == 0 { lo.z } else { hi.z },
                    );
                    let w = inst.transform.transform_point3(c);
                    acc = Some(match acc {
                        None => (w, w),
                        Some((a, b)) => (a.min(w), b.max(w)),
                    });
                }
            }
        }
        acc
    }
}
