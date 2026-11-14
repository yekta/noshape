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
use noshape_render::{Drawable, ImageData, ImageHandle, Instance, LineData, LineSegment, LinesHandle, MeshData, MeshHandle, PointData, PointSprite, PointsHandle, Scene, SilhouetteData, SilhouetteSegment, TexturedData, state};
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
    /// Higher wins when several candidates are under the cursor radius, in live's order (live/feel/3d.json
    /// "pick_priority": a sketch point over a sketch line over a mate connector over a vertex over an edge over the
    /// origin over a sketch region over a face over a plane).
    pub fn priority(self) -> i32 {
        match self {
            PickKind::SketchPoint => 8,
            PickKind::SketchCurve => 7,
            PickKind::Connector => 6,
            PickKind::Vertex => 5,
            PickKind::Edge => 4,
            PickKind::Origin => 3,
            PickKind::SketchRegion => 2,
            PickKind::Face => 1,
            PickKind::Plane => 0,
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
    /// Runs of consecutive ids, ascending by their first id − 1: a body's faces, edges and vertices are shared with
    /// the previous build's while the body is unchanged ([`GpuCache`]'s `picks`: cloning every name each build took
    /// most of a scene build on a dialog's open and close).
    runs: Vec<(u32, PickRun)>,
    /// The targets pushed one by one (planes, sketches, connectors), in [`PickRun::Own`] runs.
    own: Vec<PickTarget>,
    len: u32,
    /// Ids by [`shallow_hash`] of their reference: one group per distinct reference (a custom feature's faces are
    /// named from earlier faces, recursively; cloning and hashing every name whole took most of a scene build).
    by_ref: HashMap<u64, IdGroups, std::hash::BuildHasherDefault<KeyHasher>>,
    /// Per pickable body, ascending: (its first face id − 1, face count, its first edge id − 1, its index in the
    /// display's bodies).
    bodies: Vec<(u32, u32, u32, usize)>,
    /// Ids that win over others of their kind under the cursor (a spline's handle arm over the spline it runs
    /// along), ascending.
    boosted: Vec<u32>,
}

#[derive(Clone, Debug)]
enum PickRun {
    Shared(Arc<[PickTarget]>),
    /// `own[from..from + len]`.
    Own { from: usize, len: usize },
}

/// A body's pick targets and their [`shallow_hash`]es, kept across builds while its mesh lives and its names stay.
struct BodyPicks {
    faces: Arc<[PickTarget]>,
    edges: Arc<[PickTarget]>,
    vertices: Arc<[PickTarget]>,
    hashes: [Vec<u64>; 3],
}

impl BodyPicks {
    fn of(b: &noshape_model::display::BodyDisplay) -> Self {
        let faces: Arc<[PickTarget]> = b.face_refs.iter().map(|r| PickTarget { reference: r.clone(), kind: PickKind::Face, anchor: None }).collect();
        let edges: Arc<[PickTarget]> = b.edge_refs.iter().map(|r| PickTarget { reference: r.clone(), kind: PickKind::Edge, anchor: None }).collect();
        let vertices: Arc<[PickTarget]> = b
            .vertex_refs
            .iter()
            .enumerate()
            .map(|(i, r)| PickTarget { reference: r.clone(), kind: PickKind::Vertex, anchor: b.mesh.vertices.get(i).map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)) })
            .collect();
        let hashes = [&faces, &edges, &vertices].map(|ts| ts.iter().map(|t| shallow_hash(&t.reference)).collect());
        BodyPicks { faces, edges, vertices, hashes }
    }

    /// Whether these are `b`'s (same mesh, so the same vertex anchors): its names unchanged.
    fn matches(&self, b: &noshape_model::display::BodyDisplay) -> bool {
        let same = |ts: &[PickTarget], rs: &[Reference]| ts.len() == rs.len() && ts.iter().zip(rs).all(|(t, r)| t.reference == *r);
        same(&self.faces, &b.face_refs) && same(&self.edges, &b.edge_refs) && same(&self.vertices, &b.vertex_refs)
    }
}

/// The pick ids of one [`shallow_hash`]: one reference's (most), or groups of the references that share it.
#[derive(Clone, Debug)]
enum IdGroups {
    One(u32),
    Many(Vec<Vec<u32>>),
}

/// A fast multiply-rotate hasher for [`shallow_hash`] (SipHash's per-write cost dominated the scene build), also
/// keying the map by those hashes.
#[derive(Default)]
struct KeyHasher(u64);

impl KeyHasher {
    fn add(&mut self, w: u64) {
        self.0 = (self.0.rotate_left(5) ^ w).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl std::hash::Hasher for KeyHasher {
    fn write(&mut self, bytes: &[u8]) {
        for c in bytes.chunks(8) {
            let mut b = [0u8; 8];
            b[..c.len()].copy_from_slice(c);
            self.add(u64::from_le_bytes(b));
        }
    }
    fn write_u8(&mut self, n: u8) {
        self.add(n as u64);
    }
    fn write_u16(&mut self, n: u16) {
        self.add(n as u64);
    }
    fn write_u32(&mut self, n: u32) {
        self.add(n as u64);
    }
    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }
    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }
    fn write_isize(&mut self, n: isize) {
        self.add(n as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

/// A reference's hash from its first few levels only: equal references hash alike, and names that differ that
/// close to the top rarely collide.
fn shallow_hash(r: &Reference) -> u64 {
    use noshape_model::reference::{Role, Source, TopoName};
    use std::hash::{Hash, Hasher};
    const DEPTH: u32 = 4;
    fn source(s: &Source, h: &mut impl Hasher, depth: u32) {
        std::mem::discriminant(s).hash(h);
        match s {
            Source::Topo(n) => name(n, h, depth),
            other => other.hash(h),
        }
    }
    fn role(r: &Role, h: &mut impl Hasher, depth: u32) {
        std::mem::discriminant(r).hash(h);
        match r {
            Role::Side(s) | Role::Profile(s) => source(s, h, depth),
            Role::CapStart(s) | Role::CapEnd(s) => {
                if let Some(s) = s {
                    source(s, h, depth);
                }
            }
            Role::Blend(n) | Role::Generated(n) | Role::Offset(n) => name(n, h, depth),
            Role::New(i) | Role::Datum(i) => i.hash(h),
            Role::Copy { instance, of } => {
                instance.hash(h);
                name(of, h, depth);
            }
        }
    }
    fn name(n: &TopoName, h: &mut impl Hasher, depth: u32) {
        std::mem::discriminant(n).hash(h);
        let Some(d) = depth.checked_sub(1) else { return };
        match n {
            TopoName::Face { feature, role: r } => {
                feature.hash(h);
                role(r, h, d);
            }
            TopoName::Split { parent, by, ordinal } => {
                by.hash(h);
                ordinal.hash(h);
                name(parent, h, d);
            }
            TopoName::Edge { faces, ordinal } => {
                ordinal.hash(h);
                faces.iter().for_each(|f| name(f, h, d));
            }
            TopoName::Vertex { faces, ordinal } => {
                ordinal.hash(h);
                faces.len().hash(h);
                faces.iter().take(3).for_each(|f| name(f, h, d));
            }
            TopoName::Wire { feature, source: s } => {
                feature.hash(h);
                source(s, h, d);
            }
            TopoName::Boundary { face, ordinal } => {
                ordinal.hash(h);
                name(face, h, d);
            }
        }
    }
    let mut h = KeyHasher::default();
    std::mem::discriminant(r).hash(&mut h);
    match r {
        Reference::Face(n) | Reference::Edge(n) | Reference::Vertex(n) => name(n, &mut h, DEPTH),
        other => other.hash(&mut h),
    }
    h.finish()
}

impl PickMap {
    fn push(&mut self, t: PickTarget) -> u32 {
        let h = shallow_hash(&t.reference);
        self.own.push(t);
        match self.runs.last_mut() {
            Some((_, PickRun::Own { len, .. })) => *len += 1,
            _ => self.runs.push((self.len, PickRun::Own { from: self.own.len() - 1, len: 1 })),
        }
        self.len += 1;
        self.index(h, self.len);
        self.len
    }

    /// Appends a run of targets with their hashes, returning the base such that its `k`-th (1-based) has id
    /// `base + k`.
    fn extend(&mut self, targets: &Arc<[PickTarget]>, hashes: &[u64]) -> u32 {
        let base = self.len;
        if targets.is_empty() {
            return base;
        }
        self.runs.push((base, PickRun::Shared(targets.clone())));
        self.len += targets.len() as u32;
        for (k, &h) in hashes.iter().enumerate() {
            self.index(h, base + k as u32 + 1);
        }
        base
    }

    /// Files id `id` (already stored) under its reference's hash `h`.
    fn index(&mut self, h: u64, id: u32) {
        let Self { runs, own, by_ref, .. } = self;
        let at = |a: u32| target_in(runs, own, a).map(|t| &t.reference);
        let same = |a: u32| at(a) == at(id);
        match by_ref.entry(h) {
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(IdGroups::One(id));
            }
            std::collections::hash_map::Entry::Occupied(mut e) => {
                let g = e.get_mut();
                match g {
                    IdGroups::One(a) => *g = if same(*a) { IdGroups::Many(vec![vec![*a, id]]) } else { IdGroups::Many(vec![vec![*a], vec![id]]) },
                    IdGroups::Many(groups) => match groups.iter_mut().find(|g| same(g[0])) {
                        Some(g) => g.push(id),
                        None => groups.push(vec![id]),
                    },
                }
            }
        }
    }
    pub fn get(&self, id: u32) -> Option<&PickTarget> {
        target_in(&self.runs, &self.own, id)
    }
    fn iter(&self) -> impl Iterator<Item = &PickTarget> {
        self.runs.iter().flat_map(|(_, r)| match r {
            PickRun::Shared(ts) => &ts[..],
            PickRun::Own { from, len } => &self.own[*from..from + len],
        })
    }
    /// The body (index in the display's bodies), face index and edge id base of face pick `id`.
    pub fn face_at(&self, id: u32) -> Option<(usize, usize, u32)> {
        let k = self.bodies.partition_point(|b| b.0 < id).checked_sub(1)?;
        let (base, count, edge_base, body) = self.bodies[k];
        (id > base && id - base <= count).then(|| (body, (id - base - 1) as usize, edge_base))
    }
    pub fn ids_for(&self, r: &Reference) -> &[u32] {
        let same = |a: u32| self.get(a).is_some_and(|t| t.reference == *r);
        match self.by_ref.get(&shallow_hash(r)) {
            Some(IdGroups::One(a)) if same(*a) => std::slice::from_ref(a),
            Some(IdGroups::Many(groups)) => groups.iter().find(|g| same(g[0])).map(|g| g.as_slice()).unwrap_or(&[]),
            _ => &[],
        }
    }
    pub fn len(&self) -> usize {
        self.len as usize
    }
    /// Ids of the targets `f` accepts.
    pub fn ids_where<'a>(&'a self, f: impl Fn(&PickTarget) -> bool + 'a) -> impl Iterator<Item = u32> + 'a {
        self.iter().enumerate().filter(move |(_, t)| f(t)).map(|(i, _)| i as u32 + 1)
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn priority_fn(&self) -> Arc<dyn Fn(u32) -> i32 + Send + Sync> {
        self.priority_fn_filtered(|_| true)
    }

    /// `id` wins over the other targets of its kind under the cursor ([`Self::priority_fn`]).
    pub fn boost(&mut self, id: u32) {
        if let Err(k) = self.boosted.binary_search(&id) {
            self.boosted.insert(k, id);
        }
    }

    /// Like [`Self::priority_fn`], but kinds `allow` rejects are never
    /// picked or hovered (a dialog field's selection filter, Onshape).
    pub fn priority_fn_filtered(&self, allow: impl Fn(PickKind) -> bool) -> Arc<dyn Fn(u32) -> i32 + Send + Sync> {
        self.priority_fn_with(allow)
    }

    fn priority_fn_with(&self, allow: impl Fn(PickKind) -> bool) -> Arc<dyn Fn(u32) -> i32 + Send + Sync> {
        // (×2: a boosted id one above its kind, below the next kind.)
        let prio: Vec<i32> = self
            .iter()
            .zip(1u32..)
            .map(|(t, id)| if allow(t.kind) { 2 * t.kind.priority() + i32::from(self.boosted.binary_search(&id).is_ok()) } else { i32::MIN })
            .collect();
        Arc::new(move |id| if id == 0 { -1 } else { prio.get(id as usize - 1).copied().unwrap_or(0) })
    }
}

/// Pick `id`'s target in `runs` (ascending by first id − 1) and the singly pushed `own`.
fn target_in<'a>(runs: &'a [(u32, PickRun)], own: &'a [PickTarget], id: u32) -> Option<&'a PickTarget> {
    let k = runs.partition_point(|(base, _)| *base < id).checked_sub(1)?;
    let (base, run) = &runs[k];
    let i = (id - base - 1) as usize;
    match run {
        PickRun::Shared(ts) => ts.get(i),
        PickRun::Own { from, len } => (i < *len).then(|| &own[from + i]),
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
    /// Hidden-edge overlays (faded, on top) keyed by mesh pointer.
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
    /// Decal images decoded, keyed by their (shared) encoded bytes.
    decal_images: HashMap<usize, (std::sync::Weak<Vec<u8>>, Option<DecodedImage>)>,
    /// Bodies' pick targets keyed by their mesh's pointer.
    picks: HashMap<usize, (std::sync::Weak<noshape_model::display::MeshBuffers>, Arc<BodyPicks>)>,
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
    /// Draw edges hidden behind faces, faded toward the background.
    pub hidden_edges_visible: bool,
    /// How tangent (smooth) edges are drawn.
    pub tangent_edges: TangentEdges,
    /// Parts the section view leaves whole.
    pub section_skip: Vec<noshape_model::BodyId>,
    /// Feature preview bodies: shaded with the preview lighting.
    pub preview_bodies: Vec<noshape_model::BodyId>,
    /// The compare layer of every body and decal instance
    /// ([`noshape_render::scene::compare`]; 0 none): a feature preview's
    /// two models ([`Scene::preview_mix`]).
    pub body_layer: u8,
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
    /// Parts picks go through (Isolate's see-through parts without "Select
    /// transparent geometry"); every other part is picked nearest-first even
    /// when translucent.
    pub pick_through_bodies: Vec<noshape_model::BodyId>,
    /// How far the ghosts go toward white ([`ISOLATE_GHOST`] if unset;
    /// [`MAKE_TRANSPARENT_GHOST`] for Make transparent).
    pub ghost_mix: Option<f32>,
    /// Isolate's planes (live ui/iso-901): no fill, the outline at
    /// [`FAINT_PLANE_ALPHA`] of its opacity (172 → 252 over white).
    pub faint_planes: bool,
    /// Show dimensions is on: the parts' faces turned from the light come
    /// up 5–8 levels (live part-studio/40 vs selection/02).
    pub dims_shown: bool,
}

/// Opacity of the selection drawn again over the datum planes (fitted to
/// live selection/04's blue: 56 under one plane, 73 under two).
const SELECTION_OVER_PLANES: f32 = 0.44;

/// [`SceneOptions::dims_shown`]'s lighting shift: ambient 0.5 → 0.535,
/// light 0.5 → 0.465 (front face (84,111,127) vs live (86,110,124), the
/// top face unchanged as live's).
const DIMS_SHOWN_AMBIENT: f32 = 0.035;

/// The layer of the highlight fills [`with_highlights`] adds (a picked
/// sketch region's tint, a lit plane's fill).
pub const HIGHLIGHT_FILL_LAYER: i32 = 8;
/// The layer of the highlight lines [`with_highlights`] adds (a picked
/// region's outline, a lit plane's border).
pub const HIGHLIGHT_LINES_LAYER: i32 = 12;

/// See [`SceneOptions::faint_planes`].
pub const FAINT_PLANE_ALPHA: f32 = 0.04;

/// How far Isolate's ghosts are mixed toward white (live iso-900 → iso-901:
/// 160 → 234, 107 → 220, 209 → 246, i.e. 0.78 of the way, every face).
pub const ISOLATE_GHOST: f32 = 0.78;
/// Make transparent's parts (live 10-27, "spec: psui part-studio.md 2.18 /
/// 2.19"): 0.29 of the colour over white on every channel (Part 1's top
/// (142,187,214) → (222,235,243)).
pub const MAKE_TRANSPARENT_GHOST: f32 = 0.71;

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
    /// Decal instances: (instance index, the face, the Decal feature), for
    /// the selection tint.
    pub decals: Vec<(usize, Reference, FeatureId)>,
    /// The first pick id of the edges a preview consumed ([`add_consumed_edge_picks`]); every pick from it on is one.
    pub consumed_from: u32,
}

impl BuiltScene {
    /// Whether `r` is drawn here only as a consumed edge's pick (or not at all).
    pub fn consumed(&self, r: &Reference) -> bool {
        self.picks.ids_for(r).iter().all(|&id| id >= self.consumed_from)
    }
}

const EDGE_COLOR: u32 = theme::viewport::EDGE;
/// Mesh parts' triangle lines (live G35, part-studio/551).
pub const MESH_LINE_COLOR: u32 = 0x464646;

/// Pick base of display-only bodies: beyond any scene's pick ids.
const DISPLAY_ONLY_PICK_BASE: u32 = 1 << 30;

pub fn build(display: &StudioDisplay, opts: &SceneOptions, cache: &mut GpuCache, up: &Uploader) -> BuiltScene {
    let _t = noshape_render::perf::scope("s.build");
    // Live (behaviours "psui (2026-10-27, P2) PS-SECTION overlap"): a Part Studio's section caps are flat
    // interference red where two parts overlap, as an assembly's (the earlier capture saw no overlap).
    let mut scene = Scene { section_interference: true, no_occlusion: matches!(opts.display_mode, DisplayMode::HiddenLine | DisplayMode::Translucent), ..Default::default() };
    let mut picks = PickMap::default();

    // ---------------------------------------------------------------- bodies
    cache.bodies.retain(|_, (w, _)| w.strong_count() > 0);
    cache.colored.retain(|_, (w, _)| w.strong_count() > 0);
    cache.hidden_edges.retain(|_, (w, _)| w.strong_count() > 0);
    cache.tangent.retain(|_, (w, _)| w.strong_count() > 0);
    cache.images.retain(|_, (w, _)| w.strong_count() > 0);
    cache.picks.retain(|_, (w, _)| w.strong_count() > 0);
    let t_upload = std::time::Instant::now();
    let uploaded = upload_bodies_in_parallel(display, opts, cache, up);
    if uploaded > 0 && noshape_render::perf::enabled() {
        log::info!(target: "perf", "scene: {uploaded} body meshes uploaded in {:.1} ms", t_upload.elapsed().as_secs_f64() * 1000.0);
    }
    let mut bodies = 0u32;
    let bodies_from = scene.instances.len();
    for (part_index, b) in display.bodies.iter().enumerate().filter(|(_, b)| !b.hidden) {
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
            let key = Arc::as_ptr(&b.mesh) as usize;
            let bp = match cache.picks.get(&key).filter(|(_, bp)| bp.matches(b)) {
                Some((_, bp)) => bp.clone(),
                None => {
                    let bp = Arc::new(BodyPicks::of(b));
                    cache.picks.insert(key, (Arc::downgrade(&b.mesh), bp.clone()));
                    bp
                }
            };
            (picks.extend(&bp.faces, &bp.hashes[0]), picks.extend(&bp.edges, &bp.hashes[1]), picks.extend(&bp.vertices, &bp.hashes[2]))
        };
        if !display_only {
            picks.bodies.push((face_base, b.face_refs.len() as u32, edge_base, part_index));
        }
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
                    DisplayMode::ShadedWithEdges | DisplayMode::Shaded => mesh.with_color(color).zebra(opts.zebra.filter(|_| replaced.is_none())).reflection(opts.reflection.clone().filter(|_| replaced.is_none())).ambient(if opts.dims_shown { DIMS_SHOWN_AMBIENT } else { 0.0 }),
                    // Onshape's TRANSLUCENT_PASS_ALPHA: the shaded part's front faces at 25 % (live 11-02: a plate's top one
                    // layer, each channel a quarter of the way from white).
                    DisplayMode::Translucent => mesh.with_color([color[0], color[1], color[2], color[3].min(0.25)]).front_only(),
                    // Faces drawn flat in the background color hide what is behind
                    // them (live part-studio/61: white, unlit, only the edges show).
                    DisplayMode::HiddenLine => mesh.with_color(theme::lin(0xffffff)).unlit(),
                };
                // (The renderer draws a preview's back faces under the datum
                // planes and picked regions and its front faces over them.)
                let mesh = if opts.preview_bodies.contains(&b.id) { mesh.preview() } else { mesh.pick_solid(!opts.pick_through_bodies.contains(&b.id)) };
                let mesh = if opts.ghost_bodies.contains(&b.id) { mesh.ghost(opts.ghost_mix.unwrap_or(ISOLATE_GHOST)) } else { mesh };
                // The cap hatch's id: live's Part Studio parts read 230, 234,
                // 238, … (occurrence id + mesh group hash; live ui (10-26),
                // icons2 caps 3).
                let mesh = mesh.hatch_id(230 + 4 * part_index as u32);
                // A solid's faces (not an imported mesh's, whose winding nobody checked): back faces culled where
                // nothing cuts or shows through it (the renderer decides).
                let plain = !opts.preview_bodies.contains(&b.id) && !opts.ghost_bodies.contains(&b.id);
                let mesh = if b.kind == BodyKind::Solid && plain && cached.mesh_lines.is_none() && replaced.is_none() { mesh.closed() } else { mesh };
                // Parts excluded from the section view are not cut.
                scene.instances.push(if opts.section_skip.contains(&b.id) { mesh.ignore_clip() } else { mesh });
            }
            let edge_color = match (b.kind, opts.display_mode) {
                (BodyKind::Wire, _) => theme::lin(theme::viewport::EDGE),
                (_, DisplayMode::HiddenLine) => theme::lin(theme::viewport::EDGE_UNSHADED),
                _ => theme::lin(EDGE_COLOR),
            };
            let edge_color = if opts.ghost_bodies.contains(&b.id) { ghost_color(edge_color, opts.ghost_mix.unwrap_or(ISOLATE_GHOST)) } else { edge_color };
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
                                sharp.push_polyline(pl, [0, 0, 0, 255], ei as u32 + 1, noshape_render::scene::PART_EDGE_WIDTH, 0);
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
                        // Onshape's hidden edge pass: through the part, the edge color mixed 85 % toward the
                        // background (HIDDEN_LINE_FADE_AMOUNT), solid (live 11-02: light grey, white over faces).
                        for pl in &gmesh.edges {
                            l.push_polyline(pl, [217, 217, 217, 255], 0, 1.0, 0);
                        }
                        let h = up.upload_lines(&l);
                        cache.hidden_edges.insert(key, (Arc::downgrade(gmesh), h.clone()));
                        h
                    }
                };
                scene.instances.push(Instance::new(Drawable::Lines(handle)).per_primitive_color().unpickable().hidden_only().layer(1));
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

    // ---------------------------------------------------------------- decals
    // Live (behaviours "icons2 decal rendering"): the image on its face,
    // unlit, texel by texel, on both sides, with the part's opacity.
    let mut decal_instances = Vec::new();
    for dec in &display.decals {
        let Some(b) = display.bodies.iter().find(|b| b.id == dec.body && !b.hidden) else { continue };
        let face = Reference::Face((*dec.face).clone());
        let Some(fi) = b.face_refs.iter().position(|r| *r == face) else { continue };
        let Some(bytes) = &dec.image_bytes else { continue };
        let key = Arc::as_ptr(bytes) as usize;
        let rgba = match cache.decal_images.get(&key) {
            Some((_, img)) => img.clone(),
            None => {
                let img = image::load_from_memory(bytes).ok().map(|i| {
                    let i = i.to_rgba8();
                    Arc::new((i.width(), i.height(), i.into_raw()))
                });
                cache.decal_images.insert(key, (Arc::downgrade(bytes), img.clone()));
                img
            }
        };
        let (Some(rgba), Some(&(start, count))) = (rgba, b.mesh.face_ranges.get(fi)) else { continue };
        let m = &b.mesh;
        let mut t = TexturedData { width: rgba.0, height: rgba.1, rgba: rgba.2.clone(), ..Default::default() };
        let mut local: HashMap<u32, u32> = HashMap::new();
        let mut uv_of = |i: u32, t: &mut TexturedData| -> Option<u32> {
            if let Some(&k) = local.get(&i) {
                return Some(k);
            }
            let p = *m.positions.get(i as usize)?;
            let uv = dec.uv_at(DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
            t.positions.push(p);
            // Texture rows run top down; the image's v runs up.
            t.uvs.push([uv[0] as f32, (1.0 - uv[1]) as f32]);
            let k = t.positions.len() as u32 - 1;
            local.insert(i, k);
            Some(k)
        };
        // On a cylinder one turn moves (u, v) by 2πr·(l00, l10): a triangle
        // across the angle's wrap (in the gap opposite the image, outside
        // it) spans about that much and would stretch the image over itself.
        let wrap = match dec.mapping {
            noshape_model::regen::context::DecalMapping::Cylindrical { radius, .. } => {
                let half = std::f64::consts::PI * radius;
                Some([(half * dec.uv_linear[0][0].abs()) as f32, (half * dec.uv_linear[1][0].abs()) as f32])
            }
            noshape_model::regen::context::DecalMapping::Planar { .. } => None,
        };
        for tri in m.indices[start as usize..(start + count) as usize].as_chunks::<3>().0 {
            let Some(k) = tri.iter().map(|&i| uv_of(i, &mut t)).collect::<Option<Vec<u32>>>() else { continue };
            if let Some(half_turn) = wrap {
                let across = (0..2).any(|c| {
                    let (lo, hi) = k.iter().map(|&j| t.uvs[j as usize][c]).fold((f32::MAX, f32::MIN), |(a, b), u| (a.min(u), b.max(u)));
                    half_turn[c] > 1e-6 && hi - lo > half_turn[c]
                });
                if across {
                    continue;
                }
            }
            t.indices.extend(k);
        }
        if t.indices.is_empty() {
            continue;
        }
        let inst = Instance::new(Drawable::Image(up.upload_textured(&t))).with_color([1.0, 1.0, 1.0, b.color[3]]).unpickable().layer(9);
        decal_instances.push((scene.instances.len(), face, dec.feature));
        scene.instances.push(if opts.section_skip.contains(&b.id) { inst.ignore_clip() } else { inst });
    }
    if opts.body_layer != 0 {
        for i in &mut scene.instances[bodies_from..] {
            i.compare = opts.body_layer;
        }
    }

    // ---------------------------------------------------------------- planes
    if opts.show_planes {
        for pl in display.planes.iter().filter(|p| !p.hidden) {
            let id = picks.push(PickTarget { reference: pl.reference.clone(), kind: PickKind::Plane, anchor: Some(pl.frame.origin) });
            let f = &pl.frame;
            let c = |u: f64, v: f64| -> [f32; 3] { pl.corner(u, v).as_vec3().into() };
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
            border.push_polyline(&loop_pts, [0, 0, 0, 0], id, 2.0, LineSegment::NO_HOLD | LineSegment::WIDE_PICK);
            // Onshape: CONSTRUCTION_PLANE_AMBIENT_COLOR at CONSTRUCTION_PLANE_OPACITY, outline at 35%.
            let fill = theme::lin(theme::pick(theme::viewport::PLANE_FILL, theme::viewport_dark::PLANE_FILL));
            let outline = theme::lin(theme::pick(theme::viewport::PLANE_OUTLINE, theme::viewport_dark::PLANE_OUTLINE));
            let outline_alpha = theme::pick(theme::viewport::PLANE_OUTLINE_ALPHA, theme::viewport_dark::PLANE_OUTLINE_ALPHA) * if opts.faint_planes { FAINT_PLANE_ALPHA } else { 1.0 };
            if !opts.faint_planes {
                scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&mesh))).with_color([fill[0], fill[1], fill[2], theme::pick(theme::viewport::PLANE_FILL_ALPHA, theme::viewport_dark::PLANE_FILL_ALPHA)]).ignore_clip().unlit().layer(5));
            }
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&border))).with_color([outline[0], outline[1], outline[2], outline_alpha]).ignore_clip().pick_behind().layer(5));
            // Its name in the plane at the top-left corner, inside: over the
            // plane's fill, hidden by parts in front (live selection/02);
            // Isolate takes the names away (qa5 KB3). A FeatureScript feature's
            // planes have none (live 11-02, "Leave planes").
            if !opts.faint_planes && !matches!(pl.reference, Reference::ConstructionPlane { .. }) {
                let asc = 0.031 * 2.0 * pl.half_height;
                // Live dk-921, psui/m-01 (light, same camera): the name's
                // ink starts 0.11 of the ascender in from the corner, its
                // baseline 1.0 down (Front, Top, Right within 1 px).
                let base = pl.corner(-1.0, 1.0) + f.x * (0.11 * asc) - f.y * asc;
                let text = theme::lin(theme::pick(theme::viewport::PLANE_TEXT, theme::viewport_dark::PLANE_TEXT));
                let to_world = |q: glam::DVec2| base + f.x * (q.x * asc) + f.y * (q.y * asc);
                let contours = plane_name_contours(&pl.name);
                if let Some(m) = noshape_render::overlay::fill_mesh(&contours, to_world, f.normal) {
                    // Just under 1: drawn with the see-through meshes, so it
                    // writes no depth and ambient occlusion leaves it alone
                    // (it darkened the name 8 %); parts in front still hide it.
                    scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&m))).with_color([text[0], text[1], text[2], 0.998]).unlit().unpickable().ignore_clip().over_transparent().layer(5));
                }
            }
        }
    }

    // ---------------------------------------------------------------- origin
    if opts.show_origin && display.show_origin {
        let id = picks.push(PickTarget { reference: Reference::Origin, kind: PickKind::Origin, anchor: Some(DVec3::ZERO) });
        // Onshape draws the origin as an 11 px ring with a center dot (point style 4).
        // While sketching it stays over the edge-on planes crossing it (live
        // sketch/06 shows the whole ring and dot).
        let style = 4 | if opts.editing_sketch.is_some() { LineSegment::ON_TOP } else { 0 };
        let pts = PointData { points: vec![PointSprite { position: [0.0; 3], color: theme::rgb8(theme::pick(theme::viewport::ORIGIN, theme::viewport_dark::ORIGIN)), pick_id: id, size: 11.0, style, _pad: 0 }] };
        scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&pts))).per_primitive_color().ignore_clip().layer(6));
    }

    // ---------------------------------------------------------------- sketches
    if opts.show_sketches || opts.editing_sketch.is_some() {
        for sk in &display.sketches {
            let editing = opts.editing_sketch == Some(sk.feature);
            // (A sketch on a sheet metal flat is drawn in the flat view: live 2468.)
            if (sk.hidden || sk.on_flat) && !editing {
                continue;
            }
            if !opts.show_sketches && !editing {
                continue;
            }
            let mut lines = LineData::default();
            for c in &sk.curves {
                // A spline's handle arm: only while the sketch is edited (live: the white handles show then).
                if c.handle && !editing {
                    continue;
                }
                let id = picks.push(PickTarget { reference: Reference::SketchEntity { sketch: sk.feature, entity: c.entity }, kind: PickKind::SketchCurve, anchor: None });
                // (A handle arm, 1 px beside its 2 px spline, wins under the cursor.)
                if c.handle {
                    picks.boost(id);
                }
                // Custom sketch appearance (Edit sketch appearance) when not editing.
                let color = match sk.color.filter(|_| !editing) {
                    Some(c) => lin_to_u8(c),
                    // (A handle arm: Onshape's thin grey handle line, sketch/100–101.)
                    None if c.handle => theme::rgb8(theme::viewport::SKETCH_HANDLE_LINE),
                    None => sketch_color(&opts.graphics, c.state, c.construction, editing, c.external),
                };
                // Hovered / selected it keeps a dark core like a part edge
                // (live sketch/21, 22).
                // (A handle arm on top: it runs along its spline from the end, and under it it couldn't be picked.)
                let style = LineSegment::CORE | if c.handle { LineSegment::ON_TOP } else if c.construction { LineSegment::CONSTRUCTION } else { 0 };
                // Graphics preferences: active 2 px, inactive 1 px by default.
                let width = if c.handle { 1.0 } else if editing { opts.graphics.sketch_line_width } else { opts.graphics.inactive_line_width };
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
                // Live (sketch/07, 28; with the anti-aliased rim): vertices of
                // the edited sketch are 6.7 px dots while under-defined (blue),
                // 6 px once defined.
                let vertex = if p.state == SketchEntityState::Under { 6.7 } else { 6.0 };
                // (A spline's handle tip: Onshape's 9 px hollow handle in the point's colour, on top; lit, its disc.)
                let size = if p.standalone { 7.0 } else if !editing { 0.0 } else if p.tip { 9.0 } else if p.handle { 5.0 } else { vertex };
                // Standalone points and spline handles are hollow (live sketch/100, Point tool).
                let style = if p.tip { PointSprite::HANDLE_TIP | LineSegment::ON_TOP } else if p.standalone || p.handle { PointSprite::HOLLOW } else { 0 };
                let style = style | PointSprite::SKETCH_POINT | if p.center { PointSprite::CENTER } else { 0 };
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
                for c in sk.curves.iter().filter(|c| c.state == SketchEntityState::Under && !c.external && !c.handle) {
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
            // (While editing, not the face's imprinted pieces: only the sketch's own loops are filled.)
            for r in sk.regions.iter().filter(|r| !(editing && r.imprinted)) {
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
                // rgb(245,248,250): 0.265). Dark: `#f2f2f2` at 20%
                // (`--os-sketch-face-color`), #616161 over one plane's #3d3d3d.
                let fill = if editing {
                    let c = theme::lin(theme::pick(theme::viewport::SKETCH_REGION, theme::viewport_dark::SKETCH_REGION));
                    // Flat: lit, the fill came out darker and grayer
                    // ((215,218,221) over the plane).
                    Instance::new(Drawable::Mesh(up.upload_mesh(&mesh))).with_color([c[0], c[1], c[2], theme::pick(0.265, 0.2)]).unlit()
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
        // Drawn by `connector_glyphs` for each frame's camera; these short
        // axes only take the picks.
        let mut l = LineData::default();
        let len = 0.004f64;
        for axis in [mc.x, y, mc.z, -mc.x, -y] {
            let tip: [f32; 3] = (mc.origin + axis * len).as_vec3().into();
            l.segments.push(LineSegment { a: o, b: tip, color: [0, 0, 0, 0], pick_id: id, width: 12.0, style: LineSegment::ON_TOP });
        }
        scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&l))).per_primitive_color().pick_only().ignore_clip().layer(12));
    }

    let mut planes = opts.section.iter().map(|(n, d)| noshape_render::ClipPlane::new(n.as_vec3(), *d as f32));
    scene.clip = planes.next();
    scene.clip_extra = planes.take(2).collect();
    if let Some(slot) = opts.section_selected.and_then(|i| scene.clip_selected.get_mut(i)) {
        *slot = true;
    }
    let static_instances = scene.instances.len();
    scene.states = vec![0; picks.len() + 1];
    BuiltScene { scene, picks, static_instances, decals: decal_instances, consumed_from: u32::MAX }
}

/// While a dialog previews: the edges of `base` (the model before the feature) the preview no longer has, as pick-only
/// lines with picks of their own, so a click where a consumed edge was still reaches it (live: a fillet's selected edge
/// clicked again leaves the field).
pub fn add_consumed_edge_picks(built: &mut BuiltScene, base: &StudioDisplay, up: &Uploader) {
    let mut lines = LineData::default();
    built.consumed_from = built.picks.len() as u32 + 1;
    for b in base.bodies.iter().filter(|b| !b.hidden) {
        for (e, pl) in b.edge_refs.iter().zip(&b.mesh.edges) {
            if !built.picks.ids_for(e).is_empty() {
                continue;
            }
            let id = built.picks.push(PickTarget { reference: e.clone(), kind: PickKind::Edge, anchor: None });
            lines.push_polyline(pl, [0, 0, 0, 0], id, noshape_render::scene::PART_EDGE_WIDTH, 0);
        }
    }
    if lines.segments.is_empty() {
        return;
    }
    built.scene.instances.insert(built.static_instances, Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().pick_only().layer(1));
    built.static_instances += 1;
    built.scene.states.resize(built.picks.len() + 1, 0);
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

/// A plane name's glyph outlines (ascender 1), laid out once per name.
fn plane_name_contours(name: &str) -> Arc<Vec<Vec<glam::DVec2>>> {
    thread_local! {
        static CACHE: std::cell::RefCell<HashMap<String, Arc<Vec<Vec<glam::DVec2>>>>> = Default::default();
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(t) = c.get(name) {
            return t.clone();
        }
        // Renames could grow it without bound.
        if c.len() > 512 {
            c.clear();
        }
        let t = Arc::new(noshape_sketch::text::layout_text(name, noshape_sketch::text::DEFAULT_FONT, false, false).polylines(6));
        c.insert(name.to_string(), t.clone());
        t
    })
}

/// A rigid 3x4 row-major placement (`SharedMesh::transform`) as a matrix.
fn placement_matrix(m: &[f64; 12]) -> glam::Mat4 {
    let c = |i: usize| m[i] as f32;
    glam::Mat4::from_cols(glam::Vec4::new(c(0), c(4), c(8), 0.0), glam::Vec4::new(c(1), c(5), c(9), 0.0), glam::Vec4::new(c(2), c(6), c(10), 0.0), glam::Vec4::new(c(3), c(7), c(11), 1.0))
}

/// Per-vertex local face id (1-based) from face index ranges.
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

/// The display levels [`build`] draws for each shown body that aren't uploaded yet, uploaded on every core (NIST's
/// 400 bodies took ~330 ms on one, mostly their silhouette edges); returns how many.
fn upload_bodies_in_parallel(display: &StudioDisplay, opts: &SceneOptions, cache: &mut GpuCache, up: &Uploader) -> usize {
    let mut todo: Vec<Arc<noshape_model::display::MeshBuffers>> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for b in display.bodies.iter().filter(|b| !b.hidden) {
        let colors = opts.vertex_colors.as_ref().and_then(|m| m.get(&b.id)).filter(|c| c.len() == b.mesh.positions.len());
        let replaced = opts.replaced_meshes.as_ref().and_then(|m| m.get(&b.id));
        let shared = SharedMesh::of(display, b).filter(|_| colors.is_none() && replaced.is_none() && !noshape_render::renderer::instancing_disabled());
        let finest = shared.map_or(&b.mesh, |s| &s.mesh);
        let coarser: &[Arc<noshape_model::display::MeshBuffers>] = if colors.is_some() || replaced.is_some() { &[] } else { shared.map_or(&b.coarser[..], |s| &s.coarser[..]) };
        for m in coarser.iter().chain(std::iter::once(finest)) {
            let key = Arc::as_ptr(m) as usize;
            if !cache.bodies.contains_key(&key) && seen.insert(key) {
                todo.push(m.clone());
            }
        }
    }
    if todo.len() < 2 {
        return 0;
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(todo.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done: Vec<(usize, CachedBody)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(m) = todo.get(i) else { break };
                        out.push((i, upload_body(m, up)));
                    }
                    out
                })
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect()
    });
    let n = done.len();
    for (i, body) in done {
        let m = &todo[i];
        cache.bodies.insert(Arc::as_ptr(m) as usize, (Arc::downgrade(m), Arc::new(body)));
    }
    n
}

fn upload_body(m: &noshape_model::display::MeshBuffers, up: &Uploader) -> CachedBody {
    let pick_ids = face_pick_ids(m);

    let mut lines = LineData::default();
    lines.segments.reserve(m.edges.iter().map(|e| e.len().saturating_sub(1)).sum());
    for (ei, pl) in m.edges.iter().enumerate() {
        // Onshape DEFAULT_LINE_WIDTH 1 (live EDGE-HIGHLIGHT-LOOK: a plain edge is 1 CSS px, antialiased).
        lines.push_polyline(pl, [0, 0, 0, 255], ei as u32 + 1, 1.0, LineSegment::CORE);
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
/// the viewer and the other doesn't, like the part's edges (`PART_EDGE_WIDTH`).
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
                    Some(other) if other.dot(n) < 1.0 - 1e-6 => out.segments.push(SilhouetteSegment { a: m.positions[a as usize], b: m.positions[b as usize], n1: other.into(), n2: n.into(), pick_id: fi as u32 + 1, width: noshape_render::scene::PART_EDGE_WIDTH }),
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
/// part-studio/2022, 2028): its inputs (`selection`) are drawn red instead of
/// the orange selection: edges and face outlines red, sketch regions
/// red-outlined with a light red fill, no face fill. `accent` (hovered parts
/// and rows) keeps its colour.
#[allow(clippy::too_many_arguments)]
pub fn with_highlights(built: &BuiltScene, display: &StudioDisplay, selection: &[Reference], hover: Option<&Reference>, accent: &[Reference], extra: Vec<Instance>, error: bool, up: &Uploader) -> Scene {
    let _t = noshape_render::perf::scope("s.with_highlights");
    let mut scene = Scene { instances: built.scene.instances[..built.static_instances].to_vec(), states: vec![0; built.picks.len() + 1], clip: built.scene.clip, clip_extra: built.scene.clip_extra.clone(), clip_selected: built.scene.clip_selected, clip_hovered: [false; 3], compare: built.scene.compare, preview_mix: built.scene.preview_mix, section_interference: built.scene.section_interference, hover_outline: false, no_occlusion: built.scene.no_occlusion };
    let mut overlay_pts = PointData::default();
    let mut overlay_regions: Vec<(MeshData, [f32; 4], bool)> = Vec::new();
    let mut overlay_lines = LineData::default();
    let sel_color = theme::lin(if error { theme::viewport::ERROR } else { theme::viewport::SELECTION });
    let hov_color = theme::lin(theme::viewport::PREHIGHLIGHT);
    // Onshape outlines hovered and selected faces (their boundary edges
    // light orange / amber) instead of tinting a hovered face.
    // Lit faces (pick id, flag): their boundary edges are found from their ids after the marks.
    let mut lit_faces: Vec<(u32, u32)> = Vec::new();
    // Faces without edges (a mesh part's mesh face) get the part's
    // silhouette outline instead (live G35: light orange on hover, orange
    // around the selected part).
    let mut silhouette: Vec<u32> = Vec::new();

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
                // Live (dialogs/extrude-02, 03): 3 rows of colour on a 30°
                // edge (~2.6 px), hovered or picked.
                overlay_lines.segments.push(LineSegment { a: flat(a), b: flat(b), color: lin_to_u8(c), pick_id: 0, width: 3.0, style: 0 });
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
        // A hovered or selected datum plane (live behaviours "icons2
        // plane/origin highlight", selection/01): a ~3 px solid border,
        // #ffc685 hovered / #ffa90a selected; selected, a solid amber fill
        // too. Hover leaves the fill as it is (the plane's own state isn't
        // set, it would tint the fill).
        let plane_lit = (flag == state::SELECTED || flag == state::HOVER)
            && built.picks.ids_for(r).iter().any(|&id| built.picks.get(id).is_some_and(|t| t.kind == PickKind::Plane));
        if plane_lit && let Some(pl) = display.planes.iter().find(|p| !p.hidden && &p.reference == r) {
            let f = &pl.frame;
            let c = |u: f64, v: f64| -> [f32; 3] { pl.corner(u, v).as_vec3().into() };
            let corners = [c(-1.0, -1.0), c(1.0, -1.0), c(1.0, 1.0), c(-1.0, 1.0)];
            if flag == state::SELECTED {
                let n: [f32; 3] = f.normal.as_vec3().into();
                let k = theme::lin(theme::viewport::PLANE_SELECTED_FILL);
                overlay_regions.push((MeshData { positions: corners.to_vec(), normals: vec![n; 4], pick_ids: vec![0; 4], indices: vec![0, 1, 2, 0, 2, 3] }, [k[0], k[1], k[2], theme::viewport::PLANE_SELECTED_ALPHA], true));
            }
            let border = if flag == state::HOVER { hov_color } else { sel_color };
            for i in 0..4 {
                // Lifted: at the same depth the plane's own blue border won
                // over it; parts in front still hide it.
                overlay_lines.segments.push(LineSegment { a: corners[i], b: corners[(i + 1) % 4], color: lin_to_u8(border), pick_id: 0, width: 3.0, style: LineSegment::LIFTED });
            }
        }
        for &id in built.picks.ids_for(r) {
            let t = built.picks.get(id);
            // An erroring input face shows only its red outline; a lit
            // plane its overlay above; a hovered sketch region only its
            // boundary (live sketch/25: the fill doesn't change).
            if !(flag == state::ERROR && t.is_some_and(|t| t.kind == PickKind::Face)) && !plane_lit && !(flag == state::HOVER && t.is_some_and(|t| t.kind == PickKind::SketchRegion)) {
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
                PickKind::Face if flag & (state::HOVER | state::SELECTED | state::ERROR) != 0 => lit_faces.push((id, flag)),
                _ => {}
            }
        }
    };
    // Hovered parts and list rows, never red: the error colours only the
    // failing feature's inputs (qa PART-HOVER-RED: a part hovered for an
    // empty Split was drawn red; live's part hover is the light orange).
    for r in accent {
        mark(r, state::ACCENT, &mut scene);
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
        // A curve (its Curves row): drawn as its edges selected, as when
        // picked in the graphics (CURVE-ROW-NO-HIGHLIGHT).
        if let Reference::Body(b) = r
            && let Some(body) = display.body(b).filter(|b| b.face_refs.is_empty())
        {
            for e in &body.edge_refs {
                mark(e, if error { state::ERROR } else { state::SELECTED }, &mut scene);
            }
        }
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
    // A decal on a selected face (or of a selected Decal feature) is tinted
    // (live: hovered it isn't).
    for (i, face, feature) in &built.decals {
        if (selection.contains(face) || selection.contains(&Reference::Feature(*feature)))
            && let Some(inst) = scene.instances.get_mut(*i)
        {
            inst.decal_selected = true;
        }
    }
    // What is selected isn't pre-highlighted again (live selection/05 =
    // 04, sketch/23 = 22, "plane and mate connector hover" (c); a part edge
    // too, behaviours "psui (2026-10-27, P1) EDGE-HIGHLIGHT-LOOK" (c)).
    if let Some(h) = hover.filter(|h| !selection.contains(h)) {
        match h {
            // A hovered part (live "psui (2026-10-27, P2) PART-HOVER-RED look"): its outline in hover orange, no
            // tint, its own edges as at rest.
            // A hovered curve's row: its edges pre-highlighted, as the curve hovered in the graphics.
            Reference::Body(b) if display.body(b).is_some_and(|b| b.face_refs.is_empty()) => {
                for e in display.body(b).filter(|b| !b.hidden).map(|b| b.edge_refs.as_slice()).unwrap_or_default() {
                    mark(e, state::HOVER, &mut scene);
                }
            }
            Reference::Body(b) => {
                for f in display.body(b).filter(|b| !b.hidden).map(|b| b.face_refs.as_slice()).unwrap_or_default() {
                    for &id in built.picks.ids_for(f) {
                        scene.add_flag(id, state::HOVER | state::OUTLINE);
                        scene.hover_outline = true;
                    }
                }
            }
            h => mark(h, state::HOVER, &mut scene),
        }
    }
    let _t = noshape_render::perf::scope("s.wh_after_marks");
    // A face's boundary edges: a solid 2 device px line, not the band of an
    // edge highlighted itself (live EDGE-HIGHLIGHT-LOOK (d)).
    let mut tables: HashMap<usize, Arc<Vec<Vec<u32>>>> = HashMap::new();
    let mut boundary: Vec<(u32, u32)> = Vec::new();
    for (id, flag) in lit_faces {
        let Some((bi, fi, edge_base)) = built.picks.face_at(id) else { continue };
        let Some(b) = display.bodies.get(bi) else { continue };
        let table = tables.entry(bi).or_insert_with(|| face_edge_table(&b.mesh));
        match table.get(fi) {
            Some(edges) if !edges.is_empty() => boundary.extend(edges.iter().map(|e| (edge_base + e + 1, flag))),
            _ => silhouette.push(id),
        }
    }
    for (id, flag) in boundary {
        let own = scene.states.get(id as usize).is_some_and(|s| s & (state::HOVER | state::SELECTED | state::ERROR) != 0);
        scene.add_flag(id, if own { flag } else { flag | state::OUTLINE });
    }
    for id in silhouette {
        scene.add_flag(id, state::OUTLINE);
    }
    // A part selected whole: a light-orange silhouette round it, drawn through the parts in front too, never over
    // its own image (live2 11-14, feel/hidden-part-selection.png; the renderer's `outline_through`), as an assembly's
    // selected instance. Its edges keep their selection colour (the boundary pass above).
    let whole = state::SELECTED | state::BODY;
    let part_faces: Vec<u32> = (1..scene.states.len() as u32).filter(|&id| scene.states[id as usize] & whole == whole && built.picks.get(id).is_some_and(|t| t.kind == PickKind::Face)).collect();
    for &id in &part_faces {
        scene.add_flag(id, state::OUTLINE);
    }
    let bases: std::collections::HashSet<u32> = part_faces
        .iter()
        .copied()
        .filter_map(|id| built.picks.face_at(id).map(|(_, fi, _)| id - fi as u32 - 1))
        .collect();
    if !bases.is_empty() {
        for inst in scene.instances[..built.static_instances].iter_mut().filter(|i| matches!(i.drawable, Drawable::Mesh(_)) && i.pickable && !i.pick_only && bases.contains(&i.pick_base)) {
            inst.outline_through = true;
        }
    }
    // Selected faces drawn again at SELECTION_OVER_PLANES over the planes in
    // front of them (live selection/04: under one plane (229,170,56), two
    // (228,176,73); the plane alone over the face gives (224,174,73)). Only
    // the part meshes holding a selected face.
    let selected: Vec<u32> = (1..scene.states.len() as u32).filter(|&id| scene.states[id as usize] & state::SELECTED != 0 && built.picks.get(id).is_some_and(|t| t.kind == PickKind::Face)).collect();
    if !selected.is_empty() {
        let mut bodies: Vec<&Instance> = scene.instances[..built.static_instances]
            .iter()
            .filter(|i| matches!(i.drawable, Drawable::Mesh(_)) && i.pickable && !i.pick_only && !i.back_silhouette && !i.preview && !i.unlit && i.color[3] >= 0.999)
            .collect();
        bodies.sort_by_key(|i| i.pick_base);
        let mut again = Vec::new();
        for (k, b) in bodies.iter().enumerate() {
            let next = bodies.get(k + 1).map_or(u32::MAX, |n| n.pick_base);
            if selected.iter().any(|&s| s > b.pick_base && (s <= next || next == b.pick_base)) {
                let c = b.color;
                again.push((*b).clone().with_color([c[0], c[1], c[2], SELECTION_OVER_PLANES]).over_transparent().selected_only());
            }
        }
        scene.instances.extend(again);
    }
    if !overlay_pts.points.is_empty() {
        scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&overlay_pts))).per_primitive_color().unpickable().ignore_clip().layer(20));
    }
    for (m, c, picked) in overlay_regions {
        let inst = Instance::new(Drawable::Mesh(up.upload_mesh(&m))).with_color(c).unpickable().ignore_clip().layer(HIGHLIGHT_FILL_LAYER);
        // A picked region tints the preview built on it from above (live
        // part-studio/230): flat, after the see-through meshes.
        scene.instances.push(if picked { inst.unlit().over_transparent() } else { inst });
    }
    if !overlay_lines.segments.is_empty() {
        scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&overlay_lines))).per_primitive_color().unpickable().ignore_clip().layer(HIGHLIGHT_LINES_LAYER));
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
                // Through the model, as live (a debugged face behind the part shows).
                let m = MeshData { positions: mesh.positions.clone(), normals: mesh.normals.clone(), pick_ids: vec![0; mesh.positions.len()], indices: mesh.indices.clone() };
                let mut fill = noshape_render::srgb(*r, *g, *b);
                fill[3] = 0.5;
                out.push(Instance::new(Drawable::Mesh(up.upload_mesh(&m))).with_color(fill).on_top().unpickable().ignore_clip().layer(8));
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

/// A sketch region whose chip is hovered in a dialog (live CHIP-HOVER: the
/// entity in the query-hover yellow-green): the region filled flat in that
/// colour over everything see-through, like a picked region's fill.
pub fn query_region(display: &StudioDisplay, r: &Reference, up: &Uploader) -> Option<Instance> {
    let Reference::SketchRegion { sketch, key } = r else { return None };
    let sk = display.sketches.iter().find(|s| s.feature == *sketch)?;
    let reg = sk.regions.iter().find(|g| &g.key == key)?;
    let n: [f32; 3] = sk.frame.normal.as_vec3().into();
    let k = theme::lin(theme::viewport::QUERY_HOVER);
    let m = MeshData { positions: reg.positions.clone(), normals: vec![n; reg.positions.len()], pick_ids: vec![0; reg.positions.len()], indices: reg.indices.clone() };
    Some(Instance::new(Drawable::Mesh(up.upload_mesh(&m))).with_color([k[0], k[1], k[2], 0.7]).unpickable().ignore_clip().layer(8).unlit().over_transparent())
}

/// Create selection's result faces (live part-studio/387): see-through
/// yellow-green over the part, which leaves them out (`state::HIDDEN`), so
/// what lies behind them shows (a hole's walls). Not pickable (the hidden
/// faces' pick ids would hide it too): a click there reaches what is behind.
pub fn query_faces(display: &StudioDisplay, faces: &[Reference], up: &Uploader) -> Option<Instance> {
    let mut m = MeshData { positions: Vec::new(), normals: Vec::new(), pick_ids: Vec::new(), indices: Vec::new() };
    for b in display.bodies.iter().filter(|b| !b.hidden) {
        for (fi, r) in b.face_refs.iter().enumerate() {
            if !faces.contains(r) {
                continue;
            }
            let Some(&(first, count)) = b.mesh.face_ranges.get(fi) else { continue };
            for &ix in b.mesh.indices.get(first as usize..(first + count) as usize).unwrap_or(&[]) {
                let i = ix as usize;
                let (Some(p), Some(n)) = (b.mesh.positions.get(i), b.mesh.normals.get(i)) else { continue };
                m.indices.push(m.positions.len() as u32);
                m.positions.push(*p);
                m.normals.push(*n);
                m.pick_ids.push(0);
            }
        }
    }
    if m.indices.is_empty() {
        return None;
    }
    let k = theme::lin(theme::viewport::QUERY_HOVER);
    Some(Instance::new(Drawable::Mesh(up.upload_mesh(&m))).with_color([k[0], k[1], k[2], QUERY_FACE_ALPHA]).unpickable())
}

/// How opaque Create selection's see-through result faces are.
pub const QUERY_FACE_ALPHA: f32 = 0.5;

/// The boundary edges of `faces` where other geometry hides them, dashed in
/// the selection color (live "lp wall over" 2258: the seed of a failed
/// Feature pattern behind the block).
pub fn hidden_edges_dashed(display: &StudioDisplay, faces: &[Reference], up: &Uploader) -> Option<Instance> {
    let edges: std::collections::HashSet<Reference> = faces_edges(display, faces).into_iter().flatten().flatten().collect();
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

/// The silhouette of `faces` in the hover orange: their edges
/// where one neighbouring face turns toward the viewer and the other away,
/// and their curved faces' own silhouettes (live selection/10: a feature
/// hovered in the list outlines its part with ~2 px of #ffc685 on the
/// outline itself; the inner edges stay black). A face boundary used by one
/// face only (a sheet's) always shows.
pub fn hover_silhouette(display: &StudioDisplay, faces: &[Reference], up: &Uploader) -> Option<Instance> {
    silhouette(display, faces, theme::viewport::PREHIGHLIGHT, 2.6, up)
}

/// The silhouette of `faces` (see [`hover_silhouette`]) in `color`, `width` px.
pub fn silhouette(display: &StudioDisplay, faces: &[Reference], color: u32, width: f32, up: &Uploader) -> Option<Instance> {
    silhouette_data(display, faces, width).map(|d| silhouette_instance(&d, color, up))
}

/// [`silhouette`]'s segments, without the GPU (a background task's part: 20-28 ms for a whole part of c2-bottle-split).
pub fn silhouette_data(display: &StudioDisplay, faces: &[Reference], width: f32) -> Option<SilhouetteData> {
    let want: std::collections::HashSet<&Reference> = faces.iter().collect();
    let q = |p: &[f32; 3]| [(p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64, (p[2] * 1e6).round() as i64];
    let mut out = SilhouetteData::default();
    for b in display.bodies.iter().filter(|b| !b.hidden) {
        let m = &b.mesh;
        let lit: Vec<bool> = (0..m.face_ranges.len()).map(|fi| b.face_refs.get(fi).is_some_and(|r| want.contains(r))).collect();
        if !lit.contains(&true) {
            continue;
        }
        out.segments.extend(silhouette_edges(m).segments.into_iter().filter(|s| lit.get(s.pick_id as usize - 1) == Some(&true)).map(|s| SilhouetteSegment { pick_id: 0, width, ..s }));
        // Each face's boundary sides (used once within the face, by
        // position: a periodic face's seam is used twice) with the facet
        // normal, then paired across faces.
        let mut sides: HashMap<EdgeKey, Vec<BoundarySide>> = HashMap::new();
        for (fi, &(start, count)) in m.face_ranges.iter().enumerate() {
            let mut own: HashMap<EdgeKey, (u32, glam::Vec3, [f32; 3], [f32; 3])> = HashMap::new();
            for t in m.indices[start as usize..(start + count) as usize].as_chunks::<3>().0 {
                let Some(p) = t.iter().map(|&i| m.positions.get(i as usize).copied()).collect::<Option<Vec<[f32; 3]>>>() else { continue };
                let v = |i: usize| glam::Vec3::from_array(p[i]);
                let n = (v(1) - v(0)).cross(v(2) - v(0)).normalize_or_zero();
                if n == glam::Vec3::ZERO {
                    continue;
                }
                for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                    let (qa, qb) = (q(&p[i]), q(&p[j]));
                    let key = if qa <= qb { (qa, qb) } else { (qb, qa) };
                    own.entry(key).or_insert((0, n, p[i], p[j])).0 += 1;
                }
            }
            for (key, (uses, n, a, b)) in own {
                if uses == 1 {
                    sides.entry(key).or_default().push((fi, n, a, b));
                }
            }
        }
        for users in sides.values() {
            let Some(&(fi, n1, a, b)) = users.iter().find(|u| lit[u.0]) else { continue };
            let n2 = users.iter().find(|u| u.0 != fi).map_or(-n1, |u| u.1);
            out.segments.push(SilhouetteSegment { a, b, n1: n1.into(), n2: n2.into(), pick_id: 0, width });
        }
    }
    (!out.segments.is_empty()).then_some(out)
}

/// [`silhouette_data`] uploaded, in `color`.
pub fn silhouette_instance(data: &SilhouetteData, color: u32, up: &Uploader) -> Instance {
    let k = theme::lin(color);
    Instance::new(Drawable::Lines(up.upload_silhouettes(data))).with_color([k[0], k[1], k[2], 1.0]).unpickable().layer(12)
}

/// A mate connector's state for its glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectorLook {
    Rest,
    Hovered,
    Selected,
}

/// Mate connector glyphs, built for each frame's camera (constant screen
/// size, live behaviours "icons2 (10-26) plane and mate connector hover"):
/// a ring between 12.5 and 18 px in the connector's XY plane and the wedge
/// between +X and +Y filled (white at rest, MATE_CONNECTOR_PRE_SELECTED /
/// SELECTED amber hovered / selected), outlined (dark at rest, navy
/// `MATE_CONNECTOR_STROKE` lit), a blue Z axis up from the centre and short
/// red X / green Y ticks outside the ring.
pub struct ConnectorGlyphs {
    items: Vec<(DVec3, DVec3, DVec3, DVec3, ConnectorLook)>,
    /// Each glyph's pick id (none: unpickable): the whole disc takes the pointer, in its plane (an assembly's
    /// connectors: live 3d.json, a reach of 23 px across and 14 up and down at a tilt).
    picks: Vec<u32>,
}

impl ConnectorGlyphs {
    /// Glyphs at frames `(origin, x, z)`, each with its look and pick id.
    pub fn picked(items: impl IntoIterator<Item = (DVec3, DVec3, DVec3, ConnectorLook, u32)>) -> Arc<Self> {
        let (items, picks) = items.into_iter().map(|(o, x, z, look, pick)| ((o, x, z.cross(x), z, look), pick)).unzip();
        Arc::new(Self { items, picks })
    }
}

// Onshape's MATE_CONNECTOR_DIAMETER_PX 36 and RING_WIDTH_PX 4 (behaviours "psui (2026-10-27, P2) interaction 3.4").
const MC_R_IN_PX: f64 = 14.0;
const MC_R_OUT_PX: f64 = 18.0;
const MC_RIM_PX: f32 = 1.3;
/// Fill (sRGB) and opacity, each over what is behind the disc (live over a face (142,187,214): rest (232,241,247),
/// hover (187,190,134), selected (220,204,90)): MATE_CONNECTOR_COLOR rgba(255,255,255,0.8), PRE_SELECTED
/// rgba(255,194,14,0.4) and SELECTED rgba(255,194,14,0.5) instead of the white.
const MC_FILL_REST: ([u8; 3], f32) = ([255, 255, 255], 0.8);
const MC_FILL_HOVER: ([u8; 3], f32) = ([255, 194, 14], 0.4);
const MC_FILL_SELECTED: ([u8; 3], f32) = ([255, 194, 14], 0.5);
const MC_RIM_REST: [u8; 4] = [40, 44, 46, 255];
const MC_RIM_LIT: [u8; 4] = [0, 40, 71, 255];

impl noshape_render::camera_geometry::CameraGeometry for ConnectorGlyphs {
    /// Live 1701: a glyph is in the scene bounds the zoom limits use: the
    /// disc ±18 px, the triad's arms out to +33 px in x / y and +23 px in z,
    /// at the pixel size at its origin.
    fn zoom_points(&self, view: &noshape_render::camera_geometry::View) -> Vec<DVec3> {
        let mut out = Vec::new();
        for &(o, x, y, z, _) in &self.items {
            let Some(px) = view.world_per_px(o) else { continue };
            out.extend([o - x * (MC_R_OUT_PX * px), o - y * (MC_R_OUT_PX * px), o + x * (33.0 * px), o + y * (33.0 * px), o + z * (23.0 * px)]);
        }
        out
    }

    fn build(&self, view: &noshape_render::camera_geometry::View, up: &Uploader) -> Vec<Instance> {
        let mut out = Vec::new();
        let f = |p: DVec3| -> [f32; 3] { p.as_vec3().into() };
        for (i, &(o, x, y, z, look)) in self.items.iter().enumerate() {
            let Some(px) = view.world_per_px(o) else { continue };
            let (ri, ro) = (MC_R_IN_PX * px, MC_R_OUT_PX * px);
            let at = |r: f64, a: f64| o + x * (r * a.cos()) + y * (r * a.sin());
            let n = 64usize;
            let ang = |k: usize| k as f64 / n as f64 * std::f64::consts::TAU;
            // Fill: the ring, plus the wedge from +X to +Y (0..90°).
            let mut m = MeshData { positions: Vec::new(), normals: Vec::new(), pick_ids: Vec::new(), indices: Vec::new() };
            let quad = |a: DVec3, b: DVec3, c: DVec3, d: DVec3, m: &mut MeshData| {
                let base = m.positions.len() as u32;
                m.positions.extend([f(a), f(b), f(c), f(d)]);
                m.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            };
            for k in 0..n {
                let (a0, a1) = (ang(k), ang(k + 1));
                quad(at(ri, a0), at(ro, a0), at(ro, a1), at(ri, a1), &mut m);
                if a1 <= std::f64::consts::FRAC_PI_2 + 1e-9 {
                    quad(o, at(ri, a0), at(ri, a1), o, &mut m);
                }
            }
            let nrm: [f32; 3] = z.as_vec3().into();
            m.normals = vec![nrm; m.positions.len()];
            m.pick_ids = vec![0; m.positions.len()];
            let (rgb, alpha) = match look {
                ConnectorLook::Rest => MC_FILL_REST,
                ConnectorLook::Hovered => MC_FILL_HOVER,
                ConnectorLook::Selected => MC_FILL_SELECTED,
            };
            let c = theme::lin(u32::from(rgb[0]) << 16 | u32::from(rgb[1]) << 8 | u32::from(rgb[2]));
            out.push(Instance::new(Drawable::Mesh(up.upload_mesh(&m))).with_color([c[0], c[1], c[2], alpha]).unlit().unpickable().ignore_clip().on_top().layer(30));
            if let Some(&pick) = self.picks.get(i).filter(|p| **p != 0) {
                out.push(Instance::new(Drawable::Mesh(up.upload_mesh(&pick_disc(o, x, y, z, ro, pick)))).pick_only().ignore_clip().on_top().layer(30));
            }
            // Outline: outer circle, the inner circle outside the wedge, the
            // wedge's two sides.
            let rim = if look == ConnectorLook::Rest { MC_RIM_REST } else { MC_RIM_LIT };
            let mut l = LineData::default();
            let outer: Vec<[f32; 3]> = (0..=n).map(|k| f(at(ro, ang(k)))).collect();
            let inner: Vec<[f32; 3]> = (n / 4..=n).map(|k| f(at(ri, ang(k)))).collect();
            l.push_polyline(&outer, rim, 0, MC_RIM_PX, LineSegment::ON_TOP);
            l.push_polyline(&inner, rim, 0, MC_RIM_PX, LineSegment::ON_TOP);
            l.push_polyline(&[f(o), f(at(ro, 0.0))], rim, 0, MC_RIM_PX, LineSegment::ON_TOP);
            l.push_polyline(&[f(o), f(at(ro, std::f64::consts::FRAC_PI_2))], rim, 0, MC_RIM_PX, LineSegment::ON_TOP);
            // Axes: Z up from the centre, X / Y ticks outside the ring.
            let axis = |hex: u32| theme::rgb8(hex);
            l.push_polyline(&[f(o), f(o + z * (18.0 * px))], axis(theme::pick(theme::viewport::AXIS_Z, theme::viewport_dark::AXIS_Z)), 0, 3.0, LineSegment::ON_TOP);
            l.push_polyline(&[f(o + x * (19.5 * px)), f(o + x * (29.0 * px))], axis(theme::pick(theme::viewport::AXIS_X, theme::viewport_dark::AXIS_X)), 0, 2.5, LineSegment::ON_TOP);
            l.push_polyline(&[f(o + y * (19.5 * px)), f(o + y * (29.0 * px))], axis(theme::pick(theme::viewport::AXIS_Y, theme::viewport_dark::AXIS_Y)), 0, 2.5, LineSegment::ON_TOP);
            out.push(Instance::new(Drawable::Lines(up.upload_lines(&l))).per_primitive_color().unpickable().ignore_clip().layer(31));
        }
        out
    }
}

/// A glyph's pick area: the whole disc of radius `r` in its XY plane, every vertex with `pick`.
fn pick_disc(o: DVec3, x: DVec3, y: DVec3, z: DVec3, r: f64, pick: u32) -> MeshData {
    let n = 48u32;
    let f = |p: DVec3| -> [f32; 3] { p.as_vec3().into() };
    let mut d = MeshData { positions: vec![f(o)], normals: Vec::new(), pick_ids: Vec::new(), indices: Vec::new() };
    for k in 0..n {
        let a = f64::from(k) / f64::from(n) * std::f64::consts::TAU;
        d.positions.push(f(o + x * (r * a.cos()) + y * (r * a.sin())));
        d.indices.extend([0, k + 1, (k + 1) % n + 1]);
    }
    d.normals = vec![z.as_vec3().into(); d.positions.len()];
    d.pick_ids = vec![pick; d.positions.len()];
    d
}

/// The mate connectors' glyphs: `lit` gives each one's look.
pub fn connector_glyphs(display: &StudioDisplay, lit: impl Fn(&Reference) -> ConnectorLook) -> Option<Arc<ConnectorGlyphs>> {
    let items: Vec<_> = display.connectors.iter().map(|c| (c.origin, c.x, c.z.cross(c.x), c.z, lit(&c.reference))).collect();
    (!items.is_empty()).then(|| Arc::new(ConnectorGlyphs { items, picks: Vec::new() }))
}

/// Mate connector inference while hovering (live behaviours 1327, 33725, psui/hole-mc-1851/4): the hovered
/// entity's inference points as small white dots, and the one nearest the pointer as a hovered connector glyph.
/// Shared by the Part Studio (Mate connector, in-feature connectors) and the assembly (mates, snap mode).
pub struct InferenceMarks {
    marks: Vec<InferenceMark>,
    glyph: ConnectorGlyphs,
}

/// One inference point: its origin, X and Z (the plane its mark lies in), and whether it is a corner (a vertex:
/// the bigger mark).
#[derive(Clone, Copy, Debug)]
pub struct InferenceMark {
    pub origin: DVec3,
    pub x: DVec3,
    pub z: DVec3,
    pub corner: bool,
}

impl From<(DVec3, DVec3, DVec3)> for InferenceMark {
    fn from((origin, x, z): (DVec3, DVec3, DVec3)) -> Self {
        Self { origin, x, z, corner: false }
    }
}

/// Live (psui/hole-mc-1851/4): a corner's mark is a ~12 px disc, an edge midpoint's (and the other points') ~8 px,
/// both lying in the point's plane (ellipses at an angle), white with a thin grey rim.
const MARK_CORNER_PX: f64 = 6.0;
const MARK_POINT_PX: f64 = 4.0;

impl InferenceMarks {
    /// `points`: each point's mark (an `(origin, x, z)` is a non-corner one); `chosen` the one under the pointer.
    pub fn new<P: Into<InferenceMark>>(points: impl IntoIterator<Item = P>, chosen: Option<usize>) -> Arc<Self> {
        let mut marks = Vec::new();
        let mut items = Vec::new();
        for (i, m) in points.into_iter().map(Into::into).enumerate() {
            if Some(i) == chosen {
                items.push((m.origin, m.x, m.z.cross(m.x), m.z, ConnectorLook::Hovered));
            } else {
                marks.push(m);
            }
        }
        Arc::new(Self { marks, glyph: ConnectorGlyphs { items, picks: Vec::new() } })
    }
}

impl noshape_render::camera_geometry::CameraGeometry for InferenceMarks {
    fn build(&self, view: &noshape_render::camera_geometry::View, up: &Uploader) -> Vec<Instance> {
        let mut out = noshape_render::camera_geometry::CameraGeometry::build(&self.glyph, view, up);
        if self.marks.is_empty() {
            return out;
        }
        const N: usize = 24;
        let f = |p: DVec3| -> [f32; 3] { p.as_vec3().into() };
        let mut m = MeshData { positions: Vec::new(), normals: Vec::new(), pick_ids: Vec::new(), indices: Vec::new() };
        let mut rim = LineData::default();
        for mk in &self.marks {
            let Some(px) = view.world_per_px(mk.origin) else { continue };
            let (z, x) = (mk.z.normalize_or_zero(), mk.x.normalize_or_zero());
            let y = z.cross(x);
            let r = px * if mk.corner { MARK_CORNER_PX } else { MARK_POINT_PX };
            // 3 px toward the eye: over the face and its edge lines (live: the white disc covers the outline), with
            // no shift on screen; other parts in front still hide it.
            let o = mk.origin - view.camera.forward().normalize_or_zero() * (3.0 * px);
            let at = |k: usize| {
                let a = k as f64 / N as f64 * std::f64::consts::TAU;
                o + x * (r * a.cos()) + y * (r * a.sin())
            };
            let base = m.positions.len() as u32;
            m.positions.push(f(o));
            m.positions.extend((0..N).map(|k| f(at(k))));
            for k in 0..N as u32 {
                m.indices.extend([base, base + 1 + k, base + 1 + (k + 1) % N as u32]);
            }
            let ring: Vec<[f32; 3]> = (0..=N).map(|k| f(at(k % N))).collect();
            rim.push_polyline(&ring, [0x9f, 0x9f, 0x9f, 255], 0, 1.0, 0);
        }
        let n = m.positions.len();
        m.normals = vec![[0.0, 0.0, 1.0]; n];
        m.pick_ids = vec![0; n];
        let white = theme::lin(0xfdfdfd);
        out.push(Instance::new(Drawable::Mesh(up.upload_mesh(&m))).with_color([white[0], white[1], white[2], 1.0]).unlit().unpickable().ignore_clip().layer(22));
        out.push(Instance::new(Drawable::Lines(up.upload_lines(&rim))).per_primitive_color().unpickable().ignore_clip().layer(22));
        out
    }
}

/// A hovered / selected mate connector's owner part: its edges in the
/// secondary yellow-green (#f2ff80; live (235,246,146) / (208,219,117) /
/// (218,235,137) across an edge: 0.86 over the black edge and the face).
pub fn owner_secondary(display: &StudioDisplay, body: &noshape_model::BodyId, up: &Uploader) -> Option<Instance> {
    let b = display.body(body).filter(|b| !b.hidden)?;
    let mut lines = LineData::default();
    for pl in &b.mesh.edges {
        lines.push_polyline(pl, [0xf2, 0xff, 0x80, 219], 0, 3.0, LineSegment::LIFTED);
    }
    (!lines.segments.is_empty()).then(|| Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(12))
}

/// A decoded image: width, height, RGBA8 rows.
type DecodedImage = Arc<(u32, u32, Vec<u8>)>;

/// A face's boundary side: face index, facet normal, ends.
type BoundarySide = (usize, glam::Vec3, [f32; 3], [f32; 3]);

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
    faces_edges(display, std::slice::from_ref(r)).into_iter().next().flatten().unwrap_or_default()
}

/// [`face_edges`] of several faces in one pass over the bodies (a selected feature lights hundreds of faces; looked
/// up one by one, each searched every face and edge of its body). `None` for a face no shown body has.
pub fn faces_edges(display: &StudioDisplay, faces: &[Reference]) -> Vec<Option<Vec<Reference>>> {
    let mut out = vec![None; faces.len()];
    if faces.is_empty() {
        return out;
    }
    let mut wanted: HashMap<&Reference, Vec<usize>> = HashMap::new();
    for (k, f) in faces.iter().enumerate() {
        wanted.entry(f).or_default().push(k);
    }
    for b in display.bodies.iter().filter(|b| !b.hidden) {
        let mut table: Option<Arc<Vec<Vec<u32>>>> = None;
        for (fi, f) in b.face_refs.iter().enumerate() {
            let Some(ks) = wanted.remove(f) else { continue };
            let t = table.get_or_insert_with(|| face_edge_table(&b.mesh));
            let edges: Vec<Reference> = t.get(fi).map(|es| es.iter().filter_map(|&e| b.edge_refs.get(e as usize).cloned()).collect()).unwrap_or_default();
            for k in ks {
                out[k] = Some(edges.clone());
            }
        }
        if wanted.is_empty() {
            break;
        }
    }
    out
}

/// Per face of `mesh`, the edges bounding it ([`face_edges`]'s rule), worked out once per mesh.
/// Builds every shown body's face → edge table ahead, on the caller's (background) thread: a face's or feature's first
/// selection then finds it cached instead of building it on the UI thread (cf-surface-pattern: 16-21 ms on the first
/// click of each feature's row).
pub fn prewarm_face_edges(display: &StudioDisplay) {
    for b in display.bodies.iter().filter(|b| !b.hidden && !b.face_refs.is_empty()) {
        face_edge_table(&b.mesh);
    }
}

fn face_edge_table(mesh: &Arc<noshape_model::display::MeshBuffers>) -> Arc<Vec<Vec<u32>>> {
    type Table = (std::sync::Weak<noshape_model::display::MeshBuffers>, Arc<Vec<Vec<u32>>>);
    static CACHE: std::sync::Mutex<Vec<Table>> = std::sync::Mutex::new(Vec::new());
    if let Some(t) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(w, _)| std::ptr::eq(w.as_ptr(), Arc::as_ptr(mesh))).map(|(_, t)| t.clone()) {
        return t;
    }
    let m = &**mesh;
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
    let mut table = vec![Vec::new(); m.face_ranges.len()];
    for (ei, pl) in m.edges.iter().enumerate() {
        let Some(first) = pl.first() else { continue };
        let mut common = faces_at.get(&q(first)).cloned().unwrap_or_default();
        for p in &pl[1..] {
            if common.is_empty() {
                break;
            }
            let at = faces_at.get(&q(p)).map(Vec::as_slice).unwrap_or(&[]);
            common.retain(|f| at.contains(f));
        }
        for f in common {
            table[f as usize].push(ei as u32);
        }
    }
    let table = Arc::new(table);
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    c.retain(|(w, _)| w.strong_count() > 0);
    c.push((Arc::downgrade(mesh), table.clone()));
    table
}

fn lin_to_u8(c: [f32; 4]) -> [u8; 4] {
    let f = |x: f32| (if x <= 0.0031308 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 } * 255.0).round().clamp(0.0, 255.0) as u8;
    [f(c[0]), f(c[1]), f(c[2]), 255]
}

#[cfg(test)]
mod connector_zoom_tests {
    use super::{ConnectorGlyphs, ConnectorLook};
    use glam::{DVec2, DVec3};
    use noshape_render::camera_geometry::{CameraGeometry, View};

    /// Live 3d.json explicit_mate_connector: an assembly connector's pick is its whole 36 px disc (the reach 18 px
    /// plus the pick square's 5: 23 across), lying in the connector's plane.
    #[test]
    fn a_picked_glyphs_disc_takes_the_pointer_across_it() {
        let o = DVec3::new(0.0, 0.0, 0.01);
        let d = super::pick_disc(o, DVec3::X, DVec3::Y, DVec3::Z, 18.0, 7);
        assert!(d.pick_ids.iter().all(|p| *p == 7) && d.positions.len() == 49 && d.indices.len() == 48 * 3);
        let r = d.positions[1..].iter().map(|p| DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2])).distance(o));
        assert!(r.clone().all(|r| (r - 18.0).abs() < 1e-4), "all on the rim");
        assert!(d.positions.iter().all(|p| (f64::from(p[2]) - 0.01).abs() < 1e-6), "in the connector's plane");
    }

    /// Live 1701: a mate connector glyph reaches 18 px back along x / y, 33
    /// px out along them and 23 px up z, at the pixel size at its origin.
    #[test]
    fn connector_glyph_extent_counts_for_the_zoom_limits() {
        let o = DVec3::new(0.0, 0.0, 0.01);
        let glyphs = ConnectorGlyphs { items: vec![(o, DVec3::X, DVec3::Y, DVec3::Z, ConnectorLook::Rest)], picks: Vec::new() };
        let view = View { camera: Default::default(), size: DVec2::new(1200.0, 800.0) };
        let px = view.world_per_px(o).unwrap();
        let pts = glyphs.zoom_points(&view);
        let want = [o - DVec3::X * 18.0 * px, o - DVec3::Y * 18.0 * px, o + DVec3::X * 33.0 * px, o + DVec3::Y * 33.0 * px, o + DVec3::Z * 23.0 * px];
        assert_eq!(pts.len(), want.len());
        for (p, w) in pts.iter().zip(want) {
            assert!(p.distance(w) < 1e-12, "{p:?} vs {w:?}");
        }
    }
}

#[cfg(test)]
mod pick_priority_tests {
    use super::PickKind;

    /// Live's client pick priorities: mate connector 46, vertex 30, edge 20, face 5 (feel 3D, assembly).
    #[test]
    fn a_mate_connector_wins_over_a_vertex_an_edge_and_a_face() {
        let order = [PickKind::Connector, PickKind::Vertex, PickKind::Edge, PickKind::Face].map(PickKind::priority);
        assert!(order.windows(2).all(|w| w[0] > w[1]), "{order:?}");
    }
}

#[cfg(test)]
mod inference_mark_tests {
    use super::{InferenceMark, InferenceMarks};
    use glam::DVec3;

    /// The point under the pointer becomes the connector glyph, the others marks; a plain (origin, x, z) is a
    /// non-corner mark (MC-INFERENCE-DOTS: corners ~12 px, other points ~8 px).
    #[test]
    fn the_chosen_point_is_the_glyph_and_corners_keep_their_kind() {
        let p = |x: f64, corner| InferenceMark { origin: DVec3::new(x, 0.0, 0.0), x: DVec3::X, z: DVec3::Z, corner };
        let m = InferenceMarks::new([p(0.0, true), p(1.0, false), p(2.0, true)], Some(1));
        assert_eq!(m.glyph.items.len(), 1);
        assert_eq!(m.marks.iter().map(|k| (k.origin.x, k.corner)).collect::<Vec<_>>(), vec![(0.0, true), (2.0, true)]);
        let plain = InferenceMarks::new([(DVec3::ZERO, DVec3::X, DVec3::Z)], None);
        assert!(!plain.marks[0].corner);
    }
}

