//! Converts a [`StudioDisplay`] into a render [`Scene`] and a [`PickMap`]
//! translating pick ids back into persistent references.
//!
//! Geometry buffers are cached per `Arc<MeshBuffers>` and use local pick ids,
//! so a regeneration that keeps a body's tessellation re-uses its GPU data.
//! Hover and selection only rewrite the per-id state buffer, except for
//! invisible-but-pickable elements (vertices, sketch regions) whose highlight
//! overlays are rebuilt on demand.

use crate::theme;
use glam::DVec3;
use noshape_model::display::{BodyKind, SketchEntityState, StudioDisplay};
use noshape_model::reference::{FeatureId, Reference};
use noshape_render::renderer::Uploader;
use noshape_render::{Drawable, Instance, LineData, LineSegment, LinesHandle, MeshData, MeshHandle, PointData, PointSprite, PointsHandle, Scene, state};
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
    pub fn priority_fn(&self) -> Arc<dyn Fn(u32) -> i32 + Send + Sync> {
        let prio: Vec<i32> = self.targets.iter().map(|t| t.kind.priority()).collect();
        Arc::new(move |id| if id == 0 { -1 } else { prio.get(id as usize - 1).copied().unwrap_or(0) })
    }
}

struct CachedBody {
    mesh: MeshHandle,
    edges: LinesHandle,
    vertices: PointsHandle,
}

/// GPU caches that survive across rebuilds.
#[derive(Default)]
pub struct GpuCache {
    bodies: HashMap<usize, (std::sync::Weak<noshape_model::display::MeshBuffers>, Arc<CachedBody>)>,
    /// Colored body meshes keyed by (mesh, colors) pointers.
    colored: HashMap<(usize, usize), (std::sync::Weak<Vec<[u8; 4]>>, MeshHandle)>,
    /// Hidden-edge overlays (dashed, on top) keyed by mesh pointer.
    hidden_edges: HashMap<usize, (std::sync::Weak<noshape_model::display::MeshBuffers>, LinesHandle)>,
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
    pub display_mode: DisplayMode,
    /// Section view clipping planes (normal, offset along normal), up to
    /// three; geometry is cut away where all of them clip it.
    pub section: Vec<(glam::DVec3, f64)>,
    /// Per-vertex colors (sRGB) replacing a body's color: analysis maps
    /// and face appearances. Indexed like the body's mesh positions.
    pub vertex_colors: Option<Arc<HashMap<noshape_model::BodyId, Arc<Vec<[u8; 4]>>>>>,
    /// Draw edges hidden behind faces as dashed lines.
    pub hidden_edges_visible: bool,
}

/// Viewport render style (Onshape's display modes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DisplayMode {
    #[default]
    ShadedWithEdges,
    Shaded,
    HiddenLine,
    Wireframe,
    /// Onshape "Translucent": every part see-through.
    Translucent,
}

impl DisplayMode {
    pub const ALL: [DisplayMode; 5] = [DisplayMode::ShadedWithEdges, DisplayMode::Shaded, DisplayMode::HiddenLine, DisplayMode::Wireframe, DisplayMode::Translucent];
    pub fn label(self) -> &'static str {
        match self {
            DisplayMode::ShadedWithEdges => "Shaded with edges",
            DisplayMode::Shaded => "Shaded",
            DisplayMode::HiddenLine => "Hidden line",
            DisplayMode::Wireframe => "Wireframe",
            DisplayMode::Translucent => "Translucent",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            DisplayMode::ShadedWithEdges => "shaded-edges",
            DisplayMode::Shaded => "shaded",
            DisplayMode::HiddenLine => "hidden-lines",
            DisplayMode::Wireframe => "wireframe",
            DisplayMode::Translucent => "transparency",
        }
    }
}

pub struct BuiltScene {
    pub scene: Scene,
    pub picks: PickMap,
    /// Index range of static instances; overlays are appended after.
    pub static_instances: usize,
}

const EDGE_COLOR: u32 = theme::viewport::EDGE;

pub fn build(display: &StudioDisplay, opts: &SceneOptions, cache: &mut GpuCache, up: &Uploader) -> BuiltScene {
    let _t = noshape_render::perf::scope("s.build");
    let mut scene = Scene::default();
    let mut picks = PickMap::default();

    // ---------------------------------------------------------------- bodies
    cache.bodies.retain(|_, (w, _)| w.strong_count() > 0);
    cache.colored.retain(|_, (w, _)| w.strong_count() > 0);
    cache.hidden_edges.retain(|_, (w, _)| w.strong_count() > 0);
    for b in display.bodies.iter().filter(|b| !b.hidden) {
        let key = Arc::as_ptr(&b.mesh) as usize;
        let cached = match cache.bodies.get(&key) {
            Some((_, c)) => c.clone(),
            None => {
                let c = Arc::new(upload_body(&b.mesh, up));
                cache.bodies.insert(key, (Arc::downgrade(&b.mesh), c.clone()));
                c
            }
        };
        let face_base = picks.reserve(b.face_refs.iter().map(|r| PickTarget { reference: r.clone(), kind: PickKind::Face, anchor: None }));
        let edge_base = picks.reserve(b.edge_refs.iter().map(|r| PickTarget { reference: r.clone(), kind: PickKind::Edge, anchor: None }));
        let vert_base = picks.reserve(b.vertex_refs.iter().enumerate().map(|(i, r)| PickTarget {
            reference: r.clone(),
            kind: PickKind::Vertex,
            anchor: b.mesh.vertices.get(i).map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)),
        }));
        let mut color = b.color;
        if opts.transparent_bodies || b.kind == BodyKind::Surface {
            color[3] = color[3].min(if b.kind == BodyKind::Surface { 0.85 } else { 0.35 });
        }
        if b.kind != BodyKind::Wire {
            let colors = opts.vertex_colors.as_ref().and_then(|m| m.get(&b.id)).filter(|c| c.len() == b.mesh.positions.len());
            let mesh = match colors {
                Some(c) => {
                    let ckey = (key, Arc::as_ptr(c) as usize);
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
                DisplayMode::ShadedWithEdges | DisplayMode::Shaded => mesh.with_color(color),
                DisplayMode::Translucent => mesh.with_color([color[0], color[1], color[2], color[3].min(0.3)]),
                // Faces drawn in the background color hide what is behind them.
                DisplayMode::HiddenLine => mesh.with_color(theme::lin(0xffffff)),
                // Still pickable, but invisible.
                DisplayMode::Wireframe => mesh.with_color([0.0, 0.0, 0.0, 0.0]).pick_only(),
            };
            scene.instances.push(mesh);
        }
        let edge_color = match (b.kind, opts.display_mode) {
            (BodyKind::Wire, _) => theme::lin(theme::viewport::EDGE),
            (_, DisplayMode::HiddenLine | DisplayMode::Wireframe) => theme::lin(theme::viewport::EDGE_UNSHADED),
            _ => theme::lin(EDGE_COLOR),
        };
        let edges = Instance::new(Drawable::Lines(cached.edges.clone())).with_color(edge_color).pick_base(edge_base).layer(1);
        // Shaded mode hides edges visually but keeps them pickable.
        scene.instances.push(if opts.display_mode == DisplayMode::Shaded { edges.pick_only() } else { edges });
        scene.instances.push(Instance::new(Drawable::Points(cached.vertices.clone())).pick_base(vert_base).pick_only().layer(2));
        if opts.hidden_edges_visible && b.kind != BodyKind::Wire {
            let handle = match cache.hidden_edges.get(&key) {
                Some((_, h)) => h.clone(),
                None => {
                    let mut l = LineData::default();
                    for pl in &b.mesh.edges {
                        l.push_polyline(pl, [90, 90, 90, 140], 0, 1.0, LineSegment::DASHED | LineSegment::ON_TOP);
                    }
                    let h = up.upload_lines(&l);
                    cache.hidden_edges.insert(key, (Arc::downgrade(&b.mesh), h.clone()));
                    h
                }
            };
            scene.instances.push(Instance::new(Drawable::Lines(handle)).per_primitive_color().unpickable().layer(1));
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
            border.push_polyline(&loop_pts, [0, 0, 0, 0], id, 1.0, 0);
            // Onshape: CONSTRUCTION_PLANE_AMBIENT_COLOR at CONSTRUCTION_PLANE_OPACITY, outline at 35%.
            let fill = theme::lin(theme::viewport::PLANE_FILL);
            let outline = theme::lin(theme::viewport::PLANE_OUTLINE);
            scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&mesh))).with_color([fill[0], fill[1], fill[2], 0.2]).ignore_clip().layer(5));
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&border))).with_color([outline[0], outline[1], outline[2], 0.35]).ignore_clip().layer(5));
        }
    }

    // ---------------------------------------------------------------- origin
    if opts.show_origin && display.show_origin {
        let id = picks.push(PickTarget { reference: Reference::Origin, kind: PickKind::Origin, anchor: Some(DVec3::ZERO) });
        let pts = PointData { points: vec![PointSprite { position: [0.0; 3], color: theme::rgb8(theme::viewport::ORIGIN), pick_id: id, size: 7.0, style: 0, _pad: 0 }] };
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
                    None => sketch_color(c.state, c.construction, editing, c.external),
                };
                let style = if c.construction { LineSegment::DASHED } else { 0 };
                lines.push_polyline(&c.points, color, id, if editing { 2.0 } else { 1.5 }, style);
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
                    None => sketch_color(p.state, false, editing, false),
                };
                let size = if p.standalone { 7.0 } else if !editing { 0.0 } else if p.handle { 3.5 } else { 5.5 };
                points.points.push(PointSprite { position: p.position, color, pick_id: id, size, style: 0, _pad: 0 });
            }
            for d in &sk.dimensions {
                let color: [u8; 4] = theme::rgb8(if d.driven { theme::viewport::DIM_DRIVEN } else { theme::viewport::SKETCH_FULL });
                for seg in &d.lines {
                    lines.segments.push(LineSegment { a: seg[0], b: seg[1], color, pick_id: 0, width: 1.0, style: LineSegment::ON_TOP });
                }
            }
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().ignore_clip().layer(10));
            scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&points))).per_primitive_color().ignore_clip().layer(11));
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
                scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&mesh))).with_color([0.0, 0.0, 0.0, 0.0]).pick_only().ignore_clip().layer(9));
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
        for (axis, color) in [(mc.x, theme::rgb8(theme::viewport::AXIS_X)), (y, theme::rgb8(theme::viewport::AXIS_Y)), (mc.z, theme::rgb8(theme::viewport::AXIS_Z))] {
            let tip: [f32; 3] = (mc.origin + axis * len).as_vec3().into();
            l.segments.push(LineSegment { a: o, b: tip, color, pick_id: id, width: 2.0, style: LineSegment::ON_TOP });
        }
        scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&l))).per_primitive_color().ignore_clip().layer(12));
    }

    let mut planes = opts.section.iter().map(|(n, d)| noshape_render::ClipPlane { normal: n.as_vec3(), d: *d as f32 });
    scene.clip = planes.next();
    scene.clip_extra = planes.take(2).collect();
    let static_instances = scene.instances.len();
    scene.states = vec![0; picks.len() + 1];
    BuiltScene { scene, picks, static_instances }
}

fn sketch_color(st: SketchEntityState, construction: bool, editing: bool, external: bool) -> [u8; 4] {
    use theme::viewport as v;
    let hex = if !editing {
        v::INACTIVE
    } else if external {
        v::SKETCH_EXTERNAL
    } else {
        match st {
            SketchEntityState::Under => v::SKETCH_UNDER,
            SketchEntityState::Full => v::SKETCH_FULL,
            SketchEntityState::Over => v::SKETCH_OVER,
            SketchEntityState::Inactive => v::INACTIVE,
        }
    };
    let a = if construction { 200 } else { 255 };
    [((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8, a]
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
    CachedBody { mesh: up.upload_mesh_parts(&m.positions, &m.normals, &pick_ids, &m.indices), edges: up.upload_lines(&lines), vertices: up.upload_points(&points) }
}

/// Apply hover/selection state and rebuild highlight overlays for pick-only
/// elements. Returns a new scene sharing all static GPU buffers.
pub fn with_highlights(built: &BuiltScene, display: &StudioDisplay, selection: &[Reference], hover: Option<&Reference>, accent: &[Reference], extra: Vec<Instance>, up: &Uploader) -> Scene {
    let _t = noshape_render::perf::scope("s.with_highlights");
    let mut scene = Scene { instances: built.scene.instances[..built.static_instances].to_vec(), states: vec![0; built.picks.len() + 1], clip: built.scene.clip, clip_extra: built.scene.clip_extra.clone() };
    let mut overlay_pts = PointData::default();
    let mut overlay_regions: Vec<(MeshData, [f32; 4])> = Vec::new();
    let sel_color = theme::lin(theme::viewport::SELECTION);
    let hov_color = theme::lin(theme::viewport::PREHIGHLIGHT);

    let mut mark = |r: &Reference, flag: u32, scene: &mut Scene| {
        for &id in built.picks.ids_for(r) {
            scene.add_flag(id, flag);
            let Some(t) = built.picks.get(id) else { continue };
            match t.kind {
                PickKind::Vertex => {
                    if let Some(a) = t.anchor {
                        let c = if flag == state::HOVER { hov_color } else { sel_color };
                        overlay_pts.points.push(PointSprite { position: a.as_vec3().into(), color: lin_to_u8(c), pick_id: 0, size: 8.0, style: LineSegment::ON_TOP, _pad: 0 });
                    }
                }
                PickKind::SketchRegion => {
                    if let Reference::SketchRegion { sketch, key } = r
                        && let Some(sk) = display.sketches.iter().find(|s| s.feature == *sketch)
                        && let Some(reg) = sk.regions.iter().find(|g| &g.key == key)
                    {
                        let n: [f32; 3] = sk.frame.normal.as_vec3().into();
                        let mut c = if flag == state::HOVER { hov_color } else { sel_color };
                        c[3] = if flag == state::HOVER { 0.25 } else { 0.4 };
                        overlay_regions.push((MeshData { positions: reg.positions.clone(), normals: vec![n; reg.positions.len()], pick_ids: vec![0; reg.positions.len()], indices: reg.indices.clone() }, c));
                    }
                }
                _ => {}
            }
        }
    };
    for r in accent {
        mark(r, state::ACCENT, &mut scene);
    }
    for r in selection {
        mark(r, state::SELECTED, &mut scene);
    }
    if let Some(h) = hover {
        mark(h, state::HOVER, &mut scene);
    }
    if !overlay_pts.points.is_empty() {
        scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&overlay_pts))).per_primitive_color().unpickable().ignore_clip().layer(20));
    }
    for (m, c) in overlay_regions {
        scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&m))).with_color(c).unpickable().ignore_clip().layer(8));
    }
    scene.instances.extend(extra);
    scene
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

fn lin_to_u8(c: [f32; 4]) -> [u8; 4] {
    let f = |x: f32| (if x <= 0.0031308 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 } * 255.0).round().clamp(0.0, 255.0) as u8;
    [f(c[0]), f(c[1]), f(c[2]), 255]
}
