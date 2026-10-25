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
    /// Onshape's inferred-direction dash over a sketch rubber band: 4 px on,
    /// 4 px off, starting 1.25 px from the segment's start (live sketch/51).
    pub const INFERRED: u32 = 16;
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

/// Tessellation edges inside curved faces, each with the normals of its
/// two facets: drawn as edges where they are silhouettes for the camera
/// (one facet toward the viewer, the other away), like Onshape's
/// edge-based silhouettes.
#[derive(Clone, Debug, Default)]
pub struct SilhouetteData {
    pub segments: Vec<SilhouetteSegment>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct SilhouetteSegment {
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub n1: [f32; 3],
    pub n2: [f32; 3],
    /// The face's pick id (its state: hidden, dimmed, highlighted).
    pub pick_id: u32,
    /// Width in logical pixels (the part's edges').
    pub width: f32,
}

impl SilhouetteData {
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
    /// ([`PointSprite::HOLLOW`]): white center inside a colored ring. Bit 4
    /// ([`PointSprite::RING`]): the ring alone.
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
    /// A ring alone (inner radius 0.6 of the outer), see-through inside.
    pub const RING: u32 = 16;
}

/// The axis-aligned bounds of `it` (`None` when empty).
pub fn bounds_of(it: impl Iterator<Item = [f32; 3]>) -> Option<(Vec3, Vec3)> {
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
    /// Left out of [`Scene::bounds`] (the depth range and zoom to fit): an
    /// annotation such as a measure line can reach far beyond the model.
    pub no_bounds: bool,
    /// Drawn with its flat color, without lighting (construction planes).
    /// See [`Instance::back_silhouette`].
    pub back_silhouette: bool,
    pub unlit: bool,
    /// Lit like Onshape's feature previews (see [`Instance::preview`]).
    pub preview: bool,
    /// Onshape's Isolate ghost: after shading, the color is mixed toward
    /// white by this much in sRGB, as a see-through layer over a white page
    /// looks, while still hiding what is behind (0: off).
    pub ghost: f32,
    /// Draw order within its pass (lower first). Overlays use larger values.
    pub layer: i32,
    /// Line width multiplier for line instances (Graphics preferences line widths).
    pub line_scale: f32,
    /// Lines / points drawn only where geometry hides them (color pass
    /// only): the active sketch's "underconstrained occluded" color.
    pub hidden_only: bool,
    /// Opaque meshes drawn after the transparent ones (color pass), so
    /// see-through parts around them don't tint them: the red overlaps of
    /// Interference detection. Translucent ones go after the other
    /// translucent meshes and previews (picked sketch regions).
    pub over_transparent: bool,
    /// Translucent meshes drawn after the lines and points (color pass): a
    /// drawing's release watermark lies over the ink.
    pub over_lines: bool,
    /// Translucent meshes drawn through everything (no depth test or write,
    /// after the transparent meshes, before the lines): FeatureScript debug
    /// faces, which Onshape shows behind the model too.
    pub on_top: bool,
    /// Only rendered into the pick buffer (invisible but selectable), e.g.
    /// B-rep vertices and sketch regions; highlights are drawn separately.
    pub pick_only: bool,
    /// Added to the geometry's local (1-based) pick ids; 0 keeps them as-is.
    pub pick_base: u32,
    /// Zebra stripes instead of shading (Onshape's analysis tool).
    pub zebra: Option<Zebra>,
    /// A mirror reflecting an environment map (Reflection analysis).
    pub reflection: Option<Reflection>,
    /// Compare mode ([`Scene::compare`]): which version this instance draws
    /// ([`compare::BASE`], [`compare::TARGET`]; 0: neither, always drawn).
    pub compare: u8,
    /// The body (Onshape's body occurrence) this instance draws, 1-based
    /// and dense per scene (0: none). At its lowest level of detail
    /// ([`Detail`], `performAggressiveCulling`) all its instances give way
    /// to its [`Instance::lod_box`].
    pub body: u32,
    /// The body's display level this instance belongs to: 0 every level;
    /// k ≥ 1 drawn only while the body shows level k − 1 (0 Coarse … 3
    /// Very fine, [`mesh_lod_level`]; bodies have several only viewing in
    /// high quality): its mesh, edges, silhouettes and picks at that
    /// tessellation.
    pub mesh_lod: u8,
    /// Edge lines with levels of detail: 0 none (always drawn); k ≥ 1 the
    /// body's edges simplified to [`LINE_LOD_TOLERANCES`]`[k - 1]` of its
    /// size, drawn only when the renderer picks level k for the body
    /// ([`line_lod_level`]). Level 1 (the full edges) is also the pick.
    pub line_lod: u8,
    /// The [`Instance::body`]'s lowest level of detail: its bounding box
    /// (a unit cube placed by `transform`) in the body's color
    /// (`getDisplayIncrement`), drawn only in the body's place.
    pub lod_box: bool,
}

/// Versions and history › Compare, as Onshape draws it (webgl bundle): the
/// scene holds both versions' instances. Each version's faces are lit in
/// COMPARE_SAME_LIT_TINT where the other version has the same surface at
/// that pixel (depth within 0.0004 of Onshape's near..far, normals within
/// dot 0.98), else in its tint (base rgb(144,207,241), target
/// rgb(221,82,40)). The frame is the "under" version alone (the base below
/// blend 0.5, else the target) mixed with both versions drawn together, by
/// 2·blend below 0.5 and 2·(1 − blend) above.
pub mod compare {
    pub const BASE: u8 = 1;
    pub const TARGET: u8 = 2;
}

/// The compare blend of a frame (0 = base, 1 = target).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Compare {
    pub blend: f32,
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
            no_bounds: false,
            unlit: false,
            back_silhouette: false,
            preview: false,
            ghost: 0.0,
            layer: 0,
            line_scale: 1.0,
            hidden_only: false,
            over_transparent: false,
            over_lines: false,
            on_top: false,
            pick_only: false,
            pick_base: 0,
            zebra: None,
            reflection: None,
            compare: 0,
            body: 0,
            lod_box: false,
            line_lod: 0,
            mesh_lod: 0,
        }
    }
    /// The version this instance draws in compare mode ([`compare`]).
    pub fn compare_layer(mut self, layer: u8) -> Self {
        self.compare = layer;
        self
    }
    /// Draw an opaque mesh after the transparent meshes, a translucent one
    /// after the other translucent ones and feature previews
    /// ([`Instance::over_transparent`]).
    pub fn over_transparent(mut self) -> Self {
        self.over_transparent = true;
        self
    }
    /// Draw a translucent mesh after the lines and points ([`Instance::over_lines`]).
    pub fn over_lines(mut self) -> Self {
        self.over_lines = true;
        self
    }
    /// Draw a translucent mesh through all other geometry ([`Instance::on_top`]).
    pub fn on_top(mut self) -> Self {
        self.on_top = true;
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

    /// See [`Instance::no_bounds`].
    pub fn no_bounds(mut self) -> Self {
        self.no_bounds = true;
        self
    }

    /// Draw with the flat instance color, without lighting.
    pub fn unlit(mut self) -> Self {
        self.unlit = true;
        self
    }

    /// Part of body `id` ([`Instance::body`]).
    pub fn body(mut self, id: u32) -> Self {
        self.body = id;
        self
    }
    /// The body's edges at line level `level` ([`Instance::line_lod`]).
    pub fn line_lod(mut self, level: u8) -> Self {
        self.line_lod = level;
        self
    }
    /// Body `id`'s lowest level of detail ([`Instance::lod_box`]): `mesh` a
    /// unit cube ([`lod_box_mesh`]), `bounds` the body's in its local space.
    pub fn lod_box(mesh: MeshHandle, id: u32, (lo, hi): (Vec3, Vec3), color: [f32; 4]) -> Self {
        let mut i = Self::new(Drawable::Mesh(mesh)).with_color(color).unpickable();
        i.transform = Mat4::from_translation((lo + hi) * 0.5) * Mat4::from_scale((hi - lo).max(Vec3::splat(1e-9)));
        i.body = id;
        i.lod_box = true;
        i
    }
    /// Draw this mesh as Onshape's image-based (backface) silhouettes in
    /// the instance color, not as faces; never picked.
    pub fn back_silhouette(mut self) -> Self {
        self.back_silhouette = true;
        self.pickable = false;
        self
    }

    /// Shade as a feature preview (Onshape: flat faces lit only from the
    /// viewer's lower right, so the top and front read alike and darker than
    /// the right side), instead of the part lighting.
    pub fn preview(mut self) -> Self {
        self.preview = true;
        self
    }

    /// Draw as an Isolate ghost (see [`Instance::ghost`]).
    pub fn ghost(mut self, amount: f32) -> Self {
        self.ghost = amount;
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
    /// With SELECTED on faces: a whole part / instance selected, Onshape's
    /// lighter part amber (live #f2b648 lit, #c09039 in shade) instead of a
    /// selected face's; with OUTLINE its silhouette is #ffc04c, 3 px. On
    /// edges (alone): the part's edges let the amber through.
    pub const BODY: u32 = 256;
    /// Faces: a light selection tint over their shading (20 % of the
    /// selection color in sRGB; live "lp wall over" 2258: a feature picked
    /// in Features to pattern, end face #4e6876 → #717560).
    pub const TINT: u32 = 512;
    /// Left out of the pick pass (drawn as usual): an overlay the active
    /// dialog field can't take, so what lies under it can be picked (a
    /// shown sketch line on a part edge for Fillet's edges).
    pub const NO_PICK: u32 = 1024;
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
    /// What Onshape's interactive detail reduction can leave out (ambient
    /// occlusion is `ambient_occlusion` 0).
    pub detail: Detail,
}

/// Graphics Onshape drops while the camera moves too slowly (webgl
/// DetailManager `decreaseDetail`); all on by default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    /// Edge-based silhouettes ([`SilhouetteData`]).
    pub edge_silhouettes: bool,
    /// Image-based silhouettes ([`Instance::back_silhouette`]).
    pub backface_silhouettes: bool,
    /// The section caps' outlines.
    pub section_edges: bool,
    /// The share of bodies that may be drawn as their bounding box
    /// ([`Instance::lod_box`]; `lowLevelBodyProportion`, 0..0.9).
    pub low_level_body_proportion: f32,
    /// Bodies smaller than this on screen (CSS px) are always drawn as
    /// their box (`extentCullingThresholdPx`: 2 at full detail, 10 while
    /// the camera moves).
    pub extent_culling_px: f32,
    /// A body shows its next finer display level while its chordal error
    /// on screen is at least this (CSS px, [`mesh_lod_level`]):
    /// [`MESH_ERROR_PX`], viewing in high quality 0.1 in a Part Studio and
    /// 1 in an assembly.
    pub mesh_error_px: f32,
}

/// `BODY_SCREENSPACE_ERROR_REFINEMENT_THRESHOLD`, and View in high
/// quality's `VIEWING_PART_STUDIO_` / `VIEWING_ASSEMBLY_AGGRESSIVE_ERROR_REFINEMENT_THRESHOLD`.
pub const MESH_ERROR_PX: f32 = 3.0;
pub const MESH_ERROR_PX_HIGH_QUALITY_STUDIO: f32 = 0.1;
pub const MESH_ERROR_PX_HIGH_QUALITY_ASSEMBLY: f32 = 1.0;

/// Each display level's chordal tolerance as a share of the body's
/// bounding radius (`CHORDAL_TOLERANCE_RATIO_COARSE` … `_VERY_FINE`: the
/// sagitta of 12, 24, 72 and 300 segments per circle).
pub const MESH_LOD_CHORD_RATIOS: [f32; 4] = [0.03407418, 0.00855514, 0.00095178, 5.4831e-5];

/// Onshape's `getTessellationIndexToGoUnderThreshold` (View in high
/// quality): a body `radius_px` CSS px in bounding radius on screen steps
/// up from its own level `from` while its chordal error there is at least
/// `threshold_px`, up to `to` (Very fine).
pub fn mesh_lod_level(radius_px: f32, threshold_px: f32, from: u8, to: u8) -> u8 {
    let mut level = from;
    while level < to && (level as usize) < MESH_LOD_CHORD_RATIOS.len() && radius_px * MESH_LOD_CHORD_RATIOS[level as usize] >= threshold_px {
        level += 1;
    }
    level
}

/// `DEFAULT_EXTENT_CULLING_THRESHOLD_IN_PIXELS` /
/// `AGGRESSIVE_EXTENT_CULLING_THRESHOLD_IN_PIXELS`.
pub const EXTENT_CULLING_PX: f32 = 2.0;
pub const INTERACTIVE_EXTENT_CULLING_PX: f32 = 10.0;

impl Default for Detail {
    fn default() -> Self {
        Self { edge_silhouettes: true, backface_silhouettes: true, section_edges: true, low_level_body_proportion: 0.0, extent_culling_px: EXTENT_CULLING_PX, mesh_error_px: MESH_ERROR_PX }
    }
}

/// The unit cube (−0.5..0.5) every [`Instance::lod_box`] draws.
pub fn lod_box_mesh() -> MeshData {
    crate::primitives::box_mesh([-0.5; 3], [0.5; 3], 0)
}

/// Edge line levels of detail ([`Instance::line_lod`]): the largest
/// distance of a simplified edge from the full one, as a share of the
/// body's bounding-box diagonal. 144-segment circles cost little drawn as
/// fill but a lot as edge quads; on screen a level only shows from where
/// its error passes [`LINE_LOD_MAX_ERROR_PX`].
/// Steps of 4 from 1/16000: a large body with small detail (the big
/// plate's Ø8 holes, ~16 px each at fit while the plate is ~1100 px)
/// still drops its sub-pixel segments.
pub const LINE_LOD_TOLERANCES: [f32; 6] = [0.0, 6.25e-5, 2.5e-4, 1e-3, 4e-3, 1.6e-2];
/// The screen error (CSS px) a simplified edge level may have.
pub const LINE_LOD_MAX_ERROR_PX: f32 = 0.25;

/// The coarsest line level (1-based, [`LINE_LOD_TOLERANCES`]) for a body
/// whose bounding-box diagonal is `diagonal_px` CSS px on screen.
pub fn line_lod_level(diagonal_px: f32) -> u8 {
    LINE_LOD_TOLERANCES.iter().rposition(|&t| t * diagonal_px <= LINE_LOD_MAX_ERROR_PX).map_or(1, |i| i as u8 + 1)
}

/// Douglas–Peucker: the points of `pts` that keep every dropped point
/// within `tol` of the simplified polyline; the ends always stay.
pub fn simplify_polyline(pts: &[[f32; 3]], tol: f32) -> Vec<[f32; 3]> {
    if pts.len() < 3 || tol <= 0.0 {
        return pts.to_vec();
    }
    let dist = |p: Vec3, a: Vec3, b: Vec3| {
        let ab = b - a;
        let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        (p - (a + ab * t)).length()
    };
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0, pts.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (Vec3::from(pts[a]), Vec3::from(pts[b]));
        let far = (a + 1..b).map(|i| (i, dist(Vec3::from(pts[i]), pa, pb))).max_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((i, d)) = far
            && d > tol
        {
            keep[i] = true;
            stack.push((a, i));
            stack.push((i, b));
        }
    }
    pts.iter().zip(&keep).filter(|(_, k)| **k).map(|(p, _)| *p).collect()
}

/// A body's edge polylines (`edges`, pick ids 1-based by edge) as line
/// data at each of [`LINE_LOD_TOLERANCES`] after the first, for a body
/// `diagonal` across.
pub fn edge_line_lods(edges: &[Vec<[f32; 3]>], diagonal: f32) -> Vec<LineData> {
    LINE_LOD_TOLERANCES[1..]
        .iter()
        .map(|t| {
            let mut l = LineData::default();
            for (ei, pl) in edges.iter().enumerate() {
                l.push_polyline(&simplify_polyline(pl, t * diagonal), [0, 0, 0, 255], ei as u32 + 1, 1.25, 0);
            }
            l
        })
        .collect()
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
            detail: Detail::default(),
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
    /// The cap hatching's frame (Onshape's section plane center and
    /// tangent): a point on the plane and a unit direction in it.
    pub center: Vec3,
    pub tangent: Vec3,
}

impl ClipPlane {
    /// `normal` points to the removed side. The cap frame is Onshape's for a
    /// plane parallel to a standard one (live G44): its tangent is the
    /// standard plane's x axis (Top: X, normal Z; Right: Y, normal X;
    /// Front: X, normal -Y), negated when the normal is the standard
    /// plane's (a cut from Right +20 mm: tangent (0, -1, 0)); else any
    /// in-plane direction. Centered at the point nearest the origin.
    pub fn new(normal: Vec3, d: f32) -> Self {
        let n = normal.normalize_or_zero();
        let (axis, x) = if n.z.abs() > 0.999 {
            (Vec3::Z, Vec3::X)
        } else if n.x.abs() > 0.999 {
            (Vec3::X, Vec3::Y)
        } else if n.y.abs() > 0.999 {
            (Vec3::NEG_Y, Vec3::X)
        } else {
            (n, n.any_orthonormal_vector())
        };
        let x = -x * n.dot(axis).signum();
        Self { normal, d, center: n * d, tangent: (x - n * x.dot(n)).normalize_or_zero() }
    }

    /// This plane with a cap frame through `center` with x along `tangent`
    /// (projected into the plane).
    pub fn with_frame(mut self, center: Vec3, tangent: Vec3) -> Self {
        let n = self.normal.normalize_or_zero();
        let t = (tangent - n * tangent.dot(n)).normalize_or_zero();
        if t != Vec3::ZERO {
            self.center = center - n * (center.dot(n) - self.d);
            self.tangent = t;
        }
        self
    }
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
    /// Section planes drawn selected (`clip` first, then `clip_extra`): the
    /// one being edited in the Section view dialog. Onshape mixes its cap 80%
    /// toward the section pre-highlight color (webgl endcap shader).
    pub clip_selected: [bool; 3],
    /// Compare mode: instances tagged [`Instance::compare`] are the two
    /// versions (see [`compare`]).
    pub compare: Option<Compare>,
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
        for inst in self.instances.iter().filter(|i| !i.no_bounds) {
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
