//! Screen overlays drawn into the viewport's own frame: the view cube and
//! text laid out in model planes (plane names). They follow the camera, so
//! drawing them here keeps them in step with the 3D image and lets a camera
//! move be shown without any UI layout or paint (Onshape draws them in its
//! WebGL canvas too).
//!
//! The look is the UI toolkit's (GPUI's) path rendering: polygons and strokes
//! are tessellated with lyon exactly like `gpui::PathBuilder` does (logical
//! pixels, default fill and stroke options), rasterized with 4x MSAA and
//! blended premultiplied in gamma space (an 8-bit UNORM layer, not the sRGB
//! frame), then composited over the frame after ambient occlusion.
//! [`Overlay`] holds what doesn't depend on the camera; [`Overlay::build`]
//! turns it into triangles for one frame's camera.

use crate::camera::{Camera, StandardView};
use glam::{DMat4, DQuat, DVec2, DVec3, DVec4};
use lyon_tessellation::math::point as lpoint;
use lyon_tessellation::path::Path as LPath;
use lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers};
use std::sync::Arc;

/// Straight-alpha color, sRGB-encoded components 0..1 (as GPUI's `Rgba`).
pub type Rgba = [f32; 4];

/// Closed contours (glyph outlines), shared between frames.
pub type Contours = Arc<Vec<Vec<DVec3>>>;

/// What an overlay shows, apart from the camera.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overlay {
    /// Filled text or shapes lying in model planes, drawn first.
    pub world_fills: Vec<WorldFill>,
    /// Boxes with text at projected points (value boxes, labels), drawn
    /// after the fills, in order.
    pub labels: Vec<Label>,
    /// The frame's top-left corner in the window, logical px: GPUI rounds
    /// positions in the window, so labels round from there.
    pub window_origin: [f32; 2],
    pub view_cube: Option<ViewCube>,
}

/// A box with text at a model point, drawn as GPUI draws an absolutely
/// positioned div at the point's projection plus `offset` (logical px):
/// GPUI's layout rounds the box's corner to device pixels (half toward
/// zero) and everything inside sits at fixed device offsets from it, so the
/// shapes and text masks (rasterized by GPUI once, at those offsets) are
/// given relative to the rounded corner and only the corner moves with the
/// camera.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Label {
    pub anchor: DVec3,
    pub offset: [f32; 2],
    /// Filled rectangles (x0, y0, x1, y1), device px from the corner.
    pub rects: Vec<([f32; 4], Rgba)>,
    /// GPUI box shadows and quads (rounded, bordered), device px from the
    /// corner, drawn before the rectangles.
    pub shadows: Vec<Shadow>,
    pub quads: Vec<Quad>,
    /// Text and icon masks: top-left in device px from the corner, color.
    pub masks: Vec<(Mask, [i32; 2], Rgba)>,
    /// What the pointer finds over the label's quads and rectangles
    /// ([`Overlay::label_at`]); `None`: not hit tested.
    pub hit: Option<u64>,
}

/// A GPUI quad: a div's background and solid border, as GPUI's `fs_quad`
/// draws it (signed distance antialiasing). Device px.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Quad {
    /// (x0, y0, x1, y1).
    pub bounds: [f32; 4],
    pub corner_radius: f32,
    pub background: Rgba,
    pub border_width: f32,
    pub border_color: Rgba,
}

/// A GPUI drop shadow (`BoxShadow`) as `fs_shadow` draws it: `bounds` is
/// the element's bounds moved by the shadow's offset. Device px.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shadow {
    pub bounds: [f32; 4],
    pub corner_radius: f32,
    pub blur_radius: f32,
    pub color: Rgba,
}

/// A quad or shadow for the GPU: frame device px, straight (not
/// premultiplied) color; `params` = (corner radius, border width or blur
/// radius, 0, 0).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadInstance {
    pub bounds: [f32; 4],
    pub color: [f32; 4],
    pub border_color: [f32; 4],
    pub params: [f32; 4],
}

/// Contours in world space, filled even-odd in one color (one GPUI path).
#[derive(Clone, Debug, PartialEq)]
pub struct WorldFill {
    pub contours: Contours,
    pub color: Rgba,
}

/// An alpha mask (text or an icon rasterized by the UI toolkit), 1 byte
/// per pixel.
#[derive(Clone, Debug, PartialEq)]
pub struct Mask {
    pub width: u32,
    pub height: u32,
    pub alpha: Arc<Vec<u8>>,
    /// Device pixels from the glyph's integer origin to the mask's top-left
    /// corner (see [`AxisLabel`]).
    pub offset: [i32; 2],
    /// Texels per device pixel: 1 for text; 2 for icons, which GPUI
    /// rasterizes at `SMOOTH_SVG_SCALE_FACTOR` and samples down onto
    /// `ceil(size / 2)` pixels.
    pub density: u32,
}

impl Mask {
    /// The device pixels it covers.
    pub fn drawn_size(&self) -> [u32; 2] {
        let d = self.density.max(1);
        [self.width.div_ceil(d), self.height.div_ceil(d)]
    }
}

/// GPUI's horizontal sub-pixel glyph variants (`SUBPIXEL_VARIANTS_X`).
pub const SUBPIXEL_VARIANTS_X: usize = 4;

/// How GPUI's renderer on this platform draws what the overlay imitates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextShading {
    /// Apply GPUI's (wgpu) contrast and gamma correction to text coverage.
    pub correct: bool,
    pub grayscale_enhanced_contrast: f32,
    pub gamma_ratios: [f32; 4],
    /// Drop path pixels whose center is outside the path's bounds (GPUI's
    /// wgpu path shader; Metal clips geometrically, which changes nothing).
    pub clip_path_pixels: bool,
}

impl Default for TextShading {
    fn default() -> Self {
        Self { correct: false, grayscale_enhanced_contrast: 1.0, gamma_ratios: [0.0; 4], clip_path_pixels: false }
    }
}

/// An axis label of the view cube (one glyph), placed like GPUI places a
/// glyph: its device x quantized to [`SUBPIXEL_VARIANTS_X`] variants (one
/// mask each), its y rounded half toward zero.
#[derive(Clone, Debug, PartialEq)]
pub struct AxisLabel {
    pub masks: [Mask; SUBPIXEL_VARIANTS_X],
    /// Device pixels from the label box's left edge to the text element:
    /// centered in the 20 px box, whose width and the text's (rounded up)
    /// are whole device pixels in GPUI's layout, so this may be a half pixel.
    pub text_x: f32,
    /// The glyph's origin in device pixels from the text element's top-left
    /// corner (its x in the line, the 16 px line's baseline).
    pub glyph: [f32; 2],
    pub color: Rgba,
}

/// GPUI's `round_half_toward_zero`.
fn round_half_toward_zero(v: f32) -> f32 {
    (v.abs() - 0.5).ceil().copysign(v)
}

/// The view cube's appearance (see `view_cube`), all but the rotation.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewCube {
    /// The widget's top-left corner in the frame, in device pixels (where
    /// GPUI lays it out: it may fall between pixels).
    pub origin: [f32; 2],
    pub hover: Option<view_cube::CubeCell>,
    /// Face labels: per face (outward normal), glyph contours in cube space.
    pub face_labels: Vec<([i8; 3], Contours)>,
    /// X, Y and Z, if rasterized.
    pub axis_labels: Option<[AxisLabel; 3]>,
    pub colors: CubeColors,
}

/// Onshape's view cube colors (straight alpha).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubeColors {
    pub face: Rgba,
    pub frame: Rgba,
    pub highlight: Rgba,
    pub label: Rgba,
    pub axes: [Rgba; 3],
}

/// One frame's overlay: triangles in device pixels with premultiplied
/// colors, in drawing order, and text masks.
#[derive(Default)]
pub struct OverlayDraw {
    pub vertices: Vec<OverlayVertex>,
    pub indices: Vec<u32>,
    /// Masks drawn after the triangles: (mask, top-left in device pixels, color).
    pub masks: Vec<(Mask, [i32; 2], Rgba)>,
    /// Shadows and quads, drawn first (shadows, then quads).
    pub shadows: Vec<QuadInstance>,
    pub quads: Vec<QuadInstance>,
    /// Device-pixel bounds of everything drawn: (x0, y0, x1, y1).
    pub bounds: Option<[f32; 4]>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OverlayVertex {
    pub position: [f32; 2],
    /// Premultiplied.
    pub color: [f32; 4],
    /// The path's bounds (x0, y0, x1, y1): like GPUI's path shader, pixels
    /// whose center lies outside them are not drawn.
    pub bounds: [f32; 4],
}

impl OverlayDraw {
    fn grow(&mut self, x: f32, y: f32) {
        let b = self.bounds.get_or_insert([x, y, x, y]);
        b[0] = b[0].min(x);
        b[1] = b[1].min(y);
        b[2] = b[2].max(x);
        b[3] = b[3].max(y);
    }

    /// Append a tessellated path (logical pixels) scaled to device pixels.
    fn push(&mut self, buf: &VertexBuffers<lyon_tessellation::math::Point, u32>, color: Rgba, dpr: f32) {
        if buf.indices.is_empty() {
            return;
        }
        let base = self.vertices.len() as u32;
        let premul = [color[0] * color[3], color[1] * color[3], color[2] * color[3], color[3]];
        let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for v in &buf.vertices {
            let (x, y) = (v.x * dpr, v.y * dpr);
            bounds = [bounds[0].min(x), bounds[1].min(y), bounds[2].max(x), bounds[3].max(y)];
        }
        for v in &buf.vertices {
            let (x, y) = (v.x * dpr, v.y * dpr);
            self.grow(x, y);
            self.vertices.push(OverlayVertex { position: [x, y], color: premul, bounds });
        }
        self.indices.extend(buf.indices.iter().map(|i| base + i));
    }

    /// A filled axis-aligned rectangle in device px (a GPUI quad on whole
    /// pixels).
    fn rect(&mut self, r: [f32; 4], color: Rgba) {
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        let base = self.vertices.len() as u32;
        let premul = [color[0] * color[3], color[1] * color[3], color[2] * color[3], color[3]];
        for (x, y) in [(r[0], r[1]), (r[2], r[1]), (r[2], r[3]), (r[0], r[3])] {
            self.grow(x, y);
            self.vertices.push(OverlayVertex { position: [x, y], color: premul, bounds: r });
        }
        self.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// `gpui::PathBuilder::fill()` + `paint_path`: polygons (logical px) as
    /// one even-odd path.
    fn fill(&mut self, polys: &[Vec<DVec2>], color: Rgba, dpr: f32) {
        let mut b = LPath::builder().with_svg();
        let mut any = false;
        for poly in polys.iter().filter(|p| p.len() >= 3) {
            b.move_to(lpoint(poly[0].x as f32, poly[0].y as f32));
            for q in &poly[1..] {
                b.line_to(lpoint(q.x as f32, q.y as f32));
            }
            b.close();
            any = true;
        }
        if !any {
            return;
        }
        let path = b.build();
        let mut buf: VertexBuffers<lyon_tessellation::math::Point, u32> = VertexBuffers::new();
        if FillTessellator::new().tessellate_path(&path, &FillOptions::default(), &mut BuffersBuilder::new(&mut buf, |v: FillVertex| v.position())).is_ok() {
            self.push(&buf, color, dpr);
        }
    }

    /// `gpui::PathBuilder::stroke(width)` of one segment (logical px).
    fn stroke(&mut self, a: DVec2, b: DVec2, width: f32, color: Rgba, dpr: f32) {
        let mut p = LPath::builder().with_svg();
        p.move_to(lpoint(a.x as f32, a.y as f32));
        p.line_to(lpoint(b.x as f32, b.y as f32));
        let path = p.build();
        let mut buf: VertexBuffers<lyon_tessellation::math::Point, u32> = VertexBuffers::new();
        if StrokeTessellator::new().tessellate_path(&path, &StrokeOptions::default().with_line_width(width), &mut BuffersBuilder::new(&mut buf, |v: StrokeVertex| v.position())).is_ok() {
            self.push(&buf, color, dpr);
        }
    }
}

/// A model point in logical px of a frame (`view_proj`, `logical` size),
/// or `None` behind the camera.
fn project_px(view_proj: &DMat4, logical: DVec2, p: DVec3) -> Option<DVec2> {
    let clip = *view_proj * DVec4::new(p.x, p.y, p.z, 1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some(DVec2::new((ndc.x + 1.0) * 0.5 * logical.x, (1.0 - ndc.y) * 0.5 * logical.y))
}

/// A label's rounded corner in frame device px (GPUI's layout), or `None`
/// behind the camera. In f32, as the owner computes the div's left and
/// top; GPUI rounds the div's position in the window, the frame's corner
/// is at the window origin (logical px) rounded the same way.
fn label_corner(label: &Label, window_origin: [f32; 2], project: &impl Fn(DVec3) -> Option<DVec2>, dpr: f32) -> Option<[f32; 2]> {
    let at = project(label.anchor)?;
    let o = window_origin;
    let round = |frame: f32, local: f32, off: f32| round_half_toward_zero((frame + local + off) * dpr) - round_half_toward_zero(frame * dpr);
    Some([round(o[0], at.x as f32, label.offset[0]), round(o[1], at.y as f32, label.offset[1])])
}

/// The topmost hit-tested label of `labels` under `point` (logical px in
/// the frame) with the frame's camera: where [`Overlay::build`] draws it,
/// over its quads and rectangles (GPUI's hitbox is the div's bounds).
pub fn label_at(labels: &[Label], window_origin: [f32; 2], view_proj: &DMat4, logical: DVec2, dpr: f32, point: DVec2) -> Option<u64> {
    let project = |p: DVec3| project_px(view_proj, logical, p);
    let (px, py) = (point.x as f32 * dpr, point.y as f32 * dpr);
    labels.iter().rev().filter(|l| l.hit.is_some()).find_map(|label| {
        let corner = label_corner(label, window_origin, &project, dpr)?;
        let inside = label.quads.iter().map(|q| q.bounds).chain(label.rects.iter().map(|r| r.0)).any(|b| px >= corner[0] + b[0] && px < corner[0] + b[2] && py >= corner[1] + b[1] && py < corner[1] + b[3]);
        inside.then_some(label.hit).flatten()
    })
}

impl Overlay {
    /// [`label_at`] over this overlay's labels.
    pub fn label_at(&self, view_proj: &DMat4, logical: DVec2, dpr: f32, point: DVec2) -> Option<u64> {
        label_at(&self.labels, self.window_origin, view_proj, logical, dpr, point)
    }

    /// Whether there is nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.world_fills.is_empty() && self.labels.is_empty() && self.view_cube.is_none()
    }

    /// The triangles and masks of one frame: `view_proj` is the frame's
    /// view-projection, `logical` the viewport size in logical pixels.
    pub fn build(&self, camera: &Camera, view_proj: &DMat4, logical: DVec2, dpr: f32) -> OverlayDraw {
        let mut out = OverlayDraw::default();
        let project = |p: DVec3| project_px(view_proj, logical, p);
        for fill in &self.world_fills {
            // A contour with a point behind the camera is dropped (like
            // the Part Studio's `in_plane_text`).
            let polys: Vec<Vec<DVec2>> = fill.contours.iter().filter_map(|c| c.iter().map(|p| project(*p)).collect::<Option<Vec<_>>>()).filter(|p| p.len() >= 3).collect();
            out.fill(&polys, fill.color, dpr);
        }
        for label in &self.labels {
            let Some(corner) = label_corner(label, self.window_origin, &project, dpr) else { continue };
            let at = |b: [f32; 4]| [corner[0] + b[0], corner[1] + b[1], corner[0] + b[2], corner[1] + b[3]];
            for sh in &label.shadows {
                let b = at(sh.bounds);
                let m = 3.0 * sh.blur_radius;
                out.grow(b[0] - m, b[1] - m);
                out.grow(b[2] + m, b[3] + m);
                out.shadows.push(QuadInstance { bounds: b, color: sh.color, border_color: [0.0; 4], params: [sh.corner_radius, sh.blur_radius, 0.0, 0.0] });
            }
            for q in &label.quads {
                let b = at(q.bounds);
                out.grow(b[0], b[1]);
                out.grow(b[2], b[3]);
                out.quads.push(QuadInstance { bounds: b, color: q.background, border_color: q.border_color, params: [q.corner_radius, q.border_width, 0.0, 0.0] });
            }
            for (r, color) in &label.rects {
                out.rect([corner[0] + r[0], corner[1] + r[1], corner[0] + r[2], corner[1] + r[3]], *color);
            }
            for (mask, at, color) in &label.masks {
                let top_left = [corner[0] as i32 + at[0], corner[1] as i32 + at[1]];
                let [w, h] = mask.drawn_size();
                out.grow(top_left[0] as f32, top_left[1] as f32);
                out.grow((top_left[0] + w as i32) as f32, (top_left[1] + h as i32) as f32);
                out.masks.push((mask.clone(), top_left, *color));
            }
        }
        if let Some(cube) = &self.view_cube {
            view_cube::build(cube, camera.rotation, dpr, &mut out);
        }
        out
    }
}

/// Closed 2D contours filled even-odd (as [`Overlay`]'s fills) as a
/// triangle mesh placed by `to_world`: text lying in a plane that the
/// scene's depth hides (plane names behind a part, live selection/02).
pub fn fill_mesh(contours: &[Vec<DVec2>], to_world: impl Fn(DVec2) -> DVec3, normal: DVec3) -> Option<crate::scene::MeshData> {
    // lyon works in f32 with fixed tolerances: tessellate at a few hundred
    // units across.
    const K: f64 = 100.0;
    let mut b = LPath::builder().with_svg();
    let mut any = false;
    for c in contours.iter().filter(|c| c.len() >= 3) {
        b.move_to(lpoint((c[0].x * K) as f32, (c[0].y * K) as f32));
        for q in &c[1..] {
            b.line_to(lpoint((q.x * K) as f32, (q.y * K) as f32));
        }
        b.close();
        any = true;
    }
    if !any {
        return None;
    }
    let path = b.build();
    let mut buf: VertexBuffers<lyon_tessellation::math::Point, u32> = VertexBuffers::new();
    FillTessellator::new().tessellate_path(&path, &FillOptions::default(), &mut BuffersBuilder::new(&mut buf, |v: FillVertex| v.position())).ok()?;
    let n = [normal.x as f32, normal.y as f32, normal.z as f32];
    let positions: Vec<[f32; 3]> = buf
        .vertices
        .iter()
        .map(|p| {
            let w = to_world(DVec2::new(f64::from(p.x) / K, f64::from(p.y) / K));
            [w.x as f32, w.y as f32, w.z as f32]
        })
        .collect();
    let len = positions.len();
    Some(crate::scene::MeshData { positions, normals: vec![n; len], pick_ids: vec![0; len], indices: buf.indices })
}

/// Onshape's view cube: geometry shared by the drawing ([`Overlay`]) and
/// the app's hit testing (pixels from the widget center, y up).
pub mod view_cube {
    use super::*;

    /// Side of the whole widget (cube, arrows and the view menu button):
    /// Onshape's `VIEW_CUBE_SCREEN_SIZE_PX`.
    pub const WIDGET_PX: f32 = 150.0;

    // Onshape's view cube geometry (its WebGL view manipulator), in pixels from
    // the widget's center with y up.
    const G: f64 = WIDGET_PX as f64;
    /// The arrow tips touch the widget's edge.
    const ARROW_OUT: f64 = 0.5 * G;
    /// The geometry's center in the widget (from its top left, y down):
    /// live draws it 1 px under the middle (VC8: every arrow and the cube
    /// 1 px lower than ours at the middle, x exact).
    pub const CENTER: [f64; 2] = [0.5 * G, 0.5 * G + 1.0];
    /// Half of `VIEW_CUBE_ARROW_WIDTH` and `VIEW_CUBE_ARROW_HEIGHT`.
    const ARROW_HALF_W: f64 = 0.05 * G;
    const ARROW_HALF_H: f64 = 0.065 * G;
    /// `VIEW_CUBE_ARROW_ARC_WIDTH`: the roll arcs' thickness.
    const ARC_W: f64 = 0.04 * G;
    /// Half the cube's edge: what's left inside the arrows, less room for the
    /// 12 px axis labels.
    pub const HALF_EDGE: f64 = ((0.5 - 2.0 * 0.05) / 1.6 - 12.0 / G / 1.5) * G;
    /// Each face is a rounded square: white on the near side, a light frame
    /// seen through on the far side (`VIEW_CUBE_FRONT_SIZE`, `_BACK_SIZE`,
    /// `_CORNER_SIZE` of the edge length).
    const FRONT_SIZE: f64 = 0.75;
    const BACK_SIZE: f64 = 0.97;
    const CORNER_SIZE: f64 = 0.2;
    /// The axis lines start `VIEW_CUBE_AXIS_GAP` outside the cube's corner.
    const AXIS_GAP: f64 = 1.0;
    /// Axis parts behind the cube (`VIEW_CUBE_HIDDEN_AXIS_OPACITY`).
    pub const HIDDEN_AXIS_OPACITY: f32 = 0.3;

    /// A clickable region of the cube, identified by the view direction it
    /// stands for: one non-zero component = face, three = corner.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct CubeCell(pub [i8; 3]);

    impl CubeCell {
        pub fn dir(self) -> DVec3 {
            DVec3::new(self.0[0] as f64, self.0[1] as f64, self.0[2] as f64)
        }

        /// Camera rotation for this cell: a face's standard view, or for a
        /// corner Onshape's trimetric view from that side (30° off the side
        /// face nearest the corner's front, 30° above or below the ground).
        pub fn rotation(self) -> DQuat {
            let view = match self.0 {
                [0, -1, 0] => Some(StandardView::Front),
                [0, 1, 0] => Some(StandardView::Back),
                [-1, 0, 0] => Some(StandardView::Left),
                [1, 0, 0] => Some(StandardView::Right),
                [0, 0, 1] => Some(StandardView::Top),
                [0, 0, -1] => Some(StandardView::Bottom),
                _ => None,
            };
            if let Some(v) = view {
                return Camera::rotation_for(v);
            }
            let quarter = match (self.0[0] > 0, self.0[1] > 0) {
                (true, false) => 0.0,
                (true, true) => 1.0,
                (false, true) => 2.0,
                (false, false) => 3.0,
            };
            let t = (quarter * 90.0 + 30.0f64).to_radians();
            let eye = DVec3::new(0.75f64.sqrt() * t.sin(), -(0.75f64.sqrt()) * t.cos(), 0.5 * self.0[2].signum() as f64);
            Camera::look_rotation(-eye, DVec3::Z)
        }
    }

    /// Face labels by outward normal (Onshape: Front looks along +Y).
    pub const FACES: [([i8; 3], &str); 6] = [([0, -1, 0], "Front"), ([0, 1, 0], "Back"), ([-1, 0, 0], "Left"), ([1, 0, 0], "Right"), ([0, 0, 1], "Top"), ([0, 0, -1], "Bottom")];

    /// A face's in-plane axes: its label runs along the first, upright along
    /// the second.
    pub fn face_axes(n: [i8; 3]) -> (DVec3, DVec3) {
        match n {
            [0, -1, 0] => (DVec3::X, DVec3::Z),
            [0, 1, 0] => (DVec3::NEG_X, DVec3::Z),
            [-1, 0, 0] => (DVec3::NEG_Y, DVec3::Z),
            [1, 0, 0] => (DVec3::Y, DVec3::Z),
            [0, 0, 1] => (DVec3::X, DVec3::Y),
            _ => (DVec3::X, DVec3::NEG_Y),
        }
    }

    /// The rotate and roll arrows around the cube. While hovered, an arrow
    /// sits in the owner's hover slot as a pseudo cell ([`Arrow::cell`]).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Arrow {
        Up,
        Down,
        Left,
        Right,
        Cw,
        Ccw,
    }

    /// Marks an arrow's pseudo cell (no cube cell has a component this big).
    const ARROW_TAG: i8 = 9;

    impl Arrow {
        pub const ALL: [Arrow; 6] = [Arrow::Up, Arrow::Down, Arrow::Left, Arrow::Right, Arrow::Cw, Arrow::Ccw];

        pub fn cell(self) -> CubeCell {
            CubeCell([ARROW_TAG, self as i8, 0])
        }

        pub fn of(cell: CubeCell) -> Option<Arrow> {
            if cell.0[0] != ARROW_TAG {
                return None;
            }
            Self::ALL.get(cell.0[1] as usize).copied()
        }

        /// Outline polygons, from the widget center with y up: a triangle on
        /// each side; a roll arrow is an arc band with its head at the outer end.
        pub fn polys(self) -> Vec<Vec<DVec2>> {
            let (o, w) = (ARROW_OUT, ARROW_HALF_W);
            let b = o - ARROW_HALF_H;
            let tri = |p: [[f64; 2]; 3]| vec![p.iter().map(|q| DVec2::from(*q)).collect::<Vec<_>>()];
            let arc = |a0: f64, a1: f64| {
                let (r0, r1) = (o - w - ARC_W / 2.0, o - w + ARC_W / 2.0);
                let at = |k: usize| a0 + (a1 - a0) * k as f64 / 6.0;
                let mut p: Vec<DVec2> = (0..=6).map(|k| DVec2::from_angle(at(k)) * r1).collect();
                p.extend((0..=6).rev().map(|k| DVec2::from_angle(at(k)) * r0));
                p
            };
            let head = |sx: f64| {
                let d = DVec2::new(sx, 1.0) * std::f64::consts::FRAC_1_SQRT_2;
                let q = std::f64::consts::FRAC_PI_4 - ARROW_HALF_H / o;
                vec![d * o, d * (o - 2.0 * w), DVec2::new(sx * q.cos(), q.sin()) * (o - w)]
            };
            use std::f64::consts::PI;
            match self {
                Arrow::Up => tri([[0.0, o], [-w, b], [w, b]]),
                Arrow::Down => tri([[0.0, -o], [w, -b], [-w, -b]]),
                Arrow::Left => tri([[-o, 0.0], [-b, -w], [-b, w]]),
                Arrow::Right => tri([[o, 0.0], [b, w], [b, -w]]),
                Arrow::Cw => vec![head(1.0), arc(0.25 * PI, 0.4 * PI)],
                Arrow::Ccw => vec![head(-1.0), arc(0.6 * PI, 0.75 * PI)],
            }
        }
    }

    /// One visible piece of the cube, in pixels from the widget center with y up.
    #[derive(Clone, Debug)]
    pub struct CubeShape {
        pub cell: CubeCell,
        pub poly: Vec<DVec2>,
        /// A far face's frame (light) rather than a white near face or corner.
        pub frame: bool,
    }

    /// A rounded square in the plane through `c` spanned by `u` and `v`:
    /// half side `a`, corner radius `r`, 9 segments per corner.
    fn rounded_square(c: DVec3, u: DVec3, v: DVec3, a: f64, r: f64) -> Vec<DVec3> {
        let mut out = Vec::with_capacity(40);
        for (k, (su, sv)) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].into_iter().enumerate() {
            let corner = c + u * (su * (a - r)) + v * (sv * (a - r));
            for j in 0..=9 {
                let t = (k as f64 + j as f64 / 9.0) * std::f64::consts::FRAC_PI_2;
                out.push(corner + (u * t.cos() + v * t.sin()) * r);
            }
        }
        out
    }

    /// The cube's visible pieces for a camera rotation, back to front, like
    /// Onshape's back-face-culled mesh: the far faces' frames seen through
    /// the gaps, a white disk under each near corner, the near faces' white
    /// squares.
    /// How far from its centre a corner takes the pointer, in half edges
    /// (its disc is 0.24).
    const CORNER_HIT: f64 = 0.31;

    pub fn cube_shapes(rotation: DQuat) -> Vec<CubeShape> {
        let inv = rotation.inverse();
        let h = HALF_EDGE;
        let (mut far, mut corners, mut near) = (Vec::new(), Vec::new(), Vec::new());
        for (n, _) in FACES {
            let cell = CubeCell(n);
            let z = (inv * cell.dir()).z;
            if z.abs() < 1e-6 {
                continue;
            }
            let (u, v) = face_axes(n);
            let size = if z > 0.0 { FRONT_SIZE } else { BACK_SIZE };
            let poly = rounded_square(cell.dir() * h, u, v, size * h, CORNER_SIZE * 2.0 * h).into_iter().map(|p| (inv * p).truncate()).collect();
            if z > 0.0 {
                near.push(CubeShape { cell, poly, frame: false });
            } else {
                far.push(CubeShape { cell, poly, frame: true });
            }
        }
        for i in 0..8 {
            let s = |bit: i32| if i & bit == 0 { -1i8 } else { 1 };
            let cell = CubeCell([s(1), s(2), s(4)]);
            let d = cell.dir().normalize();
            if (inv * d).z <= 1e-6 {
                continue;
            }
            let center = cell.dir() * (0.8 * h);
            let u = d.cross(DVec3::Z).normalize();
            let v = d.cross(u);
            let poly = (0..36)
                .map(|k| {
                    let t = k as f64 / 36.0 * std::f64::consts::TAU;
                    (inv * (center + (u * t.cos() + v * t.sin()) * (0.24 * h))).truncate()
                })
                .collect();
            corners.push(CubeShape { cell, poly, frame: false });
        }
        far.into_iter().chain(corners).chain(near).collect()
    }

    /// Front-most cube cell under a point (pixels from the widget center, y up).
    /// A visible corner takes the pointer a little beyond its drawn disc
    /// (live 10-27, iso: (1829,156) lights the corner whose disc spans y
    /// 159–169), before the faces around it.
    pub fn cube_hit(rotation: DQuat, p: DVec2) -> Option<CubeCell> {
        let shapes = cube_shapes(rotation);
        let inv = rotation.inverse();
        let corner = shapes.iter().filter(|s| s.cell.0.iter().all(|c| *c != 0)).find(|s| (inv * (s.cell.dir() * (0.8 * HALF_EDGE))).truncate().distance(p) <= CORNER_HIT * HALF_EDGE);
        corner.map(|s| s.cell).or_else(|| shapes.into_iter().rev().find(|s| point_in_poly(p, &s.poly)).map(|s| s.cell))
    }

    /// What's under a widget point: an arrow's pseudo cell or a cube cell.
    pub fn widget_hit(rotation: DQuat, p: DVec2) -> Option<CubeCell> {
        Arrow::ALL.into_iter().find(|a| a.polys().iter().any(|q| point_in_poly(p, q))).map(Arrow::cell).or_else(|| cube_hit(rotation, p))
    }

    pub fn point_in_poly(p: DVec2, poly: &[DVec2]) -> bool {
        let mut inside = false;
        let mut j = poly.len().saturating_sub(1);
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[j]);
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    /// Whether the cube hides a point (pixels, world axes) from a viewer
    /// towards `eye`.
    pub fn behind_cube(p: DVec3, eye: DVec3) -> bool {
        let h = HALF_EDGE;
        let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
        for i in 0..3 {
            if eye[i].abs() < 1e-12 {
                if p[i].abs() > h {
                    return false;
                }
            } else {
                let (a, b) = ((-h - p[i]) / eye[i], (h - p[i]) / eye[i]);
                t0 = t0.max(a.min(b));
                t1 = t1.min(a.max(b));
            }
        }
        t0 < t1
    }

    /// Face label contours in cube space for `name` laid out by the caller:
    /// `polylines` are glyph contours in em units (baseline at y = 0) of a
    /// line `width` em wide, centered on face `n`.
    pub fn face_label(n: [i8; 3], width: f64, polylines: &[Vec<DVec2>]) -> Vec<Vec<DVec3>> {
        let (right, up) = face_axes(n);
        // Ascender height in cube units (a face spans -1..1).
        let asc = 0.4;
        let base = CubeCell(n).dir() - right * (width * asc * 0.5) - up * (asc * 0.36);
        polylines.iter().map(|c| c.iter().map(|q| (base + right * (q.x * asc) + up * (q.y * asc)) * HALF_EDGE).collect()).collect()
    }

    /// Where an axis label goes and how opaque it is: pixels from the widget
    /// center (y up) and alpha (faded as the axis turns end-on, faint behind
    /// the cube).
    pub fn axis_label_places(rotation: DQuat) -> [(DVec2, f32); 3] {
        let inv = rotation.inverse();
        let eye = rotation * DVec3::Z;
        let pieces = cube_shapes(rotation);
        let covered = |p: DVec3| behind_cube(p, eye) && pieces.iter().any(|s| point_in_poly((inv * p).truncate(), &s.poly));
        let origin = DVec3::splat(-(HALF_EDGE + AXIS_GAP));
        let len = 2.0 * HALF_EDGE;
        [DVec3::X, DVec3::Y, DVec3::Z].map(|axis| {
            // Past the tip by half the label's diagonal plus 3 px; faded out
            // as the axis turns end-on (8° to 2°), faint behind the cube.
            let at = origin + axis * (len + (7.5f64.hypot(12.0) / 2.0 + 3.0));
            let t = (((eye.dot(axis).abs().min(1.0).acos()).to_degrees() - 2.0) / 6.0).clamp(0.0, 1.0);
            let fade = (t * t * (3.0 - 2.0 * t)) as f32;
            let alpha = if covered(at) { fade.min(HIDDEN_AXIS_OPACITY) } else { fade };
            ((inv * at).truncate(), alpha)
        })
    }

    /// The triad's line runs: (from, to, axis index, hidden), pixels from
    /// the widget center (y up). What the cube hides shows through faintly.
    pub fn axis_runs(rotation: DQuat) -> Vec<(DVec2, DVec2, usize, bool)> {
        let inv = rotation.inverse();
        let eye = rotation * DVec3::Z;
        let to_view = |w: DVec3| (inv * w).truncate();
        let pieces = cube_shapes(rotation);
        // Hidden = behind the cube and under something drawn: the rounded
        // corners leave the axes visible next to them (live).
        let covered = |p: DVec3| behind_cube(p, eye) && pieces.iter().any(|s| point_in_poly(to_view(p), &s.poly));
        let origin = DVec3::splat(-(HALF_EDGE + AXIS_GAP));
        let mut runs = Vec::new();
        for (i, axis) in [DVec3::X, DVec3::Y, DVec3::Z].into_iter().enumerate() {
            let len = 2.0 * HALF_EDGE;
            let steps = 48;
            let hidden = |k: usize| covered(origin + axis * (len * (k as f64 + 0.5) / steps as f64));
            let mut k0 = 0;
            while k0 < steps {
                let h = hidden(k0);
                let mut k1 = k0 + 1;
                while k1 < steps && hidden(k1) == h {
                    k1 += 1;
                }
                let at = |k: usize| to_view(origin + axis * (len * k as f64 / steps as f64));
                runs.push((at(k0), at(k1), i, h));
                k0 = k1;
            }
        }
        runs
    }

    /// The widget drawn for a camera rotation, like the GPUI widget did:
    /// arrows, cube pieces, face labels, axis lines (hidden parts first),
    /// axis labels.
    pub(super) fn build(cube: &ViewCube, rotation: DQuat, dpr: f32, out: &mut OverlayDraw) {
        let inv = rotation.inverse();
        let c = cube.colors;
        let lit = |cell: CubeCell, base: Rgba| if cube.hover == Some(cell) { c.highlight } else { base };
        // Widget origin (top-left) in logical pixels within the frame.
        let o = DVec2::new(cube.origin[0] as f64 / dpr as f64, cube.origin[1] as f64 / dpr as f64);
        let at = |v: DVec2| DVec2::new(o.x + (CENTER[0] + v.x) as f32 as f64, o.y + (CENTER[1] - v.y) as f32 as f64);
        let place = |polys: &[Vec<DVec2>]| -> Vec<Vec<DVec2>> { polys.iter().map(|p| p.iter().map(|q| at(*q)).collect()).collect() };
        for a in Arrow::ALL {
            out.fill(&place(&a.polys()), lit(a.cell(), c.frame), dpr);
        }
        // A hovered piece keeps its own fill under the translucent highlight
        // (live #bce2f7 = #90cef1 at 0.6 over the white face; drawn alone the
        // back frame showed through: qa5 P1-15, (177,217,241)).
        for s in cube_shapes(rotation) {
            let poly = place(std::slice::from_ref(&s.poly));
            out.fill(&poly, if s.frame { c.frame } else { c.face }, dpr);
            if cube.hover == Some(s.cell) {
                out.fill(&poly, c.highlight, dpr);
            }
        }
        for (n, contours) in &cube.face_labels {
            if (inv * CubeCell(*n).dir()).z <= 1e-6 {
                continue;
            }
            let polys: Vec<Vec<DVec2>> = contours.iter().map(|p| p.iter().map(|q| (inv * *q).truncate()).collect()).collect();
            out.fill(&place(&polys), c.label, dpr);
        }
        let runs = axis_runs(rotation);
        for hidden_pass in [true, false] {
            for (a, b, axis, hidden) in &runs {
                if *hidden != hidden_pass {
                    continue;
                }
                let mut color = c.axes[*axis];
                if *hidden {
                    color[3] = HIDDEN_AXIS_OPACITY;
                }
                out.stroke(at(*a), at(*b), 1.0, color, dpr);
            }
        }
        if let Some(labels) = &cube.axis_labels {
            for ((v, alpha), label) in axis_label_places(rotation).into_iter().zip(labels) {
                if alpha <= 0.0 {
                    continue;
                }
                // The label's 20x16 box is centered on the point, laid out like
                // GPUI's layout does it (gpui `taffy.rs`): its offset in the
                // widget is rounded to device pixels before layout (half toward
                // zero), the text element centered in it, and the text's
                // absolute position rounded again. The glyph then goes where
                // `Window::paint_glyph` puts it.
                // In f32, as the widget computes it (`left(px(CENTER[0] as f32 + v.x as f32 - 10.0))`).
                let local = [CENTER[0] as f32 + v.x as f32 - 10.0, CENTER[1] as f32 - v.y as f32 - 8.0];
                let bx = cube.origin[0] + round_half_toward_zero(local[0] * dpr);
                let by = cube.origin[1] + round_half_toward_zero(local[1] * dpr);
                let tx = round_half_toward_zero(bx + label.text_x);
                let ty = round_half_toward_zero(by);
                let (gx, gy) = (tx + label.glyph[0], ty + label.glyph[1]);
                let n = SUBPIXEL_VARIANTS_X as f32;
                let qx = round_half_toward_zero(gx * n) / n;
                let variant = ((qx.fract() * n) as usize).min(SUBPIXEL_VARIANTS_X - 1);
                let mask = &label.masks[variant];
                let top_left = [qx.trunc() as i32 + mask.offset[0], round_half_toward_zero(gy) as i32 + mask.offset[1]];
                let mut color = label.color;
                color[3] *= alpha;
                out.grow(top_left[0] as f32, top_left[1] as f32);
                out.grow((top_left[0] + mask.width as i32) as f32, (top_left[1] + mask.height as i32) as f32);
                out.masks.push((mask.clone(), top_left, color));
            }
        }
    }
}
