//! Converts a [`StudioDisplay`] into a render [`Scene`] and a [`PickMap`]
//! translating pick ids back into persistent references.
//!
//! Geometry buffers are cached per `Arc<MeshBuffers>` and use local pick ids,
//! so a regeneration that keeps a body's tessellation re-uses its GPU data.
//! Hover and selection only rewrite the per-id state buffer, except for
//! invisible-but-pickable elements (vertices, sketch regions) whose highlight
//! overlays are rebuilt on demand.

use crate::palette as theme;
use glam::DVec3;
use noshape_model::display::{BodyKind, SharedMesh, SketchEntityState, StudioDisplay};
use noshape_model::reference::{FeatureId, Reference};
use noshape_render::renderer::Uploader;
use noshape_render::{Drawable, ImageData, ImageHandle, Instance, LineData, LineSegment, LinesHandle, MeshData, MeshHandle, PointData, PointSprite, PointsHandle, Scene, SilhouetteData, SilhouetteSegment, state};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PickKind {
    Face,
    Edge,
    Vertex,
    Plane,
    Origin,
    SketchCurve,
    SketchPoint,
    SketchRegion,
    Connector,
}

impl PickKind {
    /// Higher wins when several candidates are under the cursor radius.
    pub fn priority(self) -> i32 {
        match self {
            PickKind::Vertex | PickKind::SketchPoint | PickKind::Origin | PickKind::Connector => 3,
            PickKind::Edge | PickKind::SketchCurve => 2,
            PickKind::SketchRegion => 1,
            PickKind::Face | PickKind::Plane => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PickTarget {
    pub reference: Reference,
    pub kind: PickKind,
    /// World position hint (vertex position, region centroid...).
    pub anchor: Option<DVec3>,
}

#[derive(Clone, Debug, Default)]
pub struct PickMap {
    targets: Vec<PickTarget>,
    by_ref: HashMap<Reference, Vec<u32>>,
}

impl PickMap {
    fn push(&mut self, t: PickTarget) -> u32 {
        self.targets.push(t);
        let id = self.targets.len() as u32;
        self.by_ref.entry(self.targets[id as usize - 1].reference.clone()).or_default().push(id);
        id
    }
    /// Reserve `n` consecutive ids, returning the base such that local id
    /// `k` (1-based) maps to `base + k`.
    fn reserve(&mut self, items: impl IntoIterator<Item = PickTarget>) -> u32 {
        let base = self.targets.len() as u32;
        for t in items {
            self.push(t);
        }
        base
    }
    pub fn get(&self, id: u32) -> Option<&PickTarget> {
        if id == 0 { None } else { self.targets.get(id as usize - 1) }
    }
    pub fn ids_for(&self, r: &Reference) -> &[u32] {
        self.by_ref.get(r).map(|v| v.as_slice()).unwrap_or(&[])
    }
    pub fn len(&self) -> usize {
        self.targets.len()
    }
    /// Ids of the targets `f` accepts.
    pub fn ids_where<'a>(&'a self, f: impl Fn(&PickTarget) -> bool + 'a) -> impl Iterator<Item = u32> + 'a {
        self.targets.iter().enumerate().filter(move |(_, t)| f(t)).map(|(i, _)| i as u32 + 1)
    }
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
    pub fn priority_fn(&self) -> Arc<dyn Fn(u32) -> i32 + Send + Sync> {
        self.priority_fn_filtered(|_| true)
    }

    /// Like [`Self::priority_fn`], but kinds `allow` rejects are never
    /// picked or hovered (a dialog field's selection filter, Onshape).
    pub fn priority_fn_filtered(&self, allow: impl Fn(PickKind) -> bool) -> Arc<dyn Fn(u32) -> i32 + Send + Sync> {
        let prio: Vec<i32> = self.targets.iter().map(|t| if allow(t.kind) { t.kind.priority() } else { i32::MIN }).collect();
        Arc::new(move |id| if id == 0 { -1 } else { prio.get(id as usize - 1).copied().unwrap_or(0) })
    }
}

struct CachedBody {
    mesh: MeshHandle,
    edges: LinesHandle,
    vertices: PointsHandle,
    /// Mesh faces' triangle sides (live G35), when the body has mesh faces.
    mesh_lines: Option<LinesHandle>,
    /// Silhouette candidates of its curved faces ([`silhouette_edges`]).
    silhouettes: Option<LinesHandle>,
    /// The edges simplified to each coarser line level
    /// ([`noshape_render::scene::LINE_LOD_TOLERANCES`]).
    edge_lods: Vec<LinesHandle>,
}

/// GPU caches that survive across rebuilds.
#[derive(Default)]
pub struct GpuCache {
    bodies: HashMap<usize, (std::sync::Weak<noshape_model::display::MeshBuffers>, Arc<CachedBody>)>,
    /// Colored body meshes keyed by (mesh, colors) pointers.
    colored: HashMap<(usize, usize), ColoredMesh>,
    /// Hidden-edge overlays (dashed, on top) keyed by mesh pointer.
    hidden_edges: HashMap<usize, (std::sync::Weak<noshape_model::display::MeshBuffers>, LinesHandle)>,
    /// Edges without the tangent ones (and the tangent ones dash-dot) for
    /// the Tangent edges phantom/removed modes, keyed by mesh pointer.
    tangent: HashMap<usize, (std::sync::Weak<noshape_model::display::MeshBuffers>, TangentLines)>,
    /// Sketch image textures keyed by their (shared) encoded data.
    images: HashMap<usize, (std::sync::Weak<str>, Option<ImageHandle>)>,
    /// Boundary (laminar) edges for "Highlight boundary edges": solid and
    /// dashed-on-top lines, keyed by mesh pointer (`None`: the body has none).
    boundary: HashMap<usize, (std::sync::Weak<noshape_model::display::MeshBuffers>, Option<BoundaryLines>)>,
    /// The unit cube of the bodies' lowest level of detail
    /// ([`Instance::lod_box`]).
    lod_box: Option<MeshHandle>,
}


/// Boundary edges: solid (visible) and dashed on top (hidden) lines.
type BoundaryLines = (LinesHandle, LinesHandle);
/// Per-body per-vertex sRGB colors (face appearances).
pub type VertexColors = Arc<HashMap<noshape_model::BodyId, Arc<Vec<[u8; 4]>>>>;
/// A per-vertex-colored mesh upload and the colors it was made from.
type ColoredMesh = (std::sync::Weak<Vec<[u8; 4]>>, MeshHandle);

#[derive(Clone)]
struct TangentLines {
    sharp: LinesHandle,
    phantom: LinesHandle,
}

/// Onshape's "Tangent edges" camera-menu choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TangentEdges {
    #[default]
    Visible,
    Phantom,
    Removed,
}

impl TangentEdges {
    pub const ALL: [TangentEdges; 3] = [TangentEdges::Visible, TangentEdges::Phantom, TangentEdges::Removed];
    pub fn label(self) -> &'static str {
        match self {
            TangentEdges::Visible => "Tangent edges visible",
            TangentEdges::Phantom => "Tangent edges phantom",
            TangentEdges::Removed => "Tangent edges removed",
        }
    }
}

/// What is highlighted / filtered.
#[derive(Clone, Default)]
pub struct SceneOptions {
    pub show_planes: bool,
    pub show_sketches: bool,
    pub show_origin: bool,
    /// Sketch being edited (drawn with constraint-state colors, others dimmed).
    pub editing_sketch: Option<FeatureId>,
    /// Bodies rendered dimmed (e.g. features below the rolled-back bar preview).
    pub transparent_bodies: bool,
    /// Solid bodies drawn light gray instead of their colour (Interference
    /// detection results, live analysis/82: `#d1d6da`, see-through).
    pub neutral_bodies: bool,
    pub display_mode: DisplayMode,
    /// Section view clipping planes (normal, offset along normal), up to
    /// three; geometry is cut away where all of them clip it.
    pub section: Vec<(glam::DVec3, f64)>,
    /// The section plane being edited (drawn selected, see `Scene::clip_selected`).
    pub section_selected: Option<usize>,
    /// Per-vertex colors (sRGB) replacing a body's color: analysis maps
    /// and face appearances. Indexed like the body's mesh positions.
    pub vertex_colors: Option<VertexColors>,
    /// Draw edges hidden behind faces as dashed lines.
    pub hidden_edges_visible: bool,
    /// How tangent (smooth) edges are drawn.
    pub tangent_edges: TangentEdges,
    /// Parts the section view leaves whole.
    pub section_skip: Vec<noshape_model::BodyId>,
    /// Feature preview bodies: shaded with the preview lighting.
    pub preview_bodies: Vec<noshape_model::BodyId>,
    /// Bodies drawn for display only: not pickable and never highlighted
    /// (a Transform's moved preview, whose original is the selectable ghost).
    pub display_only: Vec<noshape_model::BodyId>,
    /// Onshape's "Highlight boundary edges": laminar edges in red, solid
    /// where visible and dashed where hidden.
    pub highlight_boundary: bool,
    /// Graphics preferences (line widths, sketch colors).
    pub graphics: crate::graphics_prefs::GraphicsPrefs,
    /// Zebra stripes on the parts (analysis tool).
    pub zebra: Option<noshape_render::Zebra>,
    /// Reflection analysis: the parts mirror an environment map.
    pub reflection: Option<noshape_render::Reflection>,
    /// Meshes drawn instead of a part's (Thickness analysis: refined and
    /// colored per vertex, with the part's face pick ids).
    pub replaced_meshes: Option<Arc<HashMap<noshape_model::BodyId, noshape_render::MeshHandle>>>,
    /// Isolate's other parts below 10 % transparency (live ui/iso-901): drawn
    /// solid but washed out, faces and edges mixed toward white by
    /// [`ISOLATE_GHOST`] in sRGB.
    pub ghost_bodies: Vec<noshape_model::BodyId>,
    /// Isolate's planes (live ui/iso-901): no fill, the outline at
    /// [`FAINT_PLANE_ALPHA`] of its opacity (172 → 252 over white).
    pub faint_planes: bool,
}

/// See [`SceneOptions::faint_planes`].
pub const FAINT_PLANE_ALPHA: f32 = 0.04;

/// How far Isolate's ghosts are mixed toward white (live iso-900 → iso-901:
/// 160 → 234, 107 → 220, 209 → 246, i.e. 0.78 of the way, every face).
pub const ISOLATE_GHOST: f32 = 0.78;

/// A linear color mixed toward white by `k` in sRGB (see [`ISOLATE_GHOST`]).
fn ghost_color(c: [f32; 4], k: f32) -> [f32; 4] {
    let ch = |x: f32| {
        let s = if x <= 0.003_130_8 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
        let s = s + (1.0 - s) * k;
        if s <= 0.040_45 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    [ch(c[0]), ch(c[1]), ch(c[2]), c[3]]
}

/// Viewport render style (Onshape's display modes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DisplayMode {
    #[default]
    ShadedWithEdges,
    Shaded,
    HiddenLine,
    /// Onshape "Translucent": every part see-through.
    Translucent,
}

pub struct BuiltScene {
    pub scene: Scene,
    pub picks: PickMap,
    /// Index range of static instances; overlays are appended after.
    pub static_instances: usize,
}

const EDGE_COLOR: u32 = theme::viewport::EDGE;
/// Mesh parts' triangle lines (live G35, part-studio/551).
pub const MESH_LINE_COLOR: u32 = 0x464646;

/// Pick base of display-only bodies: beyond any scene's pick ids.
const DISPLAY_ONLY_PICK_BASE: u32 = 1 << 30;

pub fn build(display: &StudioDisplay, opts: &SceneOptions, cache: &mut GpuCache, up: &Uploader) -> BuiltScene {
    let _t = noshape_render::perf::scope("s.build");
    let mut scene = Scene::default();
    let mut picks = PickMap::default();

    // ---------------------------------------------------------------- bodies
    cache.bodies.retain(|_, (w, _)| w.strong_count() > 0);
    cache.colored.retain(|_, (w, _)| w.strong_count() > 0);
    cache.hidden_edges.retain(|_, (w, _)| w.strong_count() > 0);
    cache.tangent.retain(|_, (w, _)| w.strong_count() > 0);
    cache.images.retain(|_, (w, _)| w.strong_count() > 0);
    let mut bodies = 0u32;
    for b in display.bodies.iter().filter(|b| !b.hidden) {
        let colors = opts.vertex_colors.as_ref().and_then(|m| m.get(&b.id)).filter(|c| c.len() == b.mesh.positions.len());
        let replaced = opts.replaced_meshes.as_ref().and_then(|m| m.get(&b.id));
        // Pattern copies sharing one mesh (`SharedMesh`): uploaded once and
        // drawn placed by their transforms, so the renderer draws all the
        // copies with one instanced call.
        let shared = SharedMesh::of(display, b).filter(|_| colors.is_none() && replaced.is_none() && !noshape_render::renderer::instancing_disabled());
        let display_only = opts.display_only.contains(&b.id);
        let first_instance = scene.instances.len();
        // Display-only bodies get pick ids past the highlight states (so no
        // state ever applies to them) and draw unpickable.
        let (face_base, edge_base, vert_base) = if display_only {
            (DISPLAY_ONLY_PICK_BASE, DISPLAY_ONLY_PICK_BASE, DISPLAY_ONLY_PICK_BASE)
        } else {
            (
                picks.reserve(b.face_refs.iter().map(|r| PickTarget { reference: r.clone(), kind: PickKind::Face, anchor: None })),
                picks.reserve(b.edge_refs.iter().map(|r| PickTarget { reference: r.clone(), kind: PickKind::Edge, anchor: None })),
                picks.reserve(b.vertex_refs.iter().enumerate().map(|(i, r)| PickTarget {
                    reference: r.clone(),
                    kind: PickKind::Vertex,
                    anchor: b.mesh.vertices.get(i).map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)),
                })),
            )
        };
        let mut color = b.color;
        if opts.neutral_bodies && b.kind == BodyKind::Solid {
            // The overlap shows through nearly unchanged (live analysis/83).
            color = theme::lin(0xd1d6da);
            color[3] = 0.2;
        }
        if opts.transparent_bodies || b.kind == BodyKind::Surface {
            color[3] = color[3].min(if b.kind == BodyKind::Surface { 0.85 } else { 0.35 });
        }
        // Onshape's display levels, coarsest first (`BodyDisplay::coarser`):
        // each level's instances draw only when the renderer picks that
        // level for the body (`Instance::mesh_lod`).
        let finest = shared.map_or(&b.mesh, |s| &s.mesh);
        let coarser: &[Arc<noshape_model::display::MeshBuffers>] = if colors.is_some() || replaced.is_some() { &[] } else { shared.map_or(&b.coarser[..], |s| &s.coarser[..]) };
        let levels: Vec<&Arc<noshape_model::display::MeshBuffers>> = coarser.iter().chain(std::iter::once(finest)).collect();
        let mut finest_cached = None;
        for (li, &gmesh) in levels.iter().enumerate() {
            let is_finest = li + 1 == levels.len();
            let level_first = scene.instances.len();
            let key = Arc::as_ptr(gmesh) as usize;
            let cached = match cache.bodies.get(&key) {
                Some((_, c)) => c.clone(),
                None => {
                    let c = Arc::new(upload_body(gmesh, up));
                    cache.bodies.insert(key, (Arc::downgrade(gmesh), c.clone()));
                    c
                }
            };
            if b.kind != BodyKind::Wire {
                let mesh = match colors {
                    _ if replaced.is_some() => Instance::new(Drawable::Mesh(replaced.cloned().expect("checked"))).pick_base(face_base).per_primitive_color(),
                    Some(c) => {
                        let ckey = (Arc::as_ptr(&b.mesh) as usize, Arc::as_ptr(c) as usize);
                        let handle = match cache.colored.get(&ckey) {
                            Some((_, h)) => h.clone(),
                            None => {
                                let pick_ids = face_pick_ids(&b.mesh);
                                let h = up.upload_mesh_colored(&b.mesh.positions, &b.mesh.normals, &pick_ids, c, &b.mesh.indices);
                                cache.colored.insert(ckey, (Arc::downgrade(c), h.clone()));
                                h
                            }
                        };
                        Instance::new(Drawable::Mesh(handle)).pick_base(face_base).per_primitive_color()
                    }
                    None => Instance::new(Drawable::Mesh(cached.mesh.clone())).pick_base(face_base),
                };
                let mesh = match opts.display_mode {
                    DisplayMode::ShadedWithEdges | DisplayMode::Shaded => mesh.with_color(color).zebra(opts.zebra.filter(|_| replaced.is_none())).reflection(opts.reflection.clone().filter(|_| replaced.is_none())),
                    // Onshape (part-studio/62): the shaded part at ~35% in sRGB,
                    // which is ~0.48 in the renderer's linear blending.
                    DisplayMode::Translucent => mesh.with_color([color[0], color[1], color[2], color[3].min(0.48)]),
                    // Faces drawn flat in the background color hide what is behind
                    // them (live part-studio/61: white, unlit, only the edges show).
                    DisplayMode::HiddenLine => mesh.with_color(theme::lin(0xffffff)).unlit(),
                };
                // (The renderer draws a preview's back faces under the datum
                // planes and picked regions and its front faces over them.)
                let mesh = if opts.preview_bodies.contains(&b.id) { mesh.preview() } else { mesh };
                let mesh = if opts.ghost_bodies.contains(&b.id) { mesh.ghost(ISOLATE_GHOST) } else { mesh };
                // Parts excluded from the section view are not cut.
                scene.instances.push(if opts.section_skip.contains(&b.id) { mesh.ignore_clip() } else { mesh });
            }
            let edge_color = match (b.kind, opts.display_mode) {
                (BodyKind::Wire, _) => theme::lin(theme::viewport::EDGE),
                (_, DisplayMode::HiddenLine) => theme::lin(theme::viewport::EDGE_UNSHADED),
                _ => theme::lin(EDGE_COLOR),
            };
            let edge_color = if opts.ghost_bodies.contains(&b.id) { ghost_color(edge_color, ISOLATE_GHOST) } else { edge_color };
            // A mesh part's triangle sides: thin dark-grey lines at every zoom
            // (live G35, part-studio/551: #464646, 1 px), like edges hidden in
            // plain Shaded mode.
            if let Some(lines) = cached.mesh_lines.clone().filter(|_| opts.display_mode != DisplayMode::Shaded) {
                let l = Instance::new(Drawable::Lines(lines)).with_color(theme::lin(MESH_LINE_COLOR)).unpickable().line_scale(opts.graphics.line_width).layer(1);
                scene.instances.push(if opts.section_skip.contains(&b.id) { l.ignore_clip() } else { l });
            }
            // Onshape's backface silhouettes: the outline band of opaque solids
            // (with edges and in hidden line).
            if b.kind == BodyKind::Solid && color[3] >= 0.999 && matches!(opts.display_mode, DisplayMode::ShadedWithEdges | DisplayMode::HiddenLine) && replaced.is_none() {
                let s = Instance::new(Drawable::Mesh(cached.mesh.clone())).with_color(edge_color).pick_base(face_base).line_scale(opts.graphics.line_width).back_silhouette();
                scene.instances.push(if opts.section_skip.contains(&b.id) { s.ignore_clip() } else { s });
            }
            // Silhouettes of curved faces, like the edges (not in plain Shaded).
            if let Some(sil) = cached.silhouettes.clone().filter(|_| opts.display_mode != DisplayMode::Shaded && b.kind != BodyKind::Wire) {
                let s = Instance::new(Drawable::Lines(sil)).with_color(edge_color).pick_base(face_base).line_scale(opts.graphics.line_width).layer(1);
                scene.instances.push(if opts.section_skip.contains(&b.id) { s.ignore_clip() } else { s });
            }
            let edges = Instance::new(Drawable::Lines(cached.edges.clone())).with_color(edge_color).pick_base(edge_base).line_scale(opts.graphics.line_width).layer(1);
            let edges = if opts.section_skip.contains(&b.id) { edges.ignore_clip() } else { edges };
            // Shaded mode hides edges visually but keeps them pickable.
            let tangent = (opts.tangent_edges != TangentEdges::Visible && b.kind != BodyKind::Wire && opts.display_mode != DisplayMode::Shaded).then(|| {
                match cache.tangent.get(&key) {
                    Some((_, t)) => t.clone(),
                    None => {
                        let smooth = gmesh.smooth_edges();
                        let (mut sharp, mut phantom) = (LineData::default(), LineData::default());
                        for (ei, pl) in gmesh.edges.iter().enumerate() {
                            if smooth.get(ei).copied().unwrap_or(false) {
                                phantom.push_polyline(pl, [0, 0, 0, 255], 0, 1.0, LineSegment::DASH_DOT);
                            } else {
                                sharp.push_polyline(pl, [0, 0, 0, 255], ei as u32 + 1, 1.25, 0);
                            }
                        }
                        let t = TangentLines { sharp: up.upload_lines(&sharp), phantom: up.upload_lines(&phantom) };
                        cache.tangent.insert(key, (Arc::downgrade(gmesh), t.clone()));
                        t
                    }
                }
            });
            match tangent {
                // Tangent edges phantom/removed: draw the sharp ones (and the
                // tangent ones dash-dot); all of them stay pickable.
                Some(t) => {
                    scene.instances.push(edges.pick_only());
                    scene.instances.push(Instance::new(Drawable::Lines(t.sharp)).with_color(edge_color).pick_base(edge_base).line_scale(opts.graphics.line_width).layer(1));
                    if opts.tangent_edges == TangentEdges::Phantom {
                        // Phantom edges: Graphics preferences width (0.5 px default) and color.
                        let c = theme::lin(opts.graphics.phantom_color);
                        scene.instances.push(Instance::new(Drawable::Lines(t.phantom)).with_color(c).unpickable().line_scale(opts.graphics.phantom_width / 0.5).layer(1));
                    }
                }
                None if opts.display_mode == DisplayMode::Shaded => scene.instances.push(edges.pick_only()),
                // The full edges (also the pick) and each coarser line level.
                None => {
                    for (k, l) in cached.edge_lods.iter().enumerate().filter(|_| is_finest) {
                        let mut e = edges.clone().line_lod(k as u8 + 2);
                        e.drawable = Drawable::Lines(l.clone());
                        scene.instances.push(e);
                    }
                    scene.instances.push(edges.line_lod(1));
                }
            }
            if opts.hidden_edges_visible && b.kind != BodyKind::Wire {
                let handle = match cache.hidden_edges.get(&key) {
                    Some((_, h)) => h.clone(),
                    None => {
                        let mut l = LineData::default();
                        for pl in &gmesh.edges {
                            l.push_polyline(pl, [90, 90, 90, 140], 0, 1.0, LineSegment::DASHED | LineSegment::ON_TOP);
                        }
                        let h = up.upload_lines(&l);
                        cache.hidden_edges.insert(key, (Arc::downgrade(gmesh), h.clone()));
                        h
                    }
                };
                scene.instances.push(Instance::new(Drawable::Lines(handle)).per_primitive_color().unpickable().layer(1));
            }
            if opts.highlight_boundary && b.kind != BodyKind::Wire {
                let lines = match cache.boundary.get(&key) {
                    Some((_, l)) => l.clone(),
                    None => {
                        let laminar = laminar_edges(gmesh);
                        let (mut solid, mut dashed) = (LineData::default(), LineData::default());
                        for (pl, _) in gmesh.edges.iter().zip(&laminar).filter(|(_, l)| **l) {
                            solid.push_polyline(pl, [0, 0, 0, 255], 0, 2.0, 0);
                            dashed.push_polyline(pl, [0, 0, 0, 255], 0, 1.5, LineSegment::DASHED | LineSegment::ON_TOP);
                        }
                        let l = (!solid.segments.is_empty()).then(|| (up.upload_lines(&solid), up.upload_lines(&dashed)));
                        cache.boundary.insert(key, (Arc::downgrade(gmesh), l.clone()));
                        l
                    }
                };
                if let Some((solid, dashed)) = lines {
                    let red = theme::lin(theme::viewport::BOUNDARY_EDGE);
                    scene.instances.push(Instance::new(Drawable::Lines(solid)).with_color(red).unpickable().layer(3));
                    scene.instances.push(Instance::new(Drawable::Lines(dashed)).with_color(red).unpickable().layer(3));
                }
            }
            if levels.len() > 1 {
                for i in &mut scene.instances[level_first..] {
                    i.mesh_lod = b.levels_from + li as u8 + 1;
                }
            }
            if is_finest {
                finest_cached = Some(cached);
            }
        }
        let cached = finest_cached.expect("a body has its finest level");
        // B-rep vertices are the same at every level.
        scene.instances.push(Instance::new(Drawable::Points(cached.vertices.clone())).pick_base(vert_base).pick_only().layer(2));
        // Onshape's lowest level of detail of the body: its bounding box.
        let body = bodies + 1;
        bodies = body;
        for i in &mut scene.instances[first_instance..] {
            i.body = body;
        }
        if let Some(bounds) = cached.mesh.bounds().filter(|_| b.kind != BodyKind::Wire) {
            let unit = cache.lod_box.get_or_insert_with(|| up.upload_mesh(&noshape_render::scene::lod_box_mesh())).clone();
            let lod = Instance::lod_box(unit, body, bounds, color);
            scene.instances.push(if opts.section_skip.contains(&b.id) { lod.ignore_clip() } else { lod });
        }
        if let Some(s) = shared {
            let placement = placement_matrix(&s.transform);
            for i in &mut scene.instances[first_instance..] {
                i.transform = placement * i.transform;
            }
        }
        if display_only {
            for i in &mut scene.instances[first_instance..] {
                i.pickable = false;
            }
        }
    }

    // ---------------------------------------------------------------- planes
    if opts.show_planes {
        for pl in display.planes.iter().filter(|p| !p.hidden) {
            let id = picks.push(PickTarget { reference: pl.reference.clone(), kind: PickKind::Plane, anchor: Some(pl.frame.origin) });
            let f = &pl.frame;
            let c = |u: f64, v: f64| -> [f32; 3] { (f.origin + f.x * u * pl.half_width + f.y * v * pl.half_height).as_vec3().into() };
            let corners = [c(-1.0, -1.0), c(1.0, -1.0), c(1.0, 1.0), c(-1.0, 1.0)];
            let n: [f32; 3] = f.normal.as_vec3().into();
            let mesh = MeshData {
                positions: corners.to_vec(),
                normals: vec![n; 4],
                pick_ids: vec![id; 4],
                indices: vec![0, 1, 2, 0, 2, 3],
            };
            let mut border = LineData::default();
            let mut loop_pts = corners.to_vec();
            loop_pts.push(corners[0]);
            border.push_polyline(&loop_pts, [0, 0, 0, 0], id, 2.0, 0);
            // Onshape: CONSTRUCTION_PLANE_AMBIENT_COLOR at CONSTRUCTION_PLANE_OPACITY, outline at 35%.
            let fill = theme::lin(theme::pick(theme::viewport::PLANE_FILL, theme::viewport_dark::PLANE_FILL));
            let outline = theme::lin(theme::pick(theme::viewport::PLANE_OUTLINE, theme::viewport_dark::PLANE_OUTLINE));
            let outline_alpha = theme::pick(theme::viewport::PLANE_OUTLINE_ALPHA, theme::viewport_dark::PLANE_OUTLINE_ALPHA) * if opts.faint_planes { FAINT_PLANE_ALPHA } else { 1.0 };
            if !opts.faint_planes {
                scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&mesh))).with_color([fill[0], fill[1], fill[2], 0.2]).ignore_clip().unlit().layer(5));
            }
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&border))).with_color([outline[0], outline[1], outline[2], outline_alpha]).ignore_clip().layer(5));
        }
    }

    // ---------------------------------------------------------------- origin
    if opts.show_origin && display.show_origin {
        let id = picks.push(PickTarget { reference: Reference::Origin, kind: PickKind::Origin, anchor: Some(DVec3::ZERO) });
        // Onshape draws the origin as an 11 px ring with a center dot (point style 4).
        // While sketching it stays over the edge-on planes crossing it (live
        // sketch/06 shows the whole ring and dot).
        let style = 4 | if opts.editing_sketch.is_some() { LineSegment::ON_TOP } else { 0 };
        let pts = PointData { points: vec![PointSprite { position: [0.0; 3], color: theme::rgb8(theme::pick(theme::viewport::ORIGIN, theme::viewport_dark::ORIGIN)), pick_id: id, size: 12.0, style, _pad: 0 }] };
        scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&pts))).per_primitive_color().ignore_clip().layer(6));
    }

    // ---------------------------------------------------------------- sketches
    if opts.show_sketches || opts.editing_sketch.is_some() {
        for sk in &display.sketches {
            let editing = opts.editing_sketch == Some(sk.feature);
            if sk.hidden && !editing {
                continue;
            }
            if !opts.show_sketches && !editing {
                continue;
            }
            let mut lines = LineData::default();
            for c in &sk.curves {
                let id = picks.push(PickTarget { reference: Reference::SketchEntity { sketch: sk.feature, entity: c.entity }, kind: PickKind::SketchCurve, anchor: None });
                // Custom sketch appearance (Edit sketch appearance) when not editing.
                let color = match sk.color.filter(|_| !editing) {
                    Some(c) => lin_to_u8(c),
                    None => sketch_color(&opts.graphics, c.state, c.construction, editing, c.external),
                };
                let style = if c.construction { LineSegment::CONSTRUCTION } else { 0 };
                // Graphics preferences: active 2 px, inactive 1 px by default.
                let width = if editing { opts.graphics.sketch_line_width } else { 1.5 * opts.graphics.inactive_line_width };
                lines.push_polyline(&c.points, color, id, width, style);
            }
            let mut points = PointData::default();
            for p in &sk.points {
                let id = picks.push(PickTarget {
                    reference: Reference::SketchEntity { sketch: sk.feature, entity: p.entity },
                    kind: PickKind::SketchPoint,
                    anchor: Some(DVec3::new(p.position[0] as f64, p.position[1] as f64, p.position[2] as f64)),
                });
                let color = match sk.color.filter(|_| !editing) {
                    Some(c) => lin_to_u8(c),
                    None => sketch_color(&opts.graphics, p.state, false, editing, false),
                };
                // Live (sketch/06): vertices of the edited sketch are 7 px dots.
                let size = if p.standalone { 7.0 } else if !editing { 0.0 } else if p.handle { 5.0 } else { 7.0 };
                // Standalone points and spline handles are hollow (live sketch/100, Point tool).
                let style = if p.standalone || p.handle { PointSprite::HOLLOW } else { 0 };
                points.points.push(PointSprite { position: p.position, color, pick_id: id, size, style, _pad: 0 });
            }
            for d in &sk.dimensions {
                let color: [u8; 4] = theme::rgb8(if d.driven { theme::pick(theme::viewport::DIM_DRIVEN, theme::viewport_dark::DIM_DRIVEN) } else { theme::pick(theme::viewport::SKETCH_FULL, theme::viewport_dark::SKETCH_FULL) });
                for seg in &d.lines {
                    lines.segments.push(LineSegment { a: seg[0], b: seg[1], color, pick_id: 0, width: 1.0, style: LineSegment::ON_TOP });
                }
            }
            // Inserted images (Insert image): textured quads in their boxes,
            // depth tested like the model (Onshape shows them in 3D too).
            for img in &sk.images {
                let key = Arc::as_ptr(&img.data_base64) as *const u8 as usize;
                let handle = match cache.images.get(&key) {
                    Some((_, h)) => h.clone(),
                    None => {
                        let h = decode_image(&img.data_base64).map(|d| up.upload_image(&d));
                        cache.images.insert(key, (Arc::downgrade(&img.data_base64), h.clone()));
                        h
                    }
                };
                let Some(handle) = handle else { continue };
                let [ll, lr, _, ul] = img.corners.map(glam::Vec3::from);
                let (x, y) = (lr - ll, ul - ll);
                let z = x.cross(y).normalize_or_zero();
                let m = glam::Mat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), ll.extend(1.0));
                scene.instances.push(Instance::new(Drawable::Image(handle)).with_transform(m).ignore_clip().layer(8));
            }
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().ignore_clip().layer(10));
            scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&points))).per_primitive_color().ignore_clip().layer(11));
            // Graphics preferences "Underconstrained occluded color": the parts of
            // the edited sketch's underconstrained geometry hidden behind parts.
            if editing {
                let oc = theme::rgb8(opts.graphics.sketch_under_occluded);
                let mut hidden = LineData::default();
                for c in sk.curves.iter().filter(|c| c.state == SketchEntityState::Under && !c.external) {
                    let style = if c.construction { LineSegment::CONSTRUCTION } else { 0 };
                    hidden.push_polyline(&c.points, oc, 0, opts.graphics.sketch_line_width, style);
                }
                let hidden_pts = PointData {
                    points: sk.points.iter().filter(|p| p.state == SketchEntityState::Under).map(|p| PointSprite { position: p.position, color: oc, pick_id: 0, size: if p.standalone { 7.0 } else if p.handle { 3.5 } else { 7.0 }, style: 0, _pad: 0 }).collect(),
                };
                if !hidden.segments.is_empty() {
                    scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&hidden))).per_primitive_color().unpickable().hidden_only().ignore_clip().layer(10));
                }
                if !hidden_pts.points.is_empty() {
                    scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&hidden_pts))).per_primitive_color().unpickable().hidden_only().ignore_clip().layer(11));
                }
            }
            // Regions: invisible, pickable; highlight overlay added on hover/selection.
            // All regions of a sketch share one mesh (vertices carry their region's
            // pick id), so a sketch with hundreds of regions is one draw call.
            let n: [f32; 3] = sk.frame.normal.as_vec3().into();
            let mut mesh = MeshData::default();
            for r in &sk.regions {
                let id = picks.push(PickTarget { reference: Reference::SketchRegion { sketch: sk.feature, key: r.key.clone() }, kind: PickKind::SketchRegion, anchor: None });
                let base = mesh.positions.len() as u32;
                mesh.positions.extend_from_slice(&r.positions);
                mesh.normals.extend(std::iter::repeat_n(n, r.positions.len()));
                mesh.pick_ids.extend(std::iter::repeat_n(id, r.positions.len()));
                mesh.indices.extend(r.indices.iter().map(|i| i + base));
            }
            if !mesh.indices.is_empty() {
                // The sketch being edited shows its closed regions filled
                // (`#9aa5af`; live sketch/17: rgb(221,226,230) over the plane's
                // rgb(245,248,250), which our linear blending reaches at 0.34).
                // Dark: `#f2f2f2` at 20% (`--os-sketch-face-color`), #616161
                // over one plane's #3d3d3d: 0.09 in linear light.
                let fill = if editing {
                    let c = theme::lin(theme::pick(theme::viewport::SKETCH_REGION, theme::viewport_dark::SKETCH_REGION));
                    Instance::new(Drawable::Mesh(up.upload_mesh(&mesh))).with_color([c[0], c[1], c[2], theme::pick(0.34, 0.09)])
                } else {
                    Instance::new(Drawable::Mesh(up.upload_mesh(&mesh))).with_color([0.0, 0.0, 0.0, 0.0]).pick_only()
                };
                scene.instances.push(fill.ignore_clip().layer(9));
            }
        }
    }

    // ---------------------------------------------------------------- connectors
    for mc in &display.connectors {
        let id = picks.push(PickTarget { reference: mc.reference.clone(), kind: PickKind::Connector, anchor: Some(mc.origin) });
        let o: [f32; 3] = mc.origin.as_vec3().into();
        let y = mc.z.cross(mc.x);
        let mut l = LineData::default();
        let len = 0.008f64;
        for (axis, color) in [(mc.x, theme::rgb8(theme::pick(theme::viewport::AXIS_X, theme::viewport_dark::AXIS_X))), (y, theme::rgb8(theme::pick(theme::viewport::AXIS_Y, theme::viewport_dark::AXIS_Y))), (mc.z, theme::rgb8(theme::pick(theme::viewport::AXIS_Z, theme::viewport_dark::AXIS_Z)))] {
            let tip: [f32; 3] = (mc.origin + axis * len).as_vec3().into();
            l.segments.push(LineSegment { a: o, b: tip, color, pick_id: id, width: 2.0, style: LineSegment::ON_TOP });
        }
        scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&l))).per_primitive_color().ignore_clip().layer(12));
    }

    let mut planes = opts.section.iter().map(|(n, d)| noshape_render::ClipPlane::new(n.as_vec3(), *d as f32));
    scene.clip = planes.next();
    scene.clip_extra = planes.take(2).collect();
    if let Some(slot) = opts.section_selected.and_then(|i| scene.clip_selected.get_mut(i)) {
        *slot = true;
    }
    let static_instances = scene.instances.len();
    scene.states = vec![0; picks.len() + 1];
    BuiltScene { scene, picks, static_instances }
}

fn sketch_color(g: &crate::graphics_prefs::GraphicsPrefs, st: SketchEntityState, construction: bool, editing: bool, external: bool) -> [u8; 4] {
    use theme::viewport as v;
    // Constrained / underconstrained / inactive colors come from the Graphics preferences.
    let hex = if !editing {
        g.inactive_color
    } else if external {
        // Used (projected) geometry is fixed by its Use constraint: Onshape
        // draws it in the constrained color (live sketch/202).
        g.sketch_constrained
    } else {
        match st {
            SketchEntityState::Under => g.sketch_under,
            SketchEntityState::Full => g.sketch_constrained,
            SketchEntityState::Over => theme::pick(v::SKETCH_OVER, theme::viewport_dark::SKETCH_OVER),
            SketchEntityState::Inactive => g.inactive_color,
        }
    };
    let a = if construction { 200 } else { 255 };
    let hex = theme::sketch_color_for_theme(hex);
    [((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8, a]
}

/// Per-vertex local face id (1-based) from face index ranges.
/// A rigid 3x4 row-major placement (`SharedMesh::transform`) as a matrix.
fn placement_matrix(m: &[f64; 12]) -> glam::Mat4 {
    let c = |i: usize| m[i] as f32;
    glam::Mat4::from_cols(glam::Vec4::new(c(0), c(4), c(8), 0.0), glam::Vec4::new(c(1), c(5), c(9), 0.0), glam::Vec4::new(c(2), c(6), c(10), 0.0), glam::Vec4::new(c(3), c(7), c(11), 1.0))
}

fn face_pick_ids(m: &noshape_model::display::MeshBuffers) -> Vec<u32> {
    let mut pick_ids = vec![0u32; m.positions.len()];
    for (fi, &(start, count)) in m.face_ranges.iter().enumerate() {
        for &ix in &m.indices[start as usize..(start + count) as usize] {
            if let Some(p) = pick_ids.get_mut(ix as usize) {
                *p = fi as u32 + 1;
            }
        }
    }
    pick_ids
}

/// A sketch image (base64 PNG / JPEG / GIF / BMP) as a unit-square textured
/// quad; the instance transform places it in its box.
fn decode_image(data_base64: &str) -> Option<ImageData> {
    let bytes = noshape_model::sketch_feature::base64_decode(data_base64)?;
    let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(ImageData { corners: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]], width, height, rgba: rgba.into_raw() })
}

fn upload_body(m: &noshape_model::display::MeshBuffers, up: &Uploader) -> CachedBody {
    let pick_ids = face_pick_ids(m);

    let mut lines = LineData::default();
    lines.segments.reserve(m.edges.iter().map(|e| e.len().saturating_sub(1)).sum());
    for (ei, pl) in m.edges.iter().enumerate() {
        lines.push_polyline(pl, [0, 0, 0, 255], ei as u32 + 1, 1.25, 0);
    }
    let points = PointData {
        points: m.vertices.iter().enumerate().map(|(i, p)| PointSprite { position: *p, color: [255; 4], pick_id: i as u32 + 1, size: 6.0, style: 0, _pad: 0 }).collect(),
    };
    let mesh_lines = (!m.mesh_lines.is_empty()).then(|| {
        let mut l = LineData::default();
        l.segments.reserve(m.mesh_lines.len() / 2);
        for s in m.mesh_lines.as_chunks::<2>().0 {
            l.push_polyline(s, [0, 0, 0, 255], 0, 1.0, 0);
        }
        up.upload_lines(&l)
    });
    let sil = silhouette_edges(m);
    let silhouettes = (!sil.segments.is_empty()).then(|| up.upload_silhouettes(&sil));
    let edge_lods = edge_lods(m, up);
    CachedBody { mesh: up.upload_mesh_parts(&m.positions, &m.normals, &pick_ids, &m.indices), edges: up.upload_lines(&lines), vertices: up.upload_points(&points), mesh_lines, silhouettes, edge_lods }
}

/// A mesh's edges at each coarser line level (uploaded; empty without
/// edges).
pub fn edge_lods(m: &noshape_model::display::MeshBuffers, up: &Uploader) -> Vec<LinesHandle> {
    let Some((lo, hi)) = noshape_render::scene::bounds_of(m.positions.iter().copied()) else { return Vec::new() };
    if m.edges.is_empty() {
        return Vec::new();
    }
    noshape_render::scene::edge_line_lods(&m.edges, (hi - lo).length()).iter().map(|l| up.upload_lines(l)).collect()
}

/// Onshape's edge-based silhouettes: every tessellation edge inside a face
/// (shared by two of its triangles) whose two facets aren't coplanar, with
/// both facet normals; the renderer draws the ones where one facet faces
/// the viewer and the other doesn't, like the part's edges (1.25 px).
pub fn silhouette_edges(m: &noshape_model::display::MeshBuffers) -> SilhouetteData {
    let mut out = SilhouetteData::default();
    let p = |i: u32| glam::Vec3::from_array(m.positions[i as usize]);
    for (fi, &(start, count)) in m.face_ranges.iter().enumerate() {
        let tris = m.indices[start as usize..(start + count) as usize].as_chunks::<3>().0;
        // Each side by its (smaller, larger) vertex index: the first
        // triangle's normal, until the second one comes.
        let mut open: HashMap<(u32, u32), glam::Vec3> = HashMap::new();
        for t in tris {
            if t.iter().any(|&i| i as usize >= m.positions.len()) {
                continue;
            }
            let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0])).normalize_or_zero();
            if n == glam::Vec3::ZERO {
                continue;
            }
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let key = (a.min(b), a.max(b));
                match open.remove(&key) {
                    Some(other) if other.dot(n) < 1.0 - 1e-6 => out.segments.push(SilhouetteSegment { a: m.positions[a as usize], b: m.positions[b as usize], n1: other.into(), n2: n.into(), pick_id: fi as u32 + 1, width: 1.25 }),
                    Some(_) => {}
                    None => {
                        open.insert(key, n);
                    }
                }
            }
        }
    }
    out
}

/// Apply hover/selection state and rebuild highlight overlays for pick-only
/// elements. Returns a new scene sharing all static GPU buffers.
/// `error`: the edited feature fails. Live (behaviours "URGENT (10-22)",
/// part-studio/2022, 2028): its inputs are drawn red instead of the orange
/// selection: edges and face outlines red, sketch regions red-outlined with
/// a light red fill, no face fill.
#[allow(clippy::too_many_arguments)]
pub fn with_highlights(built: &BuiltScene, display: &StudioDisplay, selection: &[Reference], hover: Option<&Reference>, accent: &[Reference], extra: Vec<Instance>, error: bool, up: &Uploader) -> Scene {
    let _t = noshape_render::perf::scope("s.with_highlights");
    let mut scene = Scene { instances: built.scene.instances[..built.static_instances].to_vec(), states: vec![0; built.picks.len() + 1], clip: built.scene.clip, clip_extra: built.scene.clip_extra.clone(), clip_selected: built.scene.clip_selected, compare: built.scene.compare };
    let mut overlay_pts = PointData::default();
    let mut overlay_regions: Vec<(MeshData, [f32; 4], bool)> = Vec::new();
    let mut overlay_lines = LineData::default();
    let sel_color = theme::lin(if error { theme::viewport::ERROR } else { theme::viewport::SELECTION });
    let hov_color = theme::lin(theme::viewport::PREHIGHLIGHT);
    // Onshape outlines hovered and selected faces (their boundary edges
    // light orange / amber) instead of tinting a hovered face.
    let mut boundary: Vec<(Reference, u32)> = Vec::new();
    // Faces without edges (a mesh part's mesh face) get the part's
    // silhouette outline instead (live G35: light orange on hover, orange
    // around the selected part).
    let mut silhouette: Vec<Reference> = Vec::new();

    let mut mark = |r: &Reference, flag: u32, scene: &mut Scene| {
        // A sketch region is drawn from the display, also when its sketch
        // is hidden (no picks): the usual case for an extrude's region.
        if let Reference::SketchRegion { sketch, key } = r
            && let Some(sk) = display.sketches.iter().find(|s| s.feature == *sketch)
            && let Some(reg) = sk.regions.iter().find(|g| &g.key == key)
        {
            // Live (dialogs/extrude-02, 04): a hovered region shows only
            // its boundary in light orange; a selected one is filled
            // amber with an amber boundary.
            let hovered = flag == state::HOVER;
            let c = if hovered { hov_color } else { sel_color };
            // Region meshes sit 1 µm above the plane (regen
            // display.rs); the outline goes back onto the plane so
            // it doesn't count as hiding the sketch's own curves
            // (the underconstrained-occluded pass drew them #4040ff).
            let down = (sk.frame.normal * 1e-6).as_vec3();
            let flat = |p: [f32; 3]| -> [f32; 3] { (glam::Vec3::from(p) - down).into() };
            let outline = region_outline(&reg.positions, &reg.indices);
            for &(a, b) in &outline {
                overlay_lines.segments.push(LineSegment { a: flat(a), b: flat(b), color: lin_to_u8(c), pick_id: 0, width: 2.0, style: 0 });
            }
            // Live part-studio/2028: a picked region shows its corners as
            // small grey dots.
            if !hovered {
                for p in outline_corners(&outline) {
                    overlay_pts.points.push(PointSprite { position: flat(p), color: [155, 155, 155, 255], pick_id: 0, size: 3.5, style: LineSegment::ON_TOP, _pad: 0 });
                }
            }
            if !hovered {
                let n: [f32; 3] = sk.frame.normal.as_vec3().into();
                let fill = if error {
                    [c[0], c[1], c[2], 0.3]
                } else {
                    let k = theme::lin(theme::viewport::PICKED_REGION);
                    [k[0], k[1], k[2], theme::viewport::PICKED_REGION_ALPHA]
                };
                overlay_regions.push((MeshData { positions: reg.positions.clone(), normals: vec![n; reg.positions.len()], pick_ids: vec![0; reg.positions.len()], indices: reg.indices.clone() }, fill, !error));
            }
        }
        for &id in built.picks.ids_for(r) {
            let t = built.picks.get(id);
            // An erroring input face shows only its red outline.
            if !(flag == state::ERROR && t.is_some_and(|t| t.kind == PickKind::Face)) {
                scene.add_flag(id, flag);
            }
            let Some(t) = t else { continue };
            match t.kind {
                PickKind::Vertex => {
                    if let Some(a) = t.anchor {
                        let p: [f32; 3] = a.as_vec3().into();
                        if flag == state::ERROR {
                            overlay_pts.points.push(PointSprite { position: p, color: lin_to_u8(sel_color), pick_id: 0, size: 8.0, style: LineSegment::ON_TOP, _pad: 0 });
                        } else {
                            // Live (selection/08): the point marker.
                            let cols = if flag == state::HOVER { theme::viewport::POINT_HOVER } else { theme::viewport::POINT_SELECTED };
                            for (c, size, style) in [(cols[0], 11.0, 0), (cols[1], 8.0, PointSprite::RING), (cols[2], 5.0, 0)] {
                                overlay_pts.points.push(PointSprite { position: p, color: lin_to_u8(theme::lin(c)), pick_id: 0, size, style: LineSegment::ON_TOP | style, _pad: 0 });
                            }
                        }
                    }
                }
                PickKind::Face if flag & (state::HOVER | state::SELECTED | state::ERROR) != 0 => {
                    let edges = face_edges(display, r);
                    if edges.is_empty() && display.bodies.iter().any(|b| !b.hidden && b.face_refs.contains(r)) {
                        silhouette.push(r.clone());
                    }
                    boundary.extend(edges.into_iter().map(|e| (e, flag)));
                }
                _ => {}
            }
        }
    };
    for r in accent {
        mark(r, if error { state::ERROR } else { state::ACCENT }, &mut scene);
    }
    // A whole selected part: Onshape's lighter part amber (live #c09039 …
    // #f0b850, part-studio/09), not a selected face's.
    for r in selection {
        let flag = if error {
            state::ERROR
        } else if matches!(r, Reference::Body(_)) {
            state::SELECTED | state::BODY
        } else {
            state::SELECTED
        };
        mark(r, flag, &mut scene);
        // A part given by reference (its faces listed beside it): its faces
        // take the whole-part amber.
        if !error
            && let Reference::Body(b) = r
            && let Some(body) = display.body(b)
        {
            for f in &body.face_refs {
                for &id in built.picks.ids_for(f) {
                    scene.add_flag(id, state::BODY);
                }
            }
        }
    }
    if let Some(h) = hover {
        mark(h, state::HOVER, &mut scene);
    }
    for (e, flag) in boundary {
        for &id in built.picks.ids_for(&e) {
            scene.add_flag(id, flag);
        }
    }
    for f in silhouette {
        for &id in built.picks.ids_for(&f) {
            scene.add_flag(id, state::OUTLINE);
        }
    }
    if !overlay_pts.points.is_empty() {
        scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&overlay_pts))).per_primitive_color().unpickable().ignore_clip().layer(20));
    }
    for (m, c, picked) in overlay_regions {
        let inst = Instance::new(Drawable::Mesh(up.upload_mesh(&m))).with_color(c).unpickable().ignore_clip().layer(8);
        // A picked region tints the preview built on it from above (live
        // part-studio/230): flat, after the see-through meshes.
        scene.instances.push(if picked { inst.unlit().over_transparent() } else { inst });
    }
    if !overlay_lines.segments.is_empty() {
        scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&overlay_lines))).per_primitive_color().unpickable().ignore_clip().layer(12));
    }
    scene.instances.extend(extra);
    scene
}

/// A failed feature's error geometry while its dialog is open (live "lp
/// wall over" 2256–2262: a Feature pattern's ghost instances). Faces are
/// translucent red (flat, as live's top and front faces read alike), and
/// depth-tested (the parts inside the block
/// don't show); edges are red, 3–4 px, and drawn through everything.
pub fn error_geometry(mesh: &noshape_model::display::MeshBuffers, up: &Uploader) -> Vec<Instance> {
    let mut out = Vec::new();
    if !mesh.indices.is_empty() {
        // 1 µm off the face, like sketch regions: a ghost face lying on a
        // part face shows over it.
        let positions = mesh.positions.iter().zip(&mesh.normals).map(|(p, n)| std::array::from_fn(|i| p[i] + n[i] * 1e-6)).collect();
        let m = MeshData { positions, normals: mesh.normals.clone(), pick_ids: vec![0; mesh.positions.len()], indices: mesh.indices.clone() };
        let mut fill = theme::lin(theme::viewport::ERROR_FILL);
        fill[3] = theme::viewport::ERROR_FILL_ALPHA;
        out.push(Instance::new(Drawable::Mesh(up.upload_mesh(&m))).with_color(fill).unlit().unpickable().ignore_clip().layer(8));
    }
    let color = lin_to_u8(theme::lin(theme::viewport::ERROR));
    let mut lines = LineData::default();
    for pl in &mesh.edges {
        for w in pl.windows(2) {
            lines.segments.push(LineSegment { a: w[0], b: w[1], color, pick_id: 0, width: 3.5, style: LineSegment::ON_TOP });
        }
    }
    if !lines.segments.is_empty() {
        out.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(12));
    }
    out
}

/// A feature's debug geometry while its dialog is open: Helix's profiles
/// (magenta) and FeatureScript `debug()` entities in their colours (live "fs
/// (10-25 c)": faces lit at half opacity, edges 2 px and points 6 px drawn
/// through the model).
pub fn debug_geometry(debug: &noshape_model::display::DebugGeometry, up: &Uploader) -> Vec<Instance> {
    use noshape_model::display::ColoredDebug;
    let mut out = Vec::new();
    let color = lin_to_u8(theme::lin(theme::viewport::DEBUG_MAGENTA));
    let mut lines = LineData::default();
    for pl in &debug.lines {
        lines.push_polyline(pl, color, 0, 2.5, LineSegment::ON_TOP);
    }
    let mut points = Vec::new();
    for item in &debug.colored {
        match item {
            ColoredDebug::Faces(mesh, [r, g, b]) => {
                // 1 µm off the face, so a debugged part face shows over it.
                let positions = mesh.positions.iter().zip(&mesh.normals).map(|(p, n)| std::array::from_fn(|i| p[i] + n[i] * 1e-6)).collect();
                let m = MeshData { positions, normals: mesh.normals.clone(), pick_ids: vec![0; mesh.positions.len()], indices: mesh.indices.clone() };
                let mut fill = noshape_render::srgb(*r, *g, *b);
                fill[3] = 0.5;
                out.push(Instance::new(Drawable::Mesh(up.upload_mesh(&m))).with_color(fill).unpickable().ignore_clip().layer(8));
            }
            ColoredDebug::Line(pl, [r, g, b]) => lines.push_polyline(pl, [*r, *g, *b, 255], 0, 2.0, LineSegment::ON_TOP),
            ColoredDebug::Point(p, [r, g, b]) => points.push(PointSprite { position: *p, color: [*r, *g, *b, 255], pick_id: 0, size: 6.0, style: LineSegment::ON_TOP, _pad: 0 }),
        }
    }
    if !points.is_empty() {
        out.push(Instance::new(Drawable::Points(up.upload_points(&PointData { points }))).per_primitive_color().unpickable().ignore_clip().layer(20));
    }
    if !lines.segments.is_empty() {
        out.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(12));
    }
    if !debug.points.is_empty() {
        let c = lin_to_u8(theme::lin(theme::viewport::DEBUG_MAGENTA_POINT));
        let pts = PointData { points: debug.points.iter().map(|p| PointSprite { position: *p, color: c, pick_id: 0, size: 7.0, style: LineSegment::ON_TOP, _pad: 0 }).collect() };
        out.push(Instance::new(Drawable::Points(up.upload_points(&pts))).per_primitive_color().unpickable().ignore_clip().layer(20));
    }
    out
}

/// The boundary edges of `faces` where other geometry hides them, dashed in
/// the selection color (live "lp wall over" 2258: the seed of a failed
/// Feature pattern behind the block).
pub fn hidden_edges_dashed(display: &StudioDisplay, faces: &[Reference], up: &Uploader) -> Option<Instance> {
    let edges: std::collections::HashSet<Reference> = faces.iter().flat_map(|f| face_edges(display, f)).collect();
    let color = lin_to_u8(theme::lin(theme::viewport::SELECTION));
    let mut lines = LineData::default();
    for b in display.bodies.iter().filter(|b| !b.hidden) {
        for (pl, r) in b.mesh.edges.iter().zip(&b.edge_refs) {
            if edges.contains(r) {
                lines.push_polyline(pl, color, 0, 1.5, LineSegment::DASHED);
            }
        }
    }
    (!lines.segments.is_empty()).then(|| Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().hidden_only().layer(12))
}

/// An edge by its quantized end points.
type EdgeKey = ([i64; 3], [i64; 3]);

/// The boundary of a triangulated region: the triangle edges used once
/// (vertices matched by position, so split meshes work too).
/// Points of an outline where it turns (not along a straight run).
fn outline_corners(segs: &[([f32; 3], [f32; 3])]) -> Vec<[f32; 3]> {
    let key = |p: &[f32; 3]| [(p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64, (p[2] * 1e6).round() as i64];
    let mut dirs: HashMap<[i64; 3], ([f32; 3], Vec<glam::Vec3>)> = HashMap::new();
    for (a, b) in segs {
        let d = (glam::Vec3::from(*b) - glam::Vec3::from(*a)).normalize_or_zero();
        dirs.entry(key(a)).or_insert((*a, Vec::new())).1.push(d);
        dirs.entry(key(b)).or_insert((*b, Vec::new())).1.push(-d);
    }
    dirs.into_values().filter(|(_, ds)| ds.len() == 2 && ds[0].dot(-ds[1]) < 0.999).map(|(p, _)| p).collect()
}

fn region_outline(positions: &[[f32; 3]], indices: &[u32]) -> Vec<([f32; 3], [f32; 3])> {
    let q = |p: &[f32; 3]| [(p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64, (p[2] * 1e6).round() as i64];
    // Edge (quantized ends) -> (uses, ends).
    let mut count: HashMap<EdgeKey, (u32, [f32; 3], [f32; 3])> = HashMap::new();
    for t in indices.as_chunks::<3>().0 {
        for (i, j) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (Some(a), Some(b)) = (positions.get(i as usize), positions.get(j as usize)) else { continue };
            let (qa, qb) = (q(a), q(b));
            let key = if qa <= qb { (qa, qb) } else { (qb, qa) };
            count.entry(key).or_insert((0, *a, *b)).0 += 1;
        }
    }
    count.into_values().filter(|(n, _, _)| *n == 1).map(|(_, a, b)| (a, b)).collect()
}

/// `NOSHAPE_PERF`: log a regeneration's per-feature timing breakdown.
pub fn log_regen_perf(label: &str, display: &StudioDisplay, studio: &noshape_model::partstudio::PartStudio) {
    if !noshape_render::perf::enabled() {
        return;
    }
    let mut parts: Vec<String> = Vec::new();
    for st in display.features.iter().filter(|s| s.time_ms > 0.05) {
        let name = studio.features.iter().find(|f| f.id == st.id).map(|f| f.name.as_str()).unwrap_or("?");
        parts.push(format!("{name} {:.1}", st.time_ms));
    }
    let tris: usize = display.bodies.iter().map(|b| b.mesh.indices.len() / 3).sum();
    log::info!(target: "perf", "regen[{label}]: total {:.1} ms, {tris} triangles; feature eval times (cached ones keep their last time): [{}]", display.regen_ms, parts.join(", "));
}

/// Per edge: whether only one face of the body touches it (a laminar or
/// boundary edge: the open edges of surfaces, gaps in imported parts). An
/// edge's points are vertices of the triangles of the faces it bounds.
fn laminar_edges(m: &noshape_model::display::MeshBuffers) -> Vec<bool> {
    let q = |p: &[f32; 3]| [(p[0] * 1e5).round() as i64, (p[1] * 1e5).round() as i64, (p[2] * 1e5).round() as i64];
    let mut faces_at: HashMap<[i64; 3], Vec<u32>> = HashMap::new();
    for (f, &(first, count)) in m.face_ranges.iter().enumerate() {
        for &i in m.indices.get(first as usize..(first + count) as usize).unwrap_or(&[]) {
            if let Some(p) = m.positions.get(i as usize) {
                let e = faces_at.entry(q(p)).or_default();
                if e.last() != Some(&(f as u32)) && !e.contains(&(f as u32)) {
                    e.push(f as u32);
                }
            }
        }
    }
    m.edges
        .iter()
        .map(|pl| {
            let mut common: Option<Vec<u32>> = None;
            for p in pl {
                let at = faces_at.get(&q(p)).cloned().unwrap_or_default();
                common = Some(match common {
                    None => at,
                    Some(c) => c.into_iter().filter(|f| at.contains(f)).collect(),
                });
            }
            common.is_some_and(|c| c.len() == 1)
        })
        .collect()
}

/// The edges bounding a body face: those whose polyline points are all
/// vertices of the face's triangles (the mesher shares edge points with the
/// faces it bounds).
pub fn face_edges(display: &StudioDisplay, r: &Reference) -> Vec<Reference> {
    let Some((body, fi)) = display.bodies.iter().filter(|b| !b.hidden).find_map(|b| b.face_refs.iter().position(|f| f == r).map(|i| (b, i))) else { return Vec::new() };
    let m = &body.mesh;
    let Some(&(first, count)) = m.face_ranges.get(fi) else { return Vec::new() };
    let q = |p: &[f32; 3]| [(p[0] * 1e5).round() as i64, (p[1] * 1e5).round() as i64, (p[2] * 1e5).round() as i64];
    let verts: std::collections::HashSet<[i64; 3]> = m.indices.get(first as usize..(first + count) as usize).unwrap_or(&[]).iter().filter_map(|&i| m.positions.get(i as usize)).map(q).collect();
    m.edges
        .iter()
        .zip(&body.edge_refs)
        .filter(|(pl, _)| !pl.is_empty() && pl.iter().all(|p| verts.contains(&q(p))))
        .map(|(_, e)| e.clone())
        .collect()
}

fn lin_to_u8(c: [f32; 4]) -> [u8; 4] {
    let f = |x: f32| (if x <= 0.0031308 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 } * 255.0).round().clamp(0.0, 255.0) as u8;
    [f(c[0]), f(c[1]), f(c[2]), 255]
}
