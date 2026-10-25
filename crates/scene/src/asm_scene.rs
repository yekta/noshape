//! Render scene for an evaluated assembly: every part occurrence is drawn
//! from GPU buffers cached per `Arc<MeshBuffers>` with its own instance
//! transform, so moving instances (drag, animation, explode) never
//! re-uploads geometry. Pick ids map back to (instance path, reference).

use crate::builder::{PickKind, MESH_LINE_COLOR};
use crate::palette as theme;
use glam::{DMat4, DVec3, Mat4};
use noshape_model::assembly::eval::{AssemblyResult, InstancePath};
use noshape_model::assembly::solver::Frame;
use noshape_model::display::{BodyKind, MeshBuffers};
use noshape_model::reference::Reference;
use noshape_render::renderer::Uploader;
use noshape_render::{Drawable, Instance, LineData, LineSegment, LinesHandle, MeshData, MeshHandle, PointData, PointSprite, PointsHandle, Scene, state};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

#[derive(Clone, Debug)]
pub struct AsmPickTarget {
    pub path: InstancePath,
    pub reference: Reference,
    pub kind: PickKind,
    /// Index into `AssemblyResult::occurrences`.
    pub occ: usize,
    /// Vertex position in body coordinates.
    pub anchor: Option<DVec3>,
}

#[derive(Clone, Default)]
pub struct AsmPickMap {
    targets: Vec<AsmPickTarget>,
    /// All face pick ids per top-level instance (whole-instance highlight).
    by_top: HashMap<noshape_model::assembly::InstanceId, Vec<u32>>,
    by_ref: HashMap<(InstancePath, Reference), Vec<u32>>,
}

impl AsmPickMap {
    fn push(&mut self, t: AsmPickTarget) -> u32 {
        let id = self.targets.len() as u32 + 1;
        if t.kind == PickKind::Face {
            self.by_top.entry(t.path[0]).or_default().push(id);
        }
        self.by_ref.entry((t.path.clone(), t.reference.clone())).or_default().push(id);
        self.targets.push(t);
        id
    }
    pub fn get(&self, id: u32) -> Option<&AsmPickTarget> {
        if id == 0 { None } else { self.targets.get(id as usize - 1) }
    }
    pub fn len(&self) -> usize {
        self.targets.len()
    }
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
    pub fn faces_of(&self, top: noshape_model::assembly::InstanceId) -> &[u32] {
        self.by_top.get(&top).map(|v| v.as_slice()).unwrap_or(&[])
    }
    pub fn ids_for(&self, path: &InstancePath, r: &Reference) -> &[u32] {
        self.by_ref.get(&(path.clone(), r.clone())).map(|v| v.as_slice()).unwrap_or(&[])
    }
    /// Pick ids of an instance's edges.
    pub fn edges_of(&self, top: noshape_model::assembly::InstanceId) -> Vec<u32> {
        self.by_ref.iter().filter(|((p, r), _)| p.first() == Some(&top) && matches!(r, Reference::Edge(_))).flat_map(|(_, v)| v.iter().copied()).collect()
    }
    /// Pick ids of the edges bounding a face (edge names hold their faces').
    pub fn boundary_ids(&self, path: &InstancePath, face: &Reference) -> Vec<u32> {
        let Reference::Face(n) = face else { return Vec::new() };
        self.by_ref
            .iter()
            .filter(|((p, r), _)| p == path && matches!(r, Reference::Edge(noshape_model::reference::TopoName::Edge { faces, .. }) if faces.iter().any(|f| **f == *n)))
            .flat_map(|(_, v)| v.iter().copied())
            .collect()
    }
    pub fn priority_fn(&self) -> Arc<dyn Fn(u32) -> i32 + Send + Sync> {
        let prio: Vec<i32> = self.targets.iter().map(|t| t.kind.priority()).collect();
        Arc::new(move |id| if id == 0 { -1 } else { prio.get(id as usize - 1).copied().unwrap_or(100) })
    }
}

struct CachedBody {
    mesh: MeshHandle,
    edges: LinesHandle,
    vertices: PointsHandle,
    /// Mesh parts' triangle sides (live G35), when the part has mesh faces.
    mesh_lines: Option<LinesHandle>,
    /// The edges at each coarser line level ([`crate::builder::edge_lods`]).
    edge_lods: Vec<LinesHandle>,
}

#[derive(Default)]
pub struct AsmGpuCache {
    bodies: HashMap<usize, (Weak<MeshBuffers>, Arc<CachedBody>)>,
    /// Per mesh: its sharp and tangent (dash-dot) edges, for Tangent edges
    /// phantom / removed (camera menu).
    tangent: HashMap<usize, (Weak<MeshBuffers>, (LinesHandle, LinesHandle))>,
    /// Per mesh: its edges dashed on top, for Hidden edges visible.
    hidden_edges: HashMap<usize, (Weak<MeshBuffers>, LinesHandle)>,
    /// The unit cube of the occurrences' lowest level of detail
    /// ([`Instance::lod_box`]).
    lod_box: Option<MeshHandle>,
}

pub struct AsmBuilt {
    pub scene: Scene,
    pub picks: AsmPickMap,
    pub static_instances: usize,
}

#[derive(Clone, Default)]
pub struct AsmSceneOptions {
    /// Instances drawn transparent (isolate mode shows the rest dimmed).
    pub isolate: Option<HashSet<noshape_model::assembly::InstanceId>>,
    /// Instances shown with the interference / error tint.
    pub error_tops: HashSet<noshape_model::assembly::InstanceId>,
    /// Instances drawn transparent and not pickable (being dragged in snap mode).
    pub ghost_tops: HashSet<noshape_model::assembly::InstanceId>,
    /// Section view planes (normal, d): geometry with dot(normal, p) > d is cut away.
    pub section: Vec<(DVec3, f64)>,
    /// Instances the section planes don't cut (Section view › Items to exclude).
    pub section_skip: HashSet<noshape_model::assembly::InstanceId>,
    /// The section plane being edited (drawn selected, `Scene::clip_selected`).
    pub section_selected: Option<usize>,
    /// Opacity of transparent instances (Isolate / Make transparent slider).
    pub dim_alpha: Option<f32>,
    /// Dimmed (transparent) instances stay pickable (Isolate › Select
    /// transparent geometry, Alt held).
    pub pick_dimmed: bool,
    /// Zebra stripes on the shaded parts, and whether edges are hidden.
    pub zebra: Option<noshape_render::Zebra>,
    /// Reflection analysis environment on the shaded parts.
    pub reflection: Option<noshape_render::Reflection>,
    pub hide_edges: bool,
    /// Camera menu render choices, as in the Part Studio (`SceneOptions`).
    pub display_mode: crate::builder::DisplayMode,
    pub hidden_edges_visible: bool,
    pub tangent_edges: crate::builder::TangentEdges,
    /// Line width and phantom edges' color and width.
    pub graphics: crate::graphics_prefs::GraphicsPrefs,
}

fn to_mat4(m: &DMat4) -> Mat4 {
    m.as_mat4()
}

pub fn build(result: &AssemblyResult, opts: &AsmSceneOptions, cache: &mut AsmGpuCache, up: &Uploader) -> AsmBuilt {
    let mut scene = Scene::default();
    let mut picks = AsmPickMap::default();
    cache.bodies.retain(|_, (w, _)| w.strong_count() > 0);
    cache.tangent.retain(|_, (w, _)| w.strong_count() > 0);
    cache.hidden_edges.retain(|_, (w, _)| w.strong_count() > 0);
    use crate::builder::{DisplayMode, TangentEdges};
    let mode = opts.display_mode;
    let mut bodies = 0u32;
    for (oi, o) in result.occurrences.iter().enumerate() {
        if o.hidden {
            continue;
        }
        let mk = |r: &Reference, kind, anchor| AsmPickTarget { path: o.path.clone(), reference: r.clone(), kind, occ: oi, anchor };
        let face_base = picks.targets.len() as u32;
        for r in &o.face_refs {
            picks.push(mk(r, PickKind::Face, None));
        }
        let edge_base = picks.targets.len() as u32;
        for r in &o.edge_refs {
            picks.push(mk(r, PickKind::Edge, None));
        }
        let vert_base = picks.targets.len() as u32;
        for (i, r) in o.vertex_refs.iter().enumerate() {
            let a = o.mesh.vertices.get(i).map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
            picks.push(mk(r, PickKind::Vertex, a));
        }
        let t = to_mat4(&o.transform);
        let first_instance = scene.instances.len();
        let mut color = o.color;
        let dimmed = opts.isolate.as_ref().is_some_and(|s| !s.contains(&o.top())) || opts.ghost_tops.contains(&o.top());
        if opts.ghost_tops.contains(&o.top()) {
            color[3] = opts.dim_alpha.unwrap_or(0.35);
        } else if dimmed {
            color[3] = opts.dim_alpha.unwrap_or(0.12);
        } else if o.kind == BodyKind::Surface {
            color[3] = 0.85;
        }
        if opts.error_tops.contains(&o.top()) {
            let e = theme::lin(theme::viewport::ERROR);
            color = [color[0] * 0.4 + e[0] * 0.6, color[1] * 0.4 + e[1] * 0.6, color[2] * 0.4 + e[2] * 0.6, color[3]];
        }
        // Sketch instances (wire bodies) are drawn as their translucent
        // region fill under thin gray curves (live assembly/123).
        let sketch = o.kind == BodyKind::Wire;
        // Onshape's display levels, coarsest first (`PartOccurrence::coarser`):
        // each level's instances draw only when the renderer picks that level
        // for the occurrence (`Instance::mesh_lod`).
        let levels: Vec<&Arc<MeshBuffers>> = o.coarser.iter().chain(std::iter::once(&o.mesh)).collect();
        let edge_color = if sketch { theme::lin(0x999999) } else { theme::lin(theme::viewport::EDGE) };
        let mut finest_cached = None;
        for (li, &mesh) in levels.iter().enumerate() {
            let is_finest = li + 1 == levels.len();
            let level_first = scene.instances.len();
            let key = Arc::as_ptr(mesh) as usize;
            let cached = match cache.bodies.get(&key) {
                Some((_, c)) => c.clone(),
                None => {
                    let c = Arc::new(upload_body(mesh, up));
                    cache.bodies.insert(key, (Arc::downgrade(mesh), c.clone()));
                    c
                }
            };
            if !sketch || !mesh.indices.is_empty() {
                let mut inst = Instance::new(Drawable::Mesh(cached.mesh.clone())).with_color(color).with_transform(t).pick_base(face_base).zebra(opts.zebra.filter(|_| !dimmed && !sketch)).reflection(opts.reflection.clone().filter(|_| !dimmed && !sketch));
                // Sketch regions are pickable faces (live 601).
                if dimmed && !opts.pick_dimmed {
                    inst = inst.unpickable();
                }
                // The display modes as in the Part Studio (scene_builder): see-through
                // at 0.48, or unlit faces in the background color (Unshaded).
                if !sketch {
                    inst = match mode {
                        DisplayMode::Translucent => inst.with_color([color[0], color[1], color[2], color[3].min(0.48)]),
                        DisplayMode::HiddenLine => inst.with_color(theme::lin(0xffffff)).unlit(),
                        DisplayMode::ShadedWithEdges | DisplayMode::Shaded => inst,
                    };
                }
                scene.instances.push(inst);
            }
            if !dimmed && (!opts.hide_edges || sketch) {
                let edge_color = if !sketch && mode == DisplayMode::HiddenLine { theme::lin(theme::viewport::EDGE_UNSHADED) } else { edge_color };
                let line_w = opts.graphics.line_width;
                let edges = Instance::new(Drawable::Lines(cached.edges.clone())).with_color(edge_color).with_transform(t).pick_base(edge_base).line_scale(line_w).layer(1);
                if !sketch && mode == DisplayMode::Shaded {
                    // Shaded without edges: hidden but still pickable.
                    scene.instances.push(edges.pick_only());
                } else if !sketch && opts.tangent_edges != TangentEdges::Visible {
                    // Tangent edges phantom / removed: the sharp edges (and the
                    // tangent ones dash-dot); all of them stay pickable.
                    let (sharp, phantom) = match cache.tangent.get(&key) {
                        Some((_, h)) => h.clone(),
                        None => {
                            let smooth = mesh.smooth_edges();
                            let (mut sharp, mut phantom) = (LineData::default(), LineData::default());
                            for (ei, pl) in mesh.edges.iter().enumerate() {
                                if smooth.get(ei).copied().unwrap_or(false) {
                                    phantom.push_polyline(pl, [0, 0, 0, 255], 0, 1.0, LineSegment::DASH_DOT);
                                } else {
                                    sharp.push_polyline(pl, [0, 0, 0, 255], ei as u32 + 1, 1.25, 0);
                                }
                            }
                            let h = (up.upload_lines(&sharp), up.upload_lines(&phantom));
                            cache.tangent.insert(key, (Arc::downgrade(mesh), h.clone()));
                            h
                        }
                    };
                    scene.instances.push(edges.pick_only());
                    scene.instances.push(Instance::new(Drawable::Lines(sharp)).with_color(edge_color).with_transform(t).pick_base(edge_base).line_scale(line_w).layer(1));
                    if opts.tangent_edges == TangentEdges::Phantom {
                        let c = theme::lin(opts.graphics.phantom_color);
                        scene.instances.push(Instance::new(Drawable::Lines(phantom)).with_color(c).with_transform(t).unpickable().line_scale(opts.graphics.phantom_width / 0.5).layer(1));
                    }
                } else {
                    // Each coarser line level, then the full edges (also the pick).
                    for (k, l) in cached.edge_lods.iter().enumerate().filter(|_| is_finest) {
                        let mut e = edges.clone().line_lod(k as u8 + 2);
                        e.drawable = Drawable::Lines(l.clone());
                        scene.instances.push(e);
                    }
                    scene.instances.push(edges.line_lod(1));
                }
                // Hidden edges visible: every edge dashed on top (per display level).
                if !sketch && opts.hidden_edges_visible {
                    let h = match cache.hidden_edges.get(&key) {
                        Some((_, h)) => h.clone(),
                        None => {
                            let mut l = LineData::default();
                            for pl in &mesh.edges {
                                l.push_polyline(pl, [90, 90, 90, 140], 0, 1.0, LineSegment::DASHED | LineSegment::ON_TOP);
                            }
                            let h = up.upload_lines(&l);
                            cache.hidden_edges.insert(key, (Arc::downgrade(mesh), h.clone()));
                            h
                        }
                    };
                    scene.instances.push(Instance::new(Drawable::Lines(h)).per_primitive_color().with_transform(t).unpickable().layer(1));
                }
                // A mesh part's triangle lines, as in the Part Studio (not in plain Shaded).
                if let Some(lines) = cached.mesh_lines.clone().filter(|_| mode != DisplayMode::Shaded) {
                    scene.instances.push(Instance::new(Drawable::Lines(lines)).with_color(theme::lin(MESH_LINE_COLOR)).with_transform(t).unpickable().layer(1));
                }
            }
            if levels.len() > 1 {
                for i in &mut scene.instances[level_first..] {
                    i.mesh_lod = o.levels_from + li as u8 + 1;
                }
            }
            if is_finest {
                finest_cached = Some(cached);
            }
        }
        let cached = finest_cached.expect("an occurrence has its finest level");
        if !dimmed {
            // B-rep vertices are the same at every level.
            scene.instances.push(Instance::new(Drawable::Points(cached.vertices.clone())).with_transform(t).pick_base(vert_base).pick_only().layer(2));
            if sketch {
                // The sketch's points as small gray dots.
                let dots = PointData { points: o.mesh.vertices.iter().map(|p| PointSprite { position: *p, color: [0x99, 0x99, 0x99, 255], pick_id: 0, size: 4.0, style: 0, _pad: 0 }).collect() };
                scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&dots))).per_primitive_color().with_transform(t).unpickable().layer(2));
            }
        }
        if opts.section_skip.contains(&o.top()) {
            for i in &mut scene.instances[first_instance..] {
                i.ignore_clip = true;
            }
        }
        // Onshape's lowest level of detail of the body occurrence: its
        // bounding box.
        bodies += 1;
        for i in &mut scene.instances[first_instance..] {
            i.body = bodies;
        }
        if let Some(bounds) = cached.mesh.bounds().filter(|_| !sketch) {
            let unit = cache.lod_box.get_or_insert_with(|| up.upload_mesh(&noshape_render::scene::lod_box_mesh())).clone();
            let mut lod = Instance::lod_box(unit, bodies, bounds, color);
            lod.transform = t * lod.transform;
            scene.instances.push(lod);
        }
    }
    let static_instances = scene.instances.len();
    scene.states = vec![0; picks.len() + 1];
    let clip = |(n, d): &(DVec3, f64)| noshape_render::ClipPlane::new(n.as_vec3(), *d as f32);
    scene.clip = opts.section.first().map(clip);
    scene.clip_extra = opts.section.iter().skip(1).map(clip).collect();
    if let Some(slot) = opts.section_selected.and_then(|i| scene.clip_selected.get_mut(i)) {
        *slot = true;
    }
    AsmBuilt { scene, picks, static_instances }
}

fn upload_body(m: &MeshBuffers, up: &Uploader) -> CachedBody {
    let mut pick_ids = vec![0u32; m.positions.len()];
    for (fi, &(start, count)) in m.face_ranges.iter().enumerate() {
        for &ix in &m.indices[start as usize..(start + count) as usize] {
            if let Some(p) = pick_ids.get_mut(ix as usize) {
                *p = fi as u32 + 1;
            }
        }
    }
    let mesh = MeshData { positions: m.positions.clone(), normals: m.normals.clone(), pick_ids, indices: m.indices.clone() };
    let mut lines = LineData::default();
    for (ei, pl) in m.edges.iter().enumerate() {
        lines.push_polyline(pl, [0, 0, 0, 255], ei as u32 + 1, 1.25, 0);
    }
    let points = PointData {
        points: m.vertices.iter().enumerate().map(|(i, p)| PointSprite { position: *p, color: [255; 4], pick_id: i as u32 + 1, size: 6.0, style: 0, _pad: 0 }).collect(),
    };
    let mesh_lines = (!m.mesh_lines.is_empty()).then(|| {
        let mut l = LineData::default();
        for s in m.mesh_lines.as_chunks::<2>().0 {
            l.push_polyline(s, [0, 0, 0, 255], 0, 1.0, 0);
        }
        up.upload_lines(&l)
    });
    CachedBody { mesh: up.upload_mesh(&mesh), edges: up.upload_lines(&lines), vertices: up.upload_points(&points), mesh_lines, edge_lods: crate::builder::edge_lods(m, up) }
}

/// A coordinate triad overlay (world frame).
#[derive(Clone, Copy, Debug)]
pub struct Triad {
    pub frame: Frame,
    pub size: f64,
    /// Dim (secondary) triads use thinner lines.
    pub strong: bool,
}

/// Highlight state for one frame.
#[derive(Clone, Default)]
pub struct Highlights {
    pub selected_tops: Vec<noshape_model::assembly::InstanceId>,
    pub accent_tops: Vec<noshape_model::assembly::InstanceId>,
    pub hover_entity: Option<(InstancePath, Reference)>,
    pub selected_entities: Vec<(InstancePath, Reference)>,
    pub triads: Vec<Triad>,
    /// Inference dots (world positions).
    pub dots: Vec<DVec3>,
    /// Inference points of the hovered entity (live: small white dots).
    pub light_dots: Vec<DVec3>,
    /// The face whose boundary is outlined (live: hovering a face while
    /// picking a mate connector outlines it, no fill).
    pub hover_outline: Option<(InstancePath, Reference)>,
    /// The inferred mate connector under the pointer: live's glyph, a khaki
    /// disc with a notch and a small triad (frame, disc radius).
    pub inference_glyph: Option<(Frame, f64)>,
    /// Move manipulator (triad with pickable axis arrows and rotation rings)
    /// and the hovered handle.
    pub manipulator: Option<(Frame, f64, Option<ManipHandle>)>,
    /// Explode mode (live assembly/802): the triad has no plane handles.
    pub explode_triad: bool,
    /// An explode step being made or edited (live 805 / 810): instead of the
    /// triad, one arrow along the move (origin, direction, size), or for a
    /// rotation a ring with an arrowhead around the axis.
    pub step_arrow: Option<(DVec3, DVec3, f64, bool)>,
    /// The Section view dialog's manipulator (live assembly/2368).
    pub section: Option<SectionGlyph>,
    /// Pickable assembly origin triad (size) and whether it is hovered/selected.
    pub origin: Option<(f64, bool)>,
    /// Pickable assembly mate connector triads: (frame, size, highlighted).
    /// Assembly mate connectors: frame, size, highlighted, opacity (the
    /// connector dialog's preview slider fades the edited one).
    pub connectors: Vec<(Frame, f64, bool, f32)>,
    /// Mate glyph lines (between connector origins).
    pub mate_lines: Vec<(DVec3, DVec3, bool)>,
    /// Pickable mate indicators: (connector frame, half size, highlighted,
    /// state), drawn flat in the connector's XY plane like Onshape's.
    pub mate_glyphs: Vec<(Frame, f64, bool, MateGlyphState)>,
    /// Placement preview cross at the cursor (insert).
    pub cursor_mark: Option<(DVec3, f64)>,
    /// Explode lines (dashed), start to exploded position.
    pub trails: Vec<(DVec3, DVec3)>,
    /// Selected mate's rotation value (live 305z, 575).
    pub value_arcs: Vec<ValueArc>,
    /// Selected mate's translation values.
    pub value_lines: Vec<ValueLine>,
    /// Interference detection: overlapping material (world), drawn red over
    /// the see-through parts, and the hovered overlap's bounding box.
    pub red_meshes: Vec<MeshHandle>,
    pub boxes: Vec<(DVec3, DVec3)>,
}

/// A mate's translation value as a linear dimension: a dashed blue line from
/// `a` to `b` with an arrowhead at each end pointing out (heads `head` long,
/// in the plane of `up`), broken around the value text over `gap` (start and
/// end as fractions of the line).
#[derive(Clone, Copy, Debug)]
pub struct ValueLine {
    pub a: DVec3,
    pub b: DVec3,
    pub up: DVec3,
    pub gap: Option<(f64, f64)>,
    pub head: f64,
}

/// A mate's rotation value as Onshape's angular dimension (live 305z): a
/// blue dashed arc in the first connector's XY plane from its X axis
/// counter-clockwise through `sweep`, an arrowhead at each end pointing
/// out of the arc, broken around the value text at `gap` (centre angle,
/// half width; radians).
#[derive(Clone, Copy, Debug)]
pub struct ValueArc {
    pub frame: Frame,
    pub radius: f64,
    pub sweep: f64,
    pub gap: Option<(f64, f64)>,
}

/// Manipulator handle: translate along axis k, rotate about axis k,
/// translate in the plane normal to axis k, or the center ball (moves the
/// manipulator itself).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManipHandle {
    Axis(usize),
    Ring(usize),
    Plane(usize),
    Center,
}

/// The section manipulator's handles (live assembly/2368): the base circle
/// (moves the manipulator over the plane), the arrow (offset) and the two
/// rings at its tip (turn the plane about its Y, ring 0, and X, ring 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionHandle {
    Base,
    Arrow,
    Ring(usize),
}

/// The section manipulator as drawn: its base on the plane, the arrow's
/// direction, the plane's X / Y, a pixel's world size, the view direction,
/// the hovered and dragged handles and the drag's start mark (an arrow
/// drag's start base: "⊥"; a ring drag's start direction: "⊢").
#[derive(Clone, Copy, Debug)]
pub struct SectionGlyph {
    pub base: DVec3,
    pub dir: DVec3,
    pub x: DVec3,
    pub y: DVec3,
    pub wpp: f64,
    pub view: DVec3,
    pub hovered: Option<SectionHandle>,
    pub dragging: Option<SectionHandle>,
    pub start: Option<(DVec3, bool)>,
}

/// The glyph in pixels (live assembly/2368–2370): base circle radius, the
/// head's base and the tip along the arrow, the rings' centre.
pub const SECTION_BASE_R_PX: f64 = 8.0;
pub const SECTION_HEAD_PX: f64 = 60.0;
pub const SECTION_TIP_PX: f64 = 85.0;
pub const SECTION_RINGS_PX: f64 = 97.0;

/// Pickable overlay items beyond the geometry picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayPick {
    Manip(ManipHandle),
    Origin,
    Connector(usize),
    /// Mate indicator (index into the view's displayed mates).
    Mate(usize),
    /// A Section view manipulator handle.
    Section(SectionHandle),
}

/// Mate indicator state (color).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MateGlyphState {
    Ok,
    Error,
    Inactive,
}

/// Overlay pick ids of mate indicators start here (after any number of connectors).
const MATE_PICK_BASE: u32 = 1 << 20;

fn overlay_base(built: &AsmBuilt) -> u32 {
    built.picks.len() as u32 + 1
}

/// Pick id of an overlay item.
pub fn overlay_id(built: &AsmBuilt, o: OverlayPick) -> u32 {
    let b = overlay_base(built);
    match o {
        OverlayPick::Manip(ManipHandle::Axis(k)) => b + k as u32,
        OverlayPick::Manip(ManipHandle::Ring(k)) => b + 3 + k as u32,
        OverlayPick::Origin => b + 6,
        OverlayPick::Manip(ManipHandle::Plane(k)) => b + 7 + k as u32,
        OverlayPick::Manip(ManipHandle::Center) => b + 10,
        OverlayPick::Connector(i) => b + 11 + i as u32,
        OverlayPick::Mate(i) => b + MATE_PICK_BASE + i as u32,
        OverlayPick::Section(SectionHandle::Arrow) => b + MATE_PICK_BASE - 1,
        OverlayPick::Section(SectionHandle::Base) => b + MATE_PICK_BASE - 2,
        OverlayPick::Section(SectionHandle::Ring(k)) => b + MATE_PICK_BASE - 3 - k as u32,
    }
}

/// Overlay item for a pick id, if it is one.
pub fn overlay_pick(built: &AsmBuilt, id: u32) -> Option<OverlayPick> {
    let b = overlay_base(built);
    if id < b {
        return None;
    }
    Some(match id - b {
        k @ 0..=2 => OverlayPick::Manip(ManipHandle::Axis(k as usize)),
        k @ 3..=5 => OverlayPick::Manip(ManipHandle::Ring(k as usize - 3)),
        6 => OverlayPick::Origin,
        k @ 7..=9 => OverlayPick::Manip(ManipHandle::Plane(k as usize - 7)),
        10 => OverlayPick::Manip(ManipHandle::Center),
        k if k >= MATE_PICK_BASE => OverlayPick::Mate((k - MATE_PICK_BASE) as usize),
        k if k == MATE_PICK_BASE - 1 => OverlayPick::Section(SectionHandle::Arrow),
        k if k == MATE_PICK_BASE - 2 => OverlayPick::Section(SectionHandle::Base),
        k if k == MATE_PICK_BASE - 3 || k == MATE_PICK_BASE - 4 => OverlayPick::Section(SectionHandle::Ring((MATE_PICK_BASE - 3 - k) as usize)),
        k => OverlayPick::Connector(k as usize - 11),
    })
}

fn axis_colors() -> [[u8; 4]; 3] {
    [
        theme::rgb8(theme::pick(theme::viewport::AXIS_X, theme::viewport_dark::AXIS_X)),
        theme::rgb8(theme::pick(theme::viewport::AXIS_Y, theme::viewport_dark::AXIS_Y)),
        theme::rgb8(theme::pick(theme::viewport::AXIS_Z, theme::viewport_dark::AXIS_Z)),
    ]
}
/// Hovered manipulator handles (Onshape UI_HIGHLIGHT_COLOR).
const HANDLE_HL: [u8; 4] = [0xff, 0xc2, 0x0e, 255];
/// Dark points and lines on the white viewport.
const DARK: [u8; 4] = [0x33, 0x33, 0x33, 255];

fn push_triad(lines: &mut LineData, f: &Frame, size: f64, width: f32, pick: u32, highlight: bool) {
    let o: [f32; 3] = f.origin.as_vec3().into();
    for (k, axis) in [f.x(), f.y(), f.z()].into_iter().enumerate() {
        let tip: [f32; 3] = (f.origin + axis * size).as_vec3().into();
        let c = if highlight { HANDLE_HL } else { axis_colors()[k] };
        lines.segments.push(LineSegment { a: o, b: tip, color: c, pick_id: pick, width, style: LineSegment::ON_TOP });
    }
}

pub fn with_highlights(built: &AsmBuilt, result: &AssemblyResult, h: &Highlights, up: &Uploader) -> Scene {
    let mut scene = Scene { instances: built.scene.instances[..built.static_instances].to_vec(), states: vec![0; built.picks.len() + 1], clip: built.scene.clip, clip_extra: built.scene.clip_extra.clone(), clip_selected: built.scene.clip_selected, compare: built.scene.compare };
    for top in &h.accent_tops {
        for &id in built.picks.faces_of(*top) {
            scene.add_flag(id, state::ACCENT);
        }
    }
    // A selected (or just inserted) instance: Onshape's whole-part amber,
    // lighter than a selected face, outlined in light amber; its edges go
    // dark amber (live ins-902, qa5 A15).
    for top in &h.selected_tops {
        for &id in built.picks.faces_of(*top) {
            scene.add_flag(id, state::SELECTED | state::BODY | state::OUTLINE);
        }
        for id in built.picks.edges_of(*top) {
            scene.add_flag(id, state::BODY);
        }
    }
    let mut pts = PointData::default();
    let sel = theme::lin(theme::viewport::SELECTION);
    let hov = theme::lin(theme::viewport::PREHIGHLIGHT);
    let mut mark = |path: &InstancePath, r: &Reference, flag: u32, scene: &mut Scene| {
        for &id in built.picks.ids_for(path, r) {
            scene.add_flag(id, flag);
            if let Some(t) = built.picks.get(id)
                && t.kind == PickKind::Vertex
                && let (Some(a), Some(o)) = (t.anchor, result.occurrences.get(t.occ))
            {
                let w = o.transform.transform_point3(a);
                let c = if flag == state::HOVER { hov } else { sel };
                pts.points.push(PointSprite { position: w.as_vec3().into(), color: lin_to_u8(c), pick_id: 0, size: 8.0, style: LineSegment::ON_TOP, _pad: 0 });
            }
        }
    };
    for (p, r) in &h.selected_entities {
        mark(p, r, state::SELECTED, &mut scene);
    }
    if let Some((p, r)) = &h.hover_entity {
        mark(p, r, state::HOVER, &mut scene);
    }
    if let Some((p, r)) = &h.hover_outline {
        for id in built.picks.boundary_ids(p, r) {
            scene.add_flag(id, state::HOVER);
        }
    }
    for d in &h.dots {
        pts.points.push(PointSprite { position: d.as_vec3().into(), color: DARK, pick_id: 0, size: 6.0, style: LineSegment::ON_TOP, _pad: 0 });
    }
    for d in &h.light_dots {
        // Live (qa5/asm-041): white with a soft grey rim, hidden behind
        // other parts (depth tested; qa5 A5: points of the hovered face
        // under a shaft / bracket showed on those parts).
        pts.points.push(PointSprite { position: d.as_vec3().into(), color: [0x9f, 0x9f, 0x9f, 255], pick_id: 0, size: 7.0, style: 0, _pad: 0 });
        pts.points.push(PointSprite { position: d.as_vec3().into(), color: [0xfd, 0xfd, 0xfd, 255], pick_id: 0, size: 5.0, style: 0, _pad: 0 });
    }
    let mut lines = LineData::default();
    if let Some((f, r)) = &h.inference_glyph {
        push_connector_glyph(&mut lines, f, *r);
    }
    for t in &h.triads {
        let w = if t.strong { 2.5 } else { 1.5 };
        push_triad(&mut lines, &t.frame, t.size, w, 0, false);
        pts.points.push(PointSprite { position: t.frame.origin.as_vec3().into(), color: DARK, pick_id: 0, size: if t.strong { 7.0 } else { 5.0 }, style: LineSegment::ON_TOP, _pad: 0 });
    }
    for (a, b, strong) in &h.mate_lines {
        lines.segments.push(LineSegment { a: a.as_vec3().into(), b: b.as_vec3().into(), color: theme::rgb8(theme::viewport::SELECTION), pick_id: 0, width: if *strong { 2.0 } else { 1.2 }, style: LineSegment::ON_TOP });
    }
    for arc in &h.value_arcs {
        let (f, r) = (&arc.frame, arc.radius);
        let blue = [26, 53, 184, 255];
        let at = |a: f64| f.origin + f.rotation * DVec3::new(a.cos() * r, a.sin() * r, 0.0);
        let n = ((arc.sweep / std::f64::consts::TAU * 64.0).ceil() as usize).max(1);
        let in_gap = |a: f64| arc.gap.is_some_and(|(c, w)| (a - c).abs() < w);
        for i in 0..n {
            let (a0, a1) = (arc.sweep * i as f64 / n as f64, arc.sweep * (i + 1) as f64 / n as f64);
            if in_gap(a0) || in_gap(a1) {
                continue;
            }
            lines.segments.push(LineSegment { a: at(a0).as_vec3().into(), b: at(a1).as_vec3().into(), color: blue, pick_id: 0, width: 1.5, style: LineSegment::DASHED | LineSegment::ON_TOP });
        }
        // Arrowheads at both ends, pointing away from the arc.
        if arc.sweep > 1e-6 {
            for (a, dir) in [(0.0, -1.0), (arc.sweep, 1.0)] {
                let tip = at(a);
                let tangent = f.rotation * DVec3::new(-a.sin(), a.cos(), 0.0) * dir;
                let radial = f.rotation * DVec3::new(a.cos(), a.sin(), 0.0);
                let len = r * 0.18;
                for side in [1.0, -1.0] {
                    let base = tip - tangent * len + radial * (side * len * 0.4);
                    lines.segments.push(LineSegment { a: tip.as_vec3().into(), b: base.as_vec3().into(), color: blue, pick_id: 0, width: 2.0, style: LineSegment::ON_TOP });
                }
            }
        }
    }
    for l in &h.value_lines {
        let blue = [26, 53, 184, 255];
        let at = |t: f64| l.a + (l.b - l.a) * t;
        let pieces = match l.gap {
            Some((g0, g1)) => vec![(0.0, g0.max(0.0)), (g1.min(1.0), 1.0)],
            None => vec![(0.0, 1.0)],
        };
        for (t0, t1) in pieces.into_iter().filter(|(t0, t1)| t1 > t0) {
            lines.segments.push(LineSegment { a: at(t0).as_vec3().into(), b: at(t1).as_vec3().into(), color: blue, pick_id: 0, width: 1.5, style: LineSegment::DASHED | LineSegment::ON_TOP });
        }
        let dir = (l.b - l.a).normalize_or_zero();
        if dir != DVec3::ZERO {
            let side = l.up.normalize_or_zero();
            for (tip, out) in [(l.a, -dir), (l.b, dir)] {
                for s in [1.0, -1.0] {
                    let base = tip - out * l.head + side * (s * l.head * 0.4);
                    lines.segments.push(LineSegment { a: tip.as_vec3().into(), b: base.as_vec3().into(), color: blue, pick_id: 0, width: 2.0, style: LineSegment::ON_TOP });
                }
            }
        }
    }
    // Interference detection: the hovered overlap's box (live analysis/83:
    // the query-hover yellow, ~6 px).
    for (lo, hi) in &h.boxes {
        let c = |k: usize| DVec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z });
        for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
            lines.segments.push(LineSegment { a: c(a).as_vec3().into(), b: c(b).as_vec3().into(), color: [242, 255, 128, 255], pick_id: 0, width: 6.0, style: LineSegment::ON_TOP });
        }
    }
    for (a, b) in &h.trails {
        lines.segments.push(LineSegment { a: a.as_vec3().into(), b: b.as_vec3().into(), color: DARK, pick_id: 0, width: 1.5, style: LineSegment::DASHED | LineSegment::ON_TOP });
    }
    if let Some((p, size)) = h.cursor_mark {
        for axis in [DVec3::X, DVec3::Y, DVec3::Z] {
            lines.segments.push(LineSegment { a: (p - axis * size).as_vec3().into(), b: (p + axis * size).as_vec3().into(), color: theme::rgb8(theme::pick(theme::viewport::PLANE_TEXT, theme::viewport_dark::PLANE_TEXT)), pick_id: 0, width: 1.5, style: LineSegment::ON_TOP });
        }
    }
    if !lines.segments.is_empty() {
        scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(20));
    }
    // The interfering material, red, drawn after the see-through parts
    // (its faces are coplanar with theirs; live assembly/736).
    for m in &h.red_meshes {
        scene.instances.push(Instance::new(Drawable::Mesh(m.clone())).with_color(theme::lin(0xc71010)).over_transparent().unpickable().layer(19));
    }
    // Pickable overlays: origin, assembly mate connectors, manipulator.
    let mut ol = LineData::default();
    let mut op = PointData::default();
    if let Some((size, hl)) = h.origin {
        let id = overlay_id(built, OverlayPick::Origin);
        push_triad(&mut ol, &Frame::default(), size, 1.5, id, hl);
        op.points.push(PointSprite { position: [0.0; 3], color: if hl { HANDLE_HL } else { DARK }, pick_id: id, size: 9.0, style: LineSegment::ON_TOP, _pad: 0 });
        scene.set_state(id, 0);
    }
    for (i, (f, size, hl, opacity)) in h.connectors.iter().enumerate() {
        let id = overlay_id(built, OverlayPick::Connector(i));
        let first = ol.segments.len();
        push_triad(&mut ol, f, *size, 2.0, id, *hl);
        let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        for seg in &mut ol.segments[first..] {
            seg.color[3] = alpha;
        }
        let mut dot = if *hl { HANDLE_HL } else { DARK };
        dot[3] = alpha;
        op.points.push(PointSprite { position: f.origin.as_vec3().into(), color: dot, pick_id: id, size: 9.0, style: LineSegment::ON_TOP, _pad: 0 });
        scene.set_state(id, 0);
    }
    // Onshape's mate glyph (assembly/41-43): a small square lying in the
    // connector plane, a white piece and a light-blue L inside a dark
    // outline (red when failing, gray when inactive, amber outline when
    // hovered or selected).
    for (i, (f, half, hl, st)) in h.mate_glyphs.iter().enumerate() {
        let id = overlay_id(built, OverlayPick::Mate(i));
        let (u, v, n) = (f.x(), f.y(), f.z());
        let lift = n * (*half * 0.02);
        let at = |a: f64, b: f64| -> [f32; 3] { (f.origin + lift + u * (a * half) + v * (b * half)).as_vec3().into() };
        let outline = match (hl, st) {
            (true, _) => HANDLE_HL,
            (_, MateGlyphState::Error) => theme::rgb8(theme::viewport::ERROR),
            _ => DARK,
        };
        // Filled with dense on-top strokes (meshes can't draw over the
        // parts): white top band and notch, light-blue L around them.
        let mut fill = |a0: f64, b0: f64, a1: f64, b1: f64, color: [u8; 4]| {
            let rows = ((b1 - b0) / 0.18).ceil().max(1.0) as usize;
            for r in 0..=rows {
                let b = b0 + (b1 - b0) * r as f64 / rows as f64;
                ol.segments.push(LineSegment { a: at(a0, b), b: at(a1, b), color, pick_id: id, width: 4.0, style: LineSegment::ON_TOP });
            }
        };
        const WHITE: [u8; 4] = [255, 255, 255, 255];
        let l = if *st == MateGlyphState::Inactive { [0xc8, 0xc8, 0xc8, 255] } else { [0x90, 0xce, 0xf1, 255] };
        fill(-0.45, -0.2, 1.0, 1.0, WHITE);
        fill(-0.45, -0.6, 0.3, -0.2, WHITE);
        fill(-1.0, -1.0, -0.45, 1.0, l);
        fill(-0.45, -1.0, 1.0, -0.6, l);
        fill(0.3, -0.6, 1.0, -0.2, l);
        let edge = |a: (f64, f64), b: (f64, f64)| LineSegment { a: at(a.0, a.1), b: at(b.0, b.1), color: outline, pick_id: id, width: if *hl { 2.0 } else { 1.2 }, style: LineSegment::ON_TOP };
        for (a, b) in [((-1.0, -1.0), (1.0, -1.0)), ((1.0, -1.0), (1.0, 1.0)), ((1.0, 1.0), (-1.0, 1.0)), ((-1.0, 1.0), (-1.0, -1.0))] {
            ol.segments.push(edge(a, b));
        }
        for (a, b) in [((-0.45, 1.0), (-0.45, -0.6)), ((-0.45, -0.6), (0.3, -0.6)), ((0.3, -0.6), (0.3, -0.2)), ((0.3, -0.2), (1.0, -0.2))] {
            ol.segments.push(LineSegment { width: 1.0, ..edge(a, b) });
        }
        scene.set_state(id, 0);
    }
    if let Some((f, size, hovered)) = &h.manipulator {
        // Onshape's triad (assembly/12): every part drawn as a hollow
        // stroke (light `#f7f7f8` core inside a `#969da3` edge): shafts with
        // outlined triangle heads, a small rotation ring with a tick at each
        // tip, plane parallelograms near the origin and a ring at the
        // center. The hovered handle turns blue.
        const EDGE: [u8; 4] = [0x96, 0x9d, 0xa3, 255];
        const CORE: [u8; 4] = [0xf7, 0xf7, 0xf8, 255];
        const HOVER: [u8; 4] = [0x3e, 0x94, 0xff, 255];
        const EXPLODE_CORE: [u8; 4] = [0xcd, 0xd0, 0xd3, 255];
        let size = *size;
        // Explode mode (live 802): a 2 px #cdd0d3 core between the grey edges.
        let (core, core_w) = if h.explode_triad { (EXPLODE_CORE, 2.0) } else { (CORE, 1.6) };
        let p3 = |p: DVec3| -> [f32; 3] { p.as_vec3().into() };
        let stroke = |ol: &mut LineData, pts: &[DVec3], closed: bool, id: u32, hl: bool| {
            let n = pts.len();
            let segs = if closed { n } else { n.saturating_sub(1) };
            for (w, c) in [(4.0, if hl { HOVER } else { EDGE }), (core_w, if hl { HOVER } else { core })] {
                for i in 0..segs {
                    ol.segments.push(LineSegment { a: p3(pts[i]), b: p3(pts[(i + 1) % n]), color: c, pick_id: id, width: w, style: LineSegment::ON_TOP });
                }
            }
        };
        let circle = |c: DVec3, u: DVec3, v: DVec3, r: f64| (0..24).map(|i| {
            let t = i as f64 / 24.0 * std::f64::consts::TAU;
            c + (u * t.cos() + v * t.sin()) * r
        }).collect::<Vec<_>>();
        let axes = [f.x(), f.y(), f.z()];
        for (k, axis) in axes.into_iter().enumerate() {
            let side = axes[(k + 1) % 3];
            let other = axes[(k + 2) % 3];
            // Shaft and outlined arrowhead (pickable as the axis).
            let id = overlay_id(built, OverlayPick::Manip(ManipHandle::Axis(k)));
            let hl = *hovered == Some(ManipHandle::Axis(k));
            let (r0, base, tip) = (size * 0.07, size * 0.62, size * 0.86);
            stroke(&mut ol, &[f.origin + axis * r0, f.origin + axis * base], false, id, hl);
            let w = size * 0.07;
            stroke(&mut ol, &[f.origin + axis * base + side * w, f.origin + axis * tip, f.origin + axis * base - side * w], true, id, hl);
            scene.set_state(id, 0);
            // Rotation handle at the tip: a small ring around the axis and a
            // tick across it.
            let rid = overlay_id(built, OverlayPick::Manip(ManipHandle::Ring(k)));
            let rhl = *hovered == Some(ManipHandle::Ring(k));
            let rc = f.origin + axis * (tip + size * 0.07);
            stroke(&mut ol, &circle(rc, side, other, size * 0.07), true, rid, rhl);
            stroke(&mut ol, &[rc - side * (size * 0.17), rc + side * (size * 0.17)], false, rid, rhl);
            scene.set_state(rid, 0);
            // Plane handle for the plane normal to axis k: a parallelogram
            // between the other two axes, with a faint pickable fill (not in
            // explode mode, live 802).
            if h.explode_triad {
                continue;
            }
            let (u, v) = (side, other);
            let pid = overlay_id(built, OverlayPick::Manip(ManipHandle::Plane(k)));
            let phl = *hovered == Some(ManipHandle::Plane(k));
            let (a0, a1) = (size * 0.2, size * 0.36);
            let q = |x: f64, y: f64| f.origin + u * x + v * y;
            stroke(&mut ol, &[q(a0, a0), q(a1, a0), q(a1, a1), q(a0, a1)], true, pid, phl);
            let fill = if phl { [HOVER[0], HOVER[1], HOVER[2], 70] } else { [0xf7, 0xf7, 0xf8, 40] };
            for i in 1..6 {
                let y = a0 + (a1 - a0) * i as f64 / 6.0;
                ol.segments.push(LineSegment { a: p3(q(a0, y)), b: p3(q(a1, y)), color: fill, pick_id: pid, width: 4.0, style: LineSegment::ON_TOP });
            }
            scene.set_state(pid, 0);
        }
        // Center ring (moves the triad itself).
        let cid = overlay_id(built, OverlayPick::Manip(ManipHandle::Center));
        let chl = *hovered == Some(ManipHandle::Center);
        let o: [f32; 3] = p3(f.origin);
        op.points.push(PointSprite { position: o, color: if chl { HOVER } else { EDGE }, pick_id: cid, size: 17.0, style: LineSegment::ON_TOP, _pad: 0 });
        op.points.push(PointSprite { position: o, color: core, pick_id: cid, size: 12.0, style: LineSegment::ON_TOP, _pad: 0 });
        op.points.push(PointSprite { position: o, color: if chl { HOVER } else { EDGE }, pick_id: cid, size: 9.0, style: LineSegment::ON_TOP, _pad: 0 });
        op.points.push(PointSprite { position: o, color: [255, 255, 255, 255], pick_id: cid, size: 7.0, style: LineSegment::ON_TOP, _pad: 0 });
        scene.set_state(cid, 0);
    }
    if let Some((o, dir, size, rotation)) = h.step_arrow
        && let Some(d) = dir.try_normalize()
    {
        // The explode triad's hollow stroke (grey edge, #cdd0d3 core), unpickable.
        const EDGE: [u8; 4] = [0x96, 0x9d, 0xa3, 255];
        const CORE: [u8; 4] = [0xcd, 0xd0, 0xd3, 255];
        let p3 = |p: DVec3| -> [f32; 3] { p.as_vec3().into() };
        let mut stroke = |pts: &[DVec3], closed: bool| {
            let n = pts.len();
            let segs = if closed { n } else { n.saturating_sub(1) };
            for (w, c) in [(4.0, EDGE), (2.0, CORE)] {
                for i in 0..segs {
                    ol.segments.push(LineSegment { a: p3(pts[i]), b: p3(pts[(i + 1) % n]), color: c, pick_id: 0, width: w, style: LineSegment::ON_TOP });
                }
            }
        };
        let side = d.any_orthonormal_vector();
        let other = d.cross(side);
        if rotation {
            // A ring about the axis, open at the end with an arrowhead.
            let r = size * 0.3;
            let pts: Vec<DVec3> = (0..=40).map(|i| {
                let t = i as f64 / 40.0 * std::f64::consts::TAU * 0.85;
                o + (side * t.cos() + other * t.sin()) * r
            }).collect();
            let (end, tan) = {
                let t = std::f64::consts::TAU * 0.85;
                (o + (side * t.cos() + other * t.sin()) * r, (-side * t.sin() + other * t.cos()).normalize())
            };
            stroke(&pts, false);
            let w = size * 0.07;
            let nrm = (end - o).normalize();
            stroke(&[end + nrm * w, end + tan * (size * 0.14), end - nrm * w], true);
        } else {
            let (base, tip, w) = (size * 0.62, size * 0.86, size * 0.07);
            stroke(&[o, o + d * base], false);
            stroke(&[o + d * base + side * w, o + d * tip, o + d * base - side * w], true);
        }
    }
    if let Some(g) = h.section
        && let Some(d) = g.dir.try_normalize()
    {
        push_section_glyph(&mut ol, &|h| overlay_id(built, OverlayPick::Section(h)), &g, d);
    }
    if !ol.segments.is_empty() {
        scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&ol))).per_primitive_color().ignore_clip().layer(22));
    }
    if !op.points.is_empty() {
        scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&op))).per_primitive_color().ignore_clip().layer(23));
    }
    if !pts.points.is_empty() {
        scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&pts))).per_primitive_color().unpickable().ignore_clip().layer(21));
    }
    scene
}

fn lin_to_u8(c: [f32; 4]) -> [u8; 4] {
    let f = |x: f32| (if x <= 0.0031308 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 } * 255.0).round().clamp(0.0, 255.0) as u8;
    [f(c[0]), f(c[1]), f(c[2]), 255]
}

/// Onshape's mate connector glyph (live qa5/asm-041, assembly/31-*): a khaki
/// disc in the connector's XY plane with a notch between +X and +Y, a dark
/// rim, and a small triad (Z up from the centre, X and Y outside the disc).
pub fn push_connector_glyph(lines: &mut LineData, f: &Frame, r: f64) {
    const FILL: [u8; 4] = [187, 190, 134, 255];
    const RIM: [u8; 4] = [42, 81, 105, 255];
    let (u, v, n) = (f.x(), f.y(), f.z());
    let at = |rad: f64, a: f64| -> [f32; 3] { (f.origin + u * (rad * a.cos()) + v * (rad * a.sin())).as_vec3().into() };
    let (gap0, gap1) = (15f64.to_radians(), 75f64.to_radians());
    let steps = 40;
    let sweep = std::f64::consts::TAU - (gap1 - gap0);
    let arc = |lines: &mut LineData, rad: f64, color: [u8; 4], width: f32| {
        for i in 0..steps {
            let (a0, a1) = (gap1 + sweep * i as f64 / steps as f64, gap1 + sweep * (i + 1) as f64 / steps as f64);
            lines.segments.push(LineSegment { a: at(rad, a0), b: at(rad, a1), color, pick_id: 0, width, style: LineSegment::ON_TOP });
        }
    };
    // Filled with concentric strokes (meshes can't draw over the parts).
    let rings = 6;
    for k in 1..=rings {
        arc(lines, r * k as f64 / rings as f64 * 0.95, FILL, 3.0);
    }
    arc(lines, r, RIM, 1.5);
    for a in [gap0, gap1] {
        lines.segments.push(LineSegment { a: at(0.0, a), b: at(r, a), color: RIM, pick_id: 0, width: 1.5, style: LineSegment::ON_TOP });
    }
    let o = f.origin;
    let seg = |lines: &mut LineData, a: DVec3, b: DVec3, color: [u8; 4]| lines.segments.push(LineSegment { a: a.as_vec3().into(), b: b.as_vec3().into(), color, pick_id: 0, width: 2.5, style: LineSegment::ON_TOP });
    seg(lines, o, o + n * (r * 1.3), [0, 0, 208, 255]);
    seg(lines, o + u * (r * 1.25), o + u * (r * 1.9), [208, 24, 24, 255]);
    seg(lines, o + v * (r * 1.25), o + v * (r * 1.9), [24, 168, 24, 255]);
}

/// Live's section manipulator (assembly/2368–2370): idle strokes light grey
/// (core ≈ 230,232,233, edge ≈ 194,199,203); a hovered handle blue (≈ 70,123,177);
/// the dragged one deeper blue (≈ 40,97,182). The base circle faces the
/// viewer; the shaft and hollow head; two small rings at the tip, each with
/// its axis line (ring 0 about the plane's Y, ring 1 about its X). `id`
/// gives each handle's pick id (the Part Studio's are unpickable).
pub fn push_section_glyph(ol: &mut LineData, id: &dyn Fn(SectionHandle) -> u32, g: &SectionGlyph, d: DVec3) {
    const CORE: [u8; 4] = [230, 232, 233, 255];
    const EDGE: [u8; 4] = [194, 199, 203, 255];
    const HOVER: [u8; 4] = [70, 123, 177, 255];
    const DRAG: [u8; 4] = [40, 97, 182, 255];
    let p3 = |p: DVec3| -> [f32; 3] { p.as_vec3().into() };
    let w = g.wpp;
    let side = d.cross(g.view).try_normalize().unwrap_or_else(|| d.any_orthonormal_vector());
    let up = g.view.cross(side).try_normalize().unwrap_or(d);
    let stroke = |ol: &mut LineData, handle: SectionHandle, pts: &[DVec3], closed: bool| {
        let pick_id = id(handle);
        let n = pts.len();
        let segs = if closed { n } else { n.saturating_sub(1) };
        let styles: &[([u8; 4], f32)] = if g.dragging == Some(handle) {
            &[(DRAG, 2.5)]
        } else if g.hovered == Some(handle) {
            &[(HOVER, 2.5)]
        } else {
            &[(EDGE, 3.0), (CORE, 1.5)]
        };
        for (c, wd) in styles {
            for i in 0..segs {
                ol.segments.push(LineSegment { a: p3(pts[i]), b: p3(pts[(i + 1) % n]), color: *c, pick_id, width: *wd, style: LineSegment::ON_TOP });
            }
        }
    };
    let circle = |c: DVec3, u: DVec3, v: DVec3, r: f64| -> Vec<DVec3> {
        (0..32)
            .map(|i| {
                let t = i as f64 / 32.0 * std::f64::consts::TAU;
                c + (u * t.cos() + v * t.sin()) * r
            })
            .collect()
    };
    // Base circle (facing the viewer).
    stroke(ol, SectionHandle::Base, &circle(g.base, side, up, SECTION_BASE_R_PX * w), true);
    // Shaft and head.
    let (b0, hb, tip) = (g.base + d * SECTION_BASE_R_PX * w, g.base + d * SECTION_HEAD_PX * w, g.base + d * SECTION_TIP_PX * w);
    stroke(ol, SectionHandle::Arrow, &[b0, hb], false);
    stroke(ol, SectionHandle::Arrow, &[hb + side * 10.0 * w, tip, hb - side * 10.0 * w], true);
    // The rings with their axis lines.
    let c = g.base + d * SECTION_RINGS_PX * w;
    for (k, axis) in [(0usize, g.y), (1, g.x)] {
        let Some(a) = axis.try_normalize() else { continue };
        let v = a.cross(d);
        stroke(ol, SectionHandle::Ring(k), &circle(c, d, v, 6.0 * w), true);
        stroke(ol, SectionHandle::Ring(k), &[c - a * 20.0 * w, c + a * 20.0 * w], false);
    }
    // The drag's start: "⊥" at the start base with a leader to the base
    // now; "⊢" the start direction from the base.
    if let Some((s, rotation)) = g.start {
        let mut mark = |a: DVec3, b: DVec3| ol.segments.push(LineSegment { a: p3(a), b: p3(b), color: DRAG, pick_id: 0, width: 1.5, style: LineSegment::ON_TOP });
        if rotation {
            let e = g.base + s * SECTION_HEAD_PX * w;
            let t = s.cross(g.view).try_normalize().unwrap_or(side) * 10.0 * w;
            mark(g.base, e);
            mark(e - t, e + t);
        } else {
            mark(s - side * 15.0 * w, s + side * 15.0 * w);
            mark(s, g.base);
        }
    }
}
