//! Drawing editor: shaded views (view menu › Show/hide › Show shaded view;
//! live behaviours "Shaded drawing views"): the view's parts rendered
//! offscreen along the view's line of sight (orthographic, the document
//! thumbnail look) instead of its edges, which show only while the view is
//! hovered (plain) or selected (selection colour); the background stays
//! transparent. A flat pattern view shows its flat plate, with the bend
//! lines and notes on top. PDF embeds the picture (at most 1024 px), DXF
//! keeps the edges only. Children don't inherit it, except details (live
//! request 1649). Sections render their cut bodies (the cut face shaded,
//! the hatch drawn on top), details their parent's picture within the
//! circle. Views with crops or breaks, and polygon details, stay unshaded
//! for now.

use crate::drawing_view::*;
use gpui_kit::*;
use noshape_model::drawing::views::{self, ViewFrame};
use noshape_model::drawing::{RenderStyle, ViewId, ViewKind};
use noshape_render::{Drawable, ImageData, ImageHandle, Instance, LinesHandle};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Live: the PDF picture is at most 1024 px on its long side.
const MAX_PX: f64 = 1024.0;
/// Picture pixels per sheet millimetre (before the cap).
const PX_PER_MM: f64 = 8.0;

/// A picture for export: its view, lower-left corner and size on the
/// sheet, PNG (with alpha).
pub type ShadedExport = (ViewId, [f64; 2], [f64; 2], Arc<Vec<u8>>);

/// One view's picture: what it was made for, its texture and PNG.
pub struct ShadedImage {
    key: u64,
    pub handle: ImageHandle,
    pub png: Arc<Vec<u8>>,
}

#[derive(Default)]
pub struct ShadedViews {
    pub images: HashMap<ViewId, ShadedImage>,
    busy: HashSet<ViewId>,
    /// The edges of the views with a picture: with their pick ids (picking,
    /// and drawn while selected) and without (drawn while hovered).
    pub edges: Vec<(ViewId, LinesHandle, LinesHandle)>,
}

impl DrawingEditor {
    /// Render the pictures of the sheet's shaded views that changed (called
    /// after each refresh).
    pub fn sync_shaded(&mut self, cx: &mut Context<Self>) {
        let d = self.display().clone();
        let Some(sheet) = d.sheets.get(self.sheet) else { return };
        let wanted: Vec<ViewId> = sheet
            .views
            .iter()
            .filter(|v| v.style == RenderStyle::Shaded && v.crop.is_none() && v.breaks.is_empty() && v.detail_profile.is_none())
            .filter(|v| matches!(v.kind, ViewKind::Base { .. } | ViewKind::Projected { .. } | ViewKind::Auxiliary { .. } | ViewKind::Section { .. } | ViewKind::AlignedSection { .. } | ViewKind::Detail { .. }))
            .map(|v| v.id)
            .collect();
        self.shaded.images.retain(|id, _| wanted.contains(id));
        for id in wanted {
            let Some(g) = self.geoms.get(&id).cloned() else { continue };
            let Some(frame) = g.frame else { continue };
            let Some((key, part)) = views::view_source(sheet, id) else { continue };
            let Some(src) = self.sources.get(&key).cloned() else { continue };
            let Some(pl) = self.placement(id) else { continue };
            let flat = sheet.view(id).is_some_and(|v| v.flat);
            let (lo, hi) = (g.min, g.max);
            if !(hi[0] > lo[0] && hi[1] > lo[1]) {
                continue;
            }
            let mut h = std::collections::hash_map::DefaultHasher::new();
            src.hash.hash(&mut h);
            format!("{:?}{:?}{lo:?}{hi:?}{part:?}{}{flat}", frame.dir, frame.up, pl.scale).hash(&mut h);
            // Cuts and detail circles.
            views::view_key(&d, sheet, id, &self.sources, false).hash(&mut h);
            let k = h.finish();
            if self.shaded.images.get(&id).is_some_and(|i| i.key == k) || self.shaded.busy.contains(&id) {
                continue;
            }
            self.shaded.busy.insert(id);
            let (w_mm, h_mm) = ((hi[0] - lo[0]) * pl.scale * 1000.0, (hi[1] - lo[1]) * pl.scale * 1000.0);
            let f = (MAX_PX / (w_mm.max(h_mm) * PX_PER_MM)).min(1.0) * PX_PER_MM;
            let (wp, hp) = (((w_mm * f).round() as u32).max(8), ((h_mm * f).round() as u32).max(8));
            // A detail keeps the part of the picture inside its circle.
            let circle = g.clip;
            let keep = move |u: f64, v: f64| {
                circle.is_none_or(|(c, r)| {
                    let (x, y) = (lo[0] + u * (hi[0] - lo[0]), hi[1] - v * (hi[1] - lo[1]));
                    (x - c[0]).powi(2) + (y - c[1]).powi(2) <= r * r
                })
            };
            let job = Job {
                gpu: self.gpu.clone(),
                src,
                part,
                shapes: g.cut_shapes.clone(),
                flat,
                frame,
                lo,
                hi,
                size: (wp, hp),
            };
            cx.spawn(async move |this, cx| {
                let png = job.render().await;
                let _ = this.update(cx, |this: &mut Self, cx| {
                    this.shaded.busy.remove(&id);
                    if let Some((img, png)) = png.as_deref().and_then(|p| decode(p, &keep)) {
                        let handle = this.gpu.uploader().upload_image(&img);
                        this.shaded.images.insert(id, ShadedImage { key: k, handle, png: Arc::new(png) });
                        this.geometry_dirty = true;
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    /// The pictures as scene quads over their views, behind the sheet's ink
    /// (z −0.5: images are drawn after the text meshes and depth-tested, so
    /// the bend notes and other texts over a picture stay visible; the
    /// paper is at −1).
    pub fn shaded_instances(&self) -> Vec<Instance> {
        self.shaded
            .images
            .iter()
            .filter_map(|(id, img)| {
                let (o, x, y) = self.shaded_frame(*id)?;
                let m = glam::Mat4::from_cols(
                    glam::Vec4::new(x[0] as f32, x[1] as f32, 0.0, 0.0),
                    glam::Vec4::new(y[0] as f32, y[1] as f32, 0.0, 0.0),
                    glam::Vec4::Z,
                    glam::Vec4::new(o[0] as f32, o[1] as f32, -0.5, 1.0),
                );
                Some(Instance::new(Drawable::Image(img.handle.clone())).with_transform(m).layer(-2))
            })
            .collect()
    }

    /// A shaded view's edges: pickable but hidden; plain ink while the view
    /// is hovered, the selection colour while it is selected.
    pub fn shaded_edge_instances(&self) -> Vec<Instance> {
        let hovered = self.hover.and_then(|h| self.sel_of_pick(h));
        let mut out = Vec::new();
        for (id, picked, plain) in &self.shaded.edges {
            out.push(Instance::new(Drawable::Lines(picked.clone())).per_primitive_color().pick_only());
            if self.selection.contains(&Sel::View(*id)) {
                out.push(Instance::new(Drawable::Lines(picked.clone())).per_primitive_color());
            } else if hovered == Some(Sel::View(*id)) {
                out.push(Instance::new(Drawable::Lines(plain.clone())).per_primitive_color().unpickable());
            }
        }
        out
    }

    /// The pictures for PDF / SVG / raster export: (lower-left corner,
    /// size, PNG) on the sheet (rotated views keep their axis-aligned box).
    pub fn shaded_exports(&self) -> Vec<ShadedExport> {
        self.shaded
            .images
            .iter()
            .filter_map(|(id, img)| {
                let (o, x, y) = self.shaded_frame(*id)?;
                Some((*id, o, [x[0] + y[0], x[1] + y[1]], img.png.clone()))
            })
            .collect()
    }

    /// Sheet corner and edge vectors of view `id`'s picture (its geometry box).
    fn shaded_frame(&self, id: ViewId) -> Option<([f64; 2], [f64; 2], [f64; 2])> {
        let g = self.geoms.get(&id)?;
        let pl = self.placement(id)?;
        let (lo, hi) = (g.min, g.max);
        let o = pl.to_sheet(lo);
        let a = pl.to_sheet([hi[0], lo[1]]);
        let b = pl.to_sheet([lo[0], hi[1]]);
        Some((o, [a[0] - o[0], a[1] - o[1]], [b[0] - o[0], b[1] - o[1]]))
    }
}

/// One picture to render: the parts (or `part`) of `src` seen along
/// `frame`, framing exactly the view box `lo`..`hi` (view coordinates), at
/// `size` pixels. `flat`: the part's flat pattern, seen from its reference
/// side (the flat view's coordinates are the flat's plane coordinates).
struct Job {
    gpu: crate::gpu::Gpu,
    src: views::SourceModel,
    part: Option<noshape_model::BodyId>,
    /// A cut view's bodies as cut (else the source's).
    shapes: Vec<(noshape_model::BodyId, noshape_kernel::Shape)>,
    flat: bool,
    frame: ViewFrame,
    lo: [f64; 2],
    hi: [f64; 2],
    size: (u32, u32),
}

impl Job {
    async fn render(self) -> Option<Vec<u8>> {
        use noshape_model::display::{BodyDisplay, BodyKind, StudioDisplay};
        let Job {
            gpu,
            src,
            part,
            shapes,
            flat,
            mut frame,
            lo,
            hi,
            size: (w, h),
        } = self;
        let part = part.as_ref();
        // The flat plate (the flat region made solid), placed in its plane.
        let mut origin = glam::DVec3::ZERO;
        let mut plate = None;
        if flat {
            let fp = noshape_model::sheetmetal::flat::flat_pattern_for(&src.context, part?).ok()?;
            let solid = noshape_kernel::extrude(fp.shape.as_ref()?, fp.frame.normal * -fp.thickness).ok()?;
            frame = ViewFrame::new(-fp.frame.unit_normal(), fp.frame.unit_y());
            origin = fp.frame.origin;
            plate = Some(solid.shape);
        }
        let mut display = StudioDisplay::default();
        let (mut d0, mut d1) = (f64::MAX, f64::MIN);
        let items: Vec<(_, noshape_kernel::Shape)> = if shapes.is_empty() {
            src.context.bodies.iter().filter(|b| matches!(b.kind, BodyKind::Solid | BodyKind::Surface) && part.is_none_or(|p| *p == b.id) && (part.is_some() || b.kind == BodyKind::Solid)).map(|b| (b, plate.clone().unwrap_or_else(|| b.shape.clone()))).collect()
        } else {
            shapes.iter().filter_map(|(id, s)| src.context.bodies.iter().find(|b| &b.id == id).map(|b| (b, s.clone()))).collect()
        };
        for (b, shape) in &items {
            let Some(mesh) = noshape_model::regen::tessellate_shape(shape, 0.5) else { continue };
            if let Ok(bb) = shape.bounding_box(false) {
                for c in [bb.min, bb.max] {
                    d0 = d0.min(c.dot(frame.dir));
                    d1 = d1.max(c.dot(frame.dir));
                }
            }
            let color = noshape_model::regen::PART_COLORS[b.color % noshape_model::regen::PART_COLORS.len()];
            display.bodies.push(BodyDisplay {
                id: b.id.clone(),
                name: b.name.clone(),
                kind: b.kind,
                color,
                hidden: false,
                mesh: Arc::new(mesh),
                coarser: Vec::new(),
                levels_from: 0,
                face_refs: vec![],
                edge_refs: vec![],
                vertex_refs: vec![],
                bounds: None,
                composite: None,
                is_mesh: false,
                face_colors: vec![],
            });
        }
        if display.bodies.is_empty() {
            return None;
        }
        let opts = crate::scene_builder::SceneOptions {
            show_planes: false,
            show_sketches: false,
            show_origin: false,
            ..Default::default()
        };
        let built = crate::scene_builder::build(&display, &opts, &mut crate::scene_builder::GpuCache::default(), gpu.uploader());
        let mut camera = noshape_render::Camera {
            rotation: noshape_render::Camera::look_rotation(frame.dir, frame.up),
            projection: noshape_render::Projection::Orthographic,
            ..Default::default()
        };
        let (cx, cy) = ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
        camera.target = origin + frame.right * cx + frame.up * cy + frame.dir * ((d0 + d1) / 2.0 - origin.dot(frame.dir));
        let half = (hi[1] - lo[1]) / 2.0;
        camera.distance = (half / (camera.fov_y * 0.5).tan()).max(1e-6);
        let style = noshape_render::Style {
            show_edges: false,
            ..crate::start_thumbnail::thumbnail_style()
        };
        gpu.render_png(Arc::new(built.scene), camera, style, w, h).await.ok()
    }
}

/// PNG → RGBA image for the scene, and the PNG again with the white
/// background transparent (live: the picture covers only the part), and
/// the pixels `keep` rejects (at their (0..1, 0..1 from the top) position).
fn decode(png: &[u8], keep: &dyn Fn(f64, f64) -> bool) -> Option<(ImageData, Vec<u8>)> {
    let mut rgba = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?.to_rgba8();
    let (w, h) = rgba.dimensions();
    for (x, y, px) in rgba.enumerate_pixels_mut() {
        if (px[0] >= 250 && px[1] >= 250 && px[2] >= 250) || !keep((x as f64 + 0.5) / w as f64, (y as f64 + 0.5) / h as f64) {
            px.0 = [0; 4];
        }
    }
    let mut out = std::io::Cursor::new(Vec::new());
    rgba.write_to(&mut out, image::ImageFormat::Png).ok()?;
    let (width, height) = rgba.dimensions();
    Some((
        ImageData {
            corners: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            width,
            height,
            rgba: rgba.into_raw(),
        },
        out.into_inner(),
    ))
}
