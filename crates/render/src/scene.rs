//! CPU-side scene description handed to the renderer.
//!
//! Geometry is uploaded once into GPU buffers (`MeshHandle`, `LinesHandle`,
//! `PointsHandle`, `ImageHandle`) and referenced by lightweight per-frame `Instance`s. Every
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
    /// Dash-dot (phantom) pattern.
    pub const DASH_DOT: u32 = 4;
    /// Construction line: the dash / gap / dash / gap pattern of
    /// [`Style::construction_dash`] (Onshape's Graphics preferences).
    pub const CONSTRUCTION: u32 = 8;
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
    /// Bit 1: always on top. Bit 2: ring with a center dot. Bit 3
    /// ([`PointSprite::HOLLOW`]): white center inside a colored ring.
    pub style: u32,
    pub _pad: u32,
}

/// A textured quad (e.g. an image inserted in a sketch), drawn depth-tested
/// like other geometry and never pickable.
#[derive(Clone, Debug, Default)]
pub struct ImageData {
    /// Lower-left, lower-right, upper-right, upper-left corner in local
    /// space. The quad is split into two triangles along the lower-left /
    /// upper-right diagonal, so the mapping is exact for parallelograms.
    pub corners: [[f32; 3]; 4],
    pub width: u32,
    pub height: u32,
    /// sRGB RGBA8 (straight alpha), `width * height * 4` bytes, row 0 = top
    /// row of the image.
    pub rgba: Vec<u8>,
}

impl ImageData {
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        bounds_of(self.corners.iter().copied())
    }
}

impl PointSprite {
    /// A hollow point: white center, ring in the point color.
    pub const HOLLOW: u32 = 8;
}

pub(crate) fn bounds_of(it: impl Iterator<Item = [f32; 3]>) -> Option<(Vec3, Vec3)> {
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
/// Texture and quad of an uploaded [`ImageData`].
#[derive(Clone)]
pub struct ImageHandle(pub(crate) Arc<crate::renderer::GpuImage>);

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
impl ImageHandle {
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
    /// Textured quad, multiplied by the instance color (alpha fades it).
    /// Never pickable; drawn after opaque meshes, before transparent ones.
    Image(ImageHandle),
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
    /// Drawn with its flat color, without lighting (construction planes).
    pub unlit: bool,
    /// Lit like Onshape's feature previews (see [`Instance::preview`]).
    pub preview: bool,
    /// Section caps without hatching: mesh parts (live G35: the cap is the
    /// part's colour, crossed only by its triangle lines).
    pub plain_cap: bool,
    /// Draw order within its pass (lower first). Overlays use larger values.
    pub layer: i32,
    /// Line width multiplier for line instances (Graphics preferences line widths).
    pub line_scale: f32,
    /// Lines / points drawn only where geometry hides them (color pass
    /// only): the active sketch's "underconstrained occluded" color.
    pub hidden_only: bool,
    /// Opaque meshes drawn after the transparent ones (color pass), so
    /// see-through parts around them don't tint them: the red overlaps of
    /// Interference detection.
    pub over_transparent: bool,
    /// Only rendered into the pick buffer (invisible but selectable), e.g.
    /// B-rep vertices and sketch regions; highlights are drawn separately.
    pub pick_only: bool,
    /// Added to the geometry's local (1-based) pick ids; 0 keeps them as-is.
    pub pick_base: u32,
    /// Zebra stripes instead of shading (Onshape's analysis tool).
    pub zebra: Option<Zebra>,
    /// A mirror reflecting an environment map (Reflection analysis).
    pub reflection: Option<Reflection>,
}

/// Reflection analysis: an equirectangular environment image (uploaded
/// with [`crate::Uploader::upload_image`]; its corners don't matter) turned
/// by `rotation` radians about the up axis, optionally blurred, and either
/// following the camera or, `lock`ed, fixed to the model (Z up).
#[derive(Clone)]
pub struct Reflection {
    pub environment: ImageHandle,
    pub rotation: f32,
    pub blur: bool,
    pub lock: bool,
}

/// Zebra stripes: the reflection of a striped room, `stripes` stripes
/// across the reflected half-space; `flip` turns them 90°.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Zebra {
    pub stripes: u32,
    pub flip: bool,
}

impl Instance {
    /// Images start white (true texture colors) and unpickable; everything
    /// else starts light gray and pickable.
    pub fn new(drawable: Drawable) -> Self {
        let image = matches!(drawable, Drawable::Image(_));
        Self {
            drawable,
            transform: Mat4::IDENTITY,
            color: if image { [1.0; 4] } else { [0.7, 0.72, 0.76, 1.0] },
            color_mode: ColorMode::Instance,
            pickable: !image,
            ignore_clip: false,
            unlit: false,
            preview: false,
            plain_cap: false,
            layer: 0,
            line_scale: 1.0,
            hidden_only: false,
            over_transparent: false,
            pick_only: false,
            pick_base: 0,
            zebra: None,
            reflection: None,
        }
    }
    /// Draw an opaque mesh after the transparent meshes ([`Instance::over_transparent`]).
    pub fn over_transparent(mut self) -> Self {
        self.over_transparent = true;
        self
    }
    /// Draw only the parts hidden behind other geometry (lines and points).
    pub fn hidden_only(mut self) -> Self {
        self.hidden_only = true;
        self
    }

    /// Multiply the widths of this line instance.
    pub fn line_scale(mut self, scale: f32) -> Self {
        self.line_scale = scale;
        self
    }
    pub fn pick_base(mut self, base: u32) -> Self {
        self.pick_base = base;
        self
    }
    pub fn pick_only(mut self) -> Self {
        self.pick_only = true;
        self
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

    /// Draw with the flat instance color, without lighting.
    pub fn unlit(mut self) -> Self {
        self.unlit = true;
        self
    }

    /// Shade as a feature preview (Onshape: flat faces lit only from the
    /// viewer's lower right, so the top and front read alike and darker than
    /// the right side), instead of the part lighting.
    pub fn preview(mut self) -> Self {
        self.preview = true;
        self
    }
    pub fn plain_cap(mut self) -> Self {
        self.plain_cap = true;
        self
    }

    /// Shade as a mirror of an environment map.
    pub fn reflection(mut self, reflection: Option<Reflection>) -> Self {
        self.reflection = reflection;
        self
    }

    /// Shade with zebra stripes.
    pub fn zebra(mut self, zebra: Option<Zebra>) -> Self {
        self.zebra = zebra;
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
    /// Entity of a hovered dialog selection row (yellow-green).
    pub const QUERY: u32 = 64;
    /// Faces of a part picked in a dialog field: the part's silhouette is
    /// outlined in the selection color, its faces keep their shading
    /// (Onshape, live part-studio/440).
    pub const OUTLINE: u32 = 128;
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
    /// Construction line pattern in px: dash, gap, dash, gap.
    pub construction_dash: [f32; 4],
    /// Ambient occlusion intensity, 0..1 (Graphics preferences); 0 = off.
    pub ambient_occlusion: f32,
    /// Specular color (linear rgb) and shininess (a, 0..1): Onshape's
    /// Graphics preferences "Specular color" / "Shininess".
    pub specular: [f32; 4],
    /// Draw B-rep edges on shaded bodies.
    pub show_edges: bool,
    /// 1, 2, 4 or 8.
    pub msaa: u32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            // Onshape: plain white viewport (`--os-background-primary`), no gradient.
            background_top: srgb(0xff, 0xff, 0xff),
            background_bottom: srgb(0xff, 0xff, 0xff),
            // Onshape PRE_HIGHLIGHT / ENTITY_HIGHLIGHT / UI_HIGHLIGHT / ERROR_HIGHLIGHT.
            hover_color: srgb(0xff, 0xc6, 0x85),
            select_color: srgb(0xff, 0xa9, 0x0a),
            accent_color: srgb(0xff, 0xc2, 0x0e),
            error_color: srgb(0xbd, 0x30, 0x39),
            edge_color: srgb(0x00, 0x00, 0x00),
            construction_dash: [4.0, 6.0, 30.0, 6.0],
            ambient_occlusion: 0.0,
            // Onshape's defaults: specular #999999, shininess 3%.
            specular: { let c = srgb(0x99, 0x99, 0x99); [c[0], c[1], c[2], 0.03] },
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
    /// Up to two more section planes (with `clip`): geometry is removed
    /// where all planes clip it.
    pub clip_extra: Vec<ClipPlane>,
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

    /// Whether rendering `other` would produce the same image (same GPU
    /// buffers, instance parameters, states and clipping). Cheap: compares
    /// handles by pointer, not geometry.
    pub fn same_as(&self, other: &Scene) -> bool {
        self.clip == other.clip
            && self.clip_extra == other.clip_extra
            && self.states == other.states
            && self.instances.len() == other.instances.len()
            && self.instances.iter().zip(&other.instances).all(|(a, b)| {
                let same_drawable = match (&a.drawable, &b.drawable) {
                    (Drawable::Mesh(x), Drawable::Mesh(y)) => Arc::ptr_eq(&x.0, &y.0),
                    (Drawable::Lines(x), Drawable::Lines(y)) => Arc::ptr_eq(&x.0, &y.0),
                    (Drawable::Points(x), Drawable::Points(y)) => Arc::ptr_eq(&x.0, &y.0),
                    (Drawable::Image(x), Drawable::Image(y)) => Arc::ptr_eq(&x.0, &y.0),
                    _ => false,
                };
                same_drawable
                    && a.transform == b.transform
                    && a.color == b.color
                    && a.color_mode == b.color_mode
                    && a.pickable == b.pickable
                    && a.ignore_clip == b.ignore_clip
                    && a.unlit == b.unlit
                    && a.preview == b.preview
                    && a.line_scale == b.line_scale
                    && a.hidden_only == b.hidden_only
                    && a.layer == b.layer
                    && a.pick_only == b.pick_only
                    && a.pick_base == b.pick_base
            })
    }

    /// World-space bounds of all instances.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut acc: Option<(Vec3, Vec3)> = None;
        for inst in &self.instances {
            let b = match &inst.drawable {
                Drawable::Mesh(m) => m.bounds(),
                Drawable::Lines(l) => l.bounds(),
                Drawable::Points(p) => p.bounds(),
                Drawable::Image(i) => i.bounds(),
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
