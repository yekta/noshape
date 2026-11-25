//! Render scene for an evaluated assembly: every part occurrence is drawn
//! from GPU buffers cached per `Arc<MeshBuffers>` with its own instance
//! transform, so moving instances (drag, animation, explode) never
//! re-uploads geometry. Pick ids map back to (instance path, reference).

use crate::builder::{PickKind, MESH_LINE_COLOR};
use crate::palette as theme;
use glam::{DMat4, DVec3, Mat4};
use noshape_model::assembly::eval::{AssemblyResult, InstancePath, PartOccurrence};
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
    /// Each occurrence's pick ids (faces, edges and vertices: one run, first..=last) by its path.
    by_path: HashMap<InstancePath, Vec<(u32, u32)>>,
}

impl AsmPickMap {
    fn push(&mut self, t: AsmPickTarget) -> u32 {
        let id = self.targets.len() as u32 + 1;
        if t.kind == PickKind::Face {
            self.by_top.entry(t.path[0]).or_default().push(id);
        }
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
    /// The pick ids pushed since `first` belong to the occurrence at `path`.
    fn close_occurrence(&mut self, path: &InstancePath, first: u32) {
        let last = self.targets.len() as u32;
        if last >= first {
            self.by_path.entry(path.clone()).or_default().push((first, last));
        }
    }
    /// The pick ids of the occurrence at `path` with their targets.
    fn at_path<'a>(&'a self, path: &InstancePath) -> impl Iterator<Item = (u32, &'a AsmPickTarget)> + 'a {
        self.by_path.get(path).into_iter().flatten().flat_map(|&(a, b)| a..=b).filter_map(|id| Some((id, self.get(id)?)))
    }
    pub fn ids_for<'a>(&'a self, path: &InstancePath, r: &'a Reference) -> impl Iterator<Item = u32> + 'a {
        self.at_path(path).filter(move |(_, t)| t.reference == *r).map(|(id, _)| id)
    }
    /// Pick ids of one occurrence's faces.
    pub fn faces_at(&self, path: &InstancePath) -> Vec<u32> {
        self.at_path(path).filter(|(_, t)| matches!(t.reference, Reference::Face(_))).map(|(id, _)| id).collect()
    }
    /// Pick ids of an instance's edges.
    pub fn edges_of(&self, top: noshape_model::assembly::InstanceId) -> Vec<u32> {
        let paths = self.by_path.keys().filter(|p| p.first() == Some(&top));
        paths.flat_map(|p| self.at_path(p)).filter(|(_, t)| matches!(t.reference, Reference::Edge(_))).map(|(id, _)| id).collect()
    }
    /// Pick ids of the edges bounding a face (edge names hold their faces').
    pub fn boundary_ids(&self, path: &InstancePath, face: &Reference) -> Vec<u32> {
        let Reference::Face(n) = face else { return Vec::new() };
        self.at_path(path)
            .filter(|(_, t)| matches!(&t.reference, Reference::Edge(noshape_model::reference::TopoName::Edge { faces, .. }) if faces.iter().any(|f| **f == *n)))
            .map(|(id, _)| id)
            .collect()
    }
    pub fn priority_fn(&self) -> Arc<dyn Fn(u32) -> i32 + Send + Sync> {
        let prio: Vec<i32> = self.targets.iter().map(|t| t.kind.priority()).collect();
        Arc::new(move |id| if id == 0 { -1 } else if noshape_render::section_cap_of_pick(id).is_some() { 0 } else { prio.get(id as usize - 1).copied().unwrap_or(100) })
    }

    /// Each pick id's class (an edge reached further over a face, as in a Part Studio: live reach-px-1119); a vertex
    /// within the pick radius, not the Part Studio's square (live mc-infer-1122 reach-asm-corner: lit 6 px off, the
    /// edge at 10 along the diagonal; the Part Studio's 9 px diagonal is the square's, interaction-feel "Vertex").
    pub fn class_fn(&self) -> Arc<dyn Fn(u32) -> noshape_render::PickClass + Send + Sync> {
        let class: Vec<noshape_render::PickClass> = self
            .targets
            .iter()
            .map(|t| if t.kind == crate::builder::PickKind::Vertex { noshape_render::PickClass::Vertex } else { crate::builder::pick_class(t.kind) })
            .collect();
        Arc::new(move |id| if id == 0 { noshape_render::PickClass::Other } else { class.get(id as usize - 1).copied().unwrap_or(noshape_render::PickClass::Other) })
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
    /// Each drawn occurrence's scene instances, for [`retransform`].
    pub spans: Vec<OccSpan>,
    /// The result's occurrence count when built.
    pub occurrences: usize,
}

/// The scene instances `start..end` of occurrence `occ`, all drawn at its
/// transform except its bounding-box level of detail (`lod`: instance index
/// and its box matrix in the occurrence's frame).
#[derive(Clone, Debug)]
pub struct OccSpan {
    pub occ: usize,
    /// Its top-level instance.
    pub top: noshape_model::assembly::InstanceId,
    pub start: usize,
    pub end: usize,
    pub lod: Option<(usize, Mat4)>,
}

/// Moves the built scene's occurrences to `result`'s transforms without
/// rebuilding it (a drag step: 1000 pins rebuilt every frame cost ~20 ms
/// of picks and instances). False when the occurrences changed (rebuild).
pub fn retransform(built: &mut AsmBuilt, result: &AssemblyResult) -> bool {
    if built.occurrences != result.occurrences.len() {
        return false;
    }
    for span in &built.spans {
        let o = &result.occurrences[span.occ];
        if o.hidden {
            return false;
        }
        let t = to_mat4(&o.transform);
        for i in span.start..span.end {
            let inst = &mut built.scene.instances[i];
            inst.transform = match span.lod {
                Some((li, local)) if li == i => t * local,
                _ => t,
            };
        }
    }
    true
}

#[derive(Clone, Default)]
pub struct AsmSceneOptions {
    /// Instances drawn transparent (isolate mode shows the rest dimmed).
    pub isolate: Option<noshape_model::assembly::OccurrenceSet>,
    /// Instances shown with the interference / error tint.
    pub error_tops: HashSet<noshape_model::assembly::InstanceId>,
    /// Instances drawn transparent and not pickable (being dragged in snap mode).
    pub ghost_tops: HashSet<noshape_model::assembly::InstanceId>,
    /// Occurrences drawn transparent (Make transparent, also inside
    /// subassemblies).
    pub ghost_paths: noshape_model::assembly::OccurrenceSet,
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
    /// Isolate › Show internal geometry (live asm/isolate-1122): the isolated instances drawn again over the
    /// see-through ones, which show through them only as faint edges.
    pub front_internal: bool,
    /// Zebra stripes on the shaded parts, and whether edges are hidden.
    pub zebra: Option<noshape_render::Zebra>,
    /// Reflection analysis environment on the shaded parts.
    pub reflection: Option<noshape_render::Reflection>,
    pub hide_edges: bool,
    /// Camera menu render choices, as in the Part Studio (`SceneOptions`).
    pub display_mode: crate::builder::DisplayMode,
    pub hidden_edges_visible: bool,
    /// Instances whose edges also show through what's in front of them, in the part highlight's amber (an instance
    /// being placed from Insert: live mates.json insert_dialog preview).
    pub xray_tops: HashSet<noshape_model::assembly::InstanceId>,
    /// The part a mate's preview moves (also in `xray_tops`): its edges peach, (247,193,113) in view and
    /// (220,185,113) through the others (live feel cov-1120 A2, qa asm-mate-preview-0bc7).
    pub preview_tops: HashSet<noshape_model::assembly::InstanceId>,
    pub tangent_edges: crate::builder::TangentEdges,
    /// Line width and phantom edges' color and width.
    pub graphics: crate::graphics_prefs::GraphicsPrefs,
    /// A software rasteriser draws the scene (`Gpu::is_software`): each dense body's coarsest level is its
    /// [`MeshBuffers::proxy`] (the renderer draws every body at its coarsest there).
    pub software: bool,
}

impl AsmSceneOptions {
    /// Whether a scene built with `self` draws as one built with `other`
    /// (a reflection environment always counts as changed).
    pub fn same_display(&self, other: &Self) -> bool {
        self.reflection.is_none()
            && other.reflection.is_none()
            && self.isolate == other.isolate
            && self.error_tops == other.error_tops
            && self.ghost_tops == other.ghost_tops
            && self.ghost_paths == other.ghost_paths
            && self.section == other.section
            && self.section_skip == other.section_skip
            && self.section_selected == other.section_selected
            && self.dim_alpha == other.dim_alpha
            && self.pick_dimmed == other.pick_dimmed
            && self.front_internal == other.front_internal
            && self.zebra == other.zebra
            && self.hide_edges == other.hide_edges
            && self.display_mode == other.display_mode
            && self.hidden_edges_visible == other.hidden_edges_visible
            && self.xray_tops == other.xray_tops
            && self.preview_tops == other.preview_tops
            && self.tangent_edges == other.tangent_edges
            && self.graphics == other.graphics
            && self.software == other.software
    }
}

/// What a built scene depends on in `result` besides the placements: each
/// occurrence's path, meshes, colour, state and source (a scene whose
/// signature is unchanged only needs [`retransform`]).
pub fn occurrence_signature(result: &AssemblyResult) -> Vec<u64> {
    let mut out = Vec::with_capacity(result.occurrences.len() * 8);
    for o in &result.occurrences {
        out.push(o.path.len() as u64);
        out.extend(o.path.iter().map(|i| i.0));
        out.push(Arc::as_ptr(&o.mesh) as usize as u64);
        out.extend(o.coarser.iter().map(|m| Arc::as_ptr(m) as usize as u64));
        out.push(Arc::as_ptr(&o.context) as usize as u64);
        out.extend(o.color.iter().map(|c| u64::from(c.to_bits())));
        out.push(u64::from(o.levels_from) | u64::from(o.hidden) << 8 | u64::from(o.mirrored) << 9 | (o.kind as u64) << 10);
    }
    out
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
    let t0 = std::time::Instant::now();
    prepare_bodies(result, opts.software, cache, up);
    let prepared = t0.elapsed();
    let mut pick_time = std::time::Duration::ZERO;
    use crate::builder::{DisplayMode, TangentEdges};
    let mode = opts.display_mode;
    scene.no_occlusion = matches!(mode, DisplayMode::HiddenLine | DisplayMode::Translucent);
    let mut bodies = 0u32;
    let mut spans = Vec::new();
    for (oi, o) in result.occurrences.iter().enumerate() {
        if o.hidden {
            continue;
        }
        let mk = |r: &Reference, kind, anchor| AsmPickTarget { path: o.path.clone(), reference: r.clone(), kind, occ: oi, anchor };
        let tp = std::time::Instant::now();
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
        picks.close_occurrence(&o.path, face_base + 1);
        pick_time += tp.elapsed();
        let t = to_mat4(&o.transform);
        let first_instance = scene.instances.len();
        let mut color = o.color;
        let ghost = opts.ghost_tops.contains(&o.top()) || noshape_model::assembly::occurrence_in(&opts.ghost_paths, &o.path);
        let dimmed = opts.isolate.as_ref().is_some_and(|s| !noshape_model::assembly::occurrence_in(s, &o.path)) || ghost;
        if ghost {
            // Live's client SNAP_SOURCE_OCCURRENCE_OPACITY (mates.json snap_mode).
            color[3] = opts.dim_alpha.unwrap_or(0.3);
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
        let levels = display_levels(o, opts.software);
        let previewed = opts.preview_tops.contains(&o.top());
        // Isolate's knob at the far left: the others aren't drawn (live asm/isolate-1122 ps-01-off-0).
        let gone = dimmed && !ghost && color[3] <= 0.0;
        let front = opts.front_internal && opts.isolate.is_some() && !dimmed && !sketch;
        let edge_color = if sketch { theme::lin(0x999999) } else if previewed { theme::lin(0xf7c171) } else { theme::lin(theme::viewport::EDGE) };
        let mut finest_cached = None;
        for (li, mesh) in levels.iter().enumerate() {
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
            if !gone && (!sketch || !mesh.indices.is_empty()) {
                let mut inst = Instance::new(Drawable::Mesh(cached.mesh.clone())).with_color(color).with_transform(t).pick_base(face_base).zebra(opts.zebra.filter(|_| !dimmed && !sketch)).reflection(opts.reflection.clone().filter(|_| !dimmed && !sketch)).depth_rank(oi);
                // Sketch regions are pickable faces (live 601).
                if dimmed && !opts.pick_dimmed {
                    inst = inst.unpickable();
                }
                // The display modes as in the Part Studio (scene_builder): front faces see-through at 0.25, or unlit
                // faces in the background color (Unshaded).
                if !sketch {
                    inst = match mode {
                        DisplayMode::Translucent => inst.with_color([color[0], color[1], color[2], color[3].min(0.25)]).front_only(),
                        DisplayMode::HiddenLine => inst.with_color(theme::lin(0xffffff)).unlit(),
                        DisplayMode::ShadedWithEdges | DisplayMode::Shaded => inst,
                    };
                }
                if opts.xray_tops.contains(&o.top()) {
                    inst = inst.see_through_self();
                }
                if front {
                    scene.instances.push(inst.clone().over_transparent());
                }
                scene.instances.push(inst);
            }
            if dimmed && !gone && opts.front_internal {
                let e = theme::lin(theme::viewport::EDGE);
                scene.instances.push(Instance::new(Drawable::Lines(cached.edges.clone())).with_color([e[0], e[1], e[2], 0.08]).with_transform(t).unpickable().hidden_only().layer(1));
            }
            if opts.xray_tops.contains(&o.top()) {
                let hidden = if previewed { 0xdcb971 } else { 0xffc04c };
                scene.instances.push(Instance::new(Drawable::Lines(cached.edges.clone())).with_color(theme::lin(hidden)).with_transform(t).unpickable().hidden_only().see_through_self().layer(1));
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
                                    sharp.push_polyline(pl, [0, 0, 0, 255], ei as u32 + 1, noshape_render::scene::PART_EDGE_WIDTH, LineSegment::NO_HOLD | LineSegment::DRAWN_PICK);
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
                                l.push_polyline(pl, [217, 217, 217, 255], 0, 1.0, 0);
                            }
                            let h = up.upload_lines(&l);
                            cache.hidden_edges.insert(key, (Arc::downgrade(mesh), h.clone()));
                            h
                        }
                    };
                    scene.instances.push(Instance::new(Drawable::Lines(h)).per_primitive_color().with_transform(t).unpickable().hidden_only().layer(1));
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
        let mut lod_at = None;
        if let Some(bounds) = cached.mesh.bounds().filter(|_| !sketch) {
            let unit = cache.lod_box.get_or_insert_with(|| up.upload_mesh(&noshape_render::scene::lod_box_mesh())).clone();
            let mut lod = Instance::lod_box(unit, bodies, bounds, color);
            lod_at = Some((scene.instances.len(), lod.transform));
            lod.transform = t * lod.transform;
            scene.instances.push(lod);
        }
        spans.push(OccSpan { occ: oi, top: o.top(), start: first_instance, end: scene.instances.len(), lod: lod_at });
    }
    log::info!(target: "perf", "assembly scene: bodies prepared in {} ms, {} picks in {} ms, all in {} ms", prepared.as_millis(), picks.len(), pick_time.as_millis(), t0.elapsed().as_millis());
    let static_instances = scene.instances.len();
    scene.states = vec![0; picks.len() + 1];
    let clip = |(n, d): &(DVec3, f64)| noshape_render::ClipPlane::new(n.as_vec3(), *d as f32);
    scene.clip = opts.section.first().map(clip);
    scene.clip_extra = opts.section.iter().skip(1).map(clip).collect();
    scene.section_interference = true;
    if let Some(slot) = opts.section_selected.and_then(|i| scene.clip_selected.get_mut(i)) {
        *slot = true;
    }
    AsmBuilt { scene, picks, static_instances, spans, occurrences: result.occurrences.len() }
}

/// An occurrence's display levels, coarsest first, its finest last; on a software rasteriser (`software`) a dense
/// body's coarsest is its [`MeshBuffers::proxy`].
pub fn display_levels(o: &PartOccurrence, software: bool) -> Vec<Arc<MeshBuffers>> {
    let proxy = o.coarser.first().filter(|_| software).and_then(|m| m.proxy());
    let coarsest = proxy.into_iter().chain(o.coarser.first().cloned()).take(1);
    coarsest.chain(o.coarser.iter().skip(1).cloned()).chain(std::iter::once(o.mesh.clone())).collect()
}

/// The visible occurrences' meshes not yet uploaded, made on every core (infento's ~0.5 s one after another).
fn prepare_bodies(result: &AssemblyResult, software: bool, cache: &mut AsmGpuCache, up: &Uploader) {
    let mut seen = HashSet::new();
    let todo: Vec<Arc<MeshBuffers>> = result
        .occurrences
        .iter()
        .filter(|o| !o.hidden)
        .flat_map(|o| display_levels(o, software))
        .filter(|m| {
            let key = Arc::as_ptr(m) as usize;
            !cache.bodies.contains_key(&key) && seen.insert(key)
        })
        .collect();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(todo.len());
    if threads < 2 {
        return;
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let made: Vec<(usize, CachedBody)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut out = Vec::new();
                    while let Some(m) = todo.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) {
                        out.push((Arc::as_ptr(m) as usize, upload_body(m, up)));
                    }
                    out
                })
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect()
    });
    let by_key: HashMap<usize, &Arc<MeshBuffers>> = todo.iter().map(|m| (Arc::as_ptr(m) as usize, m)).collect();
    for (key, body) in made {
        if let Some(m) = by_key.get(&key) {
            cache.bodies.insert(key, (Arc::downgrade(m), Arc::new(body)));
        }
    }
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
        // (Hovered, an assembly's edge holds nothing further off: live/feel/3d.json lights it at 6 px, lit to 6.)
        lines.push_polyline(pl, [0, 0, 0, 255], ei as u32 + 1, noshape_render::scene::PART_EDGE_WIDTH, LineSegment::NO_HOLD | LineSegment::DRAWN_PICK);
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
    /// Instances hovered as a whole (their row in the list): outlined in hover orange, no tint, their edges as at
    /// rest (live asm/feel-1117 c-a-rowhover; the Part Studio's hovered part).
    pub hover_tops: Vec<noshape_model::assembly::InstanceId>,
    /// Instances a crossing box touches while dragged: outlined in #FFC20E, as a hovered part (live psui/sel-gestures-1122
    /// asm box crossing mid, qa FEEL-ASM-CROSSING-OUTLINE).
    pub crossing_tops: Vec<noshape_model::assembly::InstanceId>,
    /// The part whose own mate connector is hovered or selected: banded as a mate pick's part (live hover-matrix-1121
    /// as2-mc-*: #dab46b..#e9c177, qa FEEL-ASM-MC-LOOK), not the #ffa033 instance band a selected face gives it.
    pub mc_owner: Option<InstancePath>,
    /// The selected entities are a dialog field's pick (a pattern's axis or direction): no instance band, the entity
    /// alone lit (live workflows-1123 04 / 05, qa FEEL-ASM-PATTERN-PICK-LOOK).
    pub entities_only: bool,
    pub hover_entity: Option<(InstancePath, Reference)>,
    /// A hovered vertex under a mate field's white corner mark: its disc depth-tested, the mark over its middle (live
    /// hover-matrix-1121 b5-fastened-vertex).
    pub hover_vertex_open: bool,
    pub selected_entities: Vec<(InstancePath, Reference)>,
    pub triads: Vec<Triad>,
    /// Inference dots (world positions).
    pub dots: Vec<DVec3>,
    /// The face whose boundary is outlined (live: hovering a face while
    /// picking a mate connector outlines it, no fill).
    pub hover_outline: Option<(InstancePath, Reference)>,
    /// A curved `hover_outline` face: outlined round its whole image, its hidden part too (a cylinder side's
    /// silhouettes and bottom rim; a planar face keeps its boundary lines under its corner dots).
    pub hover_outline_image: bool,
    /// A dialog's hovered selection row's entities: yellow-green (`state::QUERY`).
    pub query_entities: Vec<(InstancePath, Reference)>,
    /// Entities a mate dialog picked: filled yellow (`state::PICKED`), their boundary #FFC20E (live feel cov-1120 A2:
    /// (229,183,37) inside (255,194,14), qa asm-mate-preview-0bc7; a selected face is amber inside 3 px #ffa90a).
    pub picked_entities: Vec<(InstancePath, Reference)>,
    /// The parts a mate dialog's picks are on (the moving one left out once both are in): a 2 px (242,195,108) band at
    /// 85 % over the silhouette line, through the parts in front (live workflows-1122 08 s06-s08).
    pub picked_parts: Vec<InstancePath>,
    /// The hovered part's occurrence: its silhouette in the hover colour, no tint (live2 asm-feel (3b): only that
    /// part, inside a subassembly too).
    pub hover_part: Option<InstancePath>,
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
    /// Pickable assembly origin (triad size) and whether it is hovered/selected.
    pub origin: Option<(f64, bool)>,
    /// The origin as a triad (picking it as a mate connector's origin); else the Part Studio's ring with a dot.
    pub origin_triad: bool,
    /// The Origin row hovered in the list: the origin lit (live: its ring and dot over a light orange disc).
    pub origin_row_hover: bool,
    /// Pickable mate connectors (the assembly's, then its parts' own): frame, size, look, opacity (the connector
    /// dialog's preview slider fades the edited one).
    pub connectors: Vec<(Frame, f64, crate::builder::ConnectorLook, f32)>,
    /// Mate glyph lines (between connector origins).
    pub mate_lines: Vec<(DVec3, DVec3, bool)>,
    /// Pickable mate indicators: (connector frame, highlighted, state), drawn flat in the connector's XY plane like
    /// Onshape's, at screen size ([`MateGlyphs`]).
    pub mate_glyphs: Vec<(Frame, bool, MateGlyphState, noshape_model::assembly::MateType)>,
    /// Explode lines (dashed), start to exploded position.
    pub trails: Vec<(DVec3, DVec3)>,
    /// A triad handle being dragged: the other handles faded (live manip-feel-1118 m9).
    pub manip_dragging: bool,
    /// The dragged arrow's or ring's own dimension, from a tick at its start.
    pub drag_dim: Option<DragDim>,
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

/// A triad drag's dimension (live manip-feel-1118 m9): a solid blue line from where the dragged arrow started to
/// where it is, or an arc about the ring's axis from where the dragged ring started through `sweep`; across the start
/// a tick (`tick`: half its length and its direction).
#[derive(Clone, Copy, Debug)]
pub enum DragDim {
    Line { start: DVec3, end: DVec3, tick: DVec3 },
    Arc { center: DVec3, from: DVec3, axis: DVec3, sweep: f64, tick: DVec3 },
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

impl ManipHandle {
    /// The axis a triad ring turns about: the ring at axis k's tip lies in the plane of axes k and k+1 and turns
    /// about axis k+2 (live asm-triad-rotate-90: the ring at Z's tip turns about Y).
    pub fn ring_axis(k: usize) -> usize {
        (k + 2) % 3
    }
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

/// The move triad's size in pixels at any zoom (live 3087: the Z arrow's tip 86 px from the origin, its ring's
/// centre 94 px, r 8 px, the ring's bar ±20 px; the glyph's proportions are of this size).
pub const MANIP_SIZE_PX: f64 = 100.0;

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

/// The first overlay pick id: the scene's own geometry picks lie under it.
pub fn overlay_base(built: &AsmBuilt) -> u32 {
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
    if id < b || noshape_render::section_cap_of_pick(id).is_some() {
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

/// Mate coordinate systems at screen size: each (frame, axis length in px, strong). Live's client draws a mate's axes
/// MATE_COORDINATE_SYSTEM_AXIS_LINE_LENGTH_PX 50 long whatever the zoom (mates.json after_second_pick).
pub struct ScreenTriads {
    pub items: Vec<(Frame, f64, bool)>,
}

/// [`ScreenTriads`]' full axis length (px).
pub const MATE_AXIS_PX: f64 = 50.0;

impl ScreenTriads {
    /// The axes and origin dots for `view`.
    fn geometry(&self, view: &noshape_render::camera_geometry::View) -> (LineData, PointData) {
        let (mut lines, mut pts) = (LineData::default(), PointData::default());
        for (f, len, strong) in &self.items {
            let Some(px) = view.world_per_px(f.origin) else { continue };
            push_triad(&mut lines, f, len * px, if *strong { 2.5 } else { 1.5 }, 0, false);
            pts.points.push(PointSprite { position: f.origin.as_vec3().into(), color: DARK, pick_id: 0, size: if *strong { 7.0 } else { 5.0 }, style: LineSegment::ON_TOP, _pad: 0 });
        }
        (lines, pts)
    }
}

impl noshape_render::camera_geometry::CameraGeometry for ScreenTriads {
    fn key(&self) -> Option<u64> {
        Some(noshape_render::camera_geometry::key_of("ScreenTriads", &self.items))
    }

    fn build(&self, view: &noshape_render::camera_geometry::View, up: &Uploader) -> Vec<Instance> {
        let (lines, pts) = self.geometry(view);
        if lines.segments.is_empty() {
            return Vec::new();
        }
        vec![
            Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(20),
            Instance::new(Drawable::Points(up.upload_points(&pts))).per_primitive_color().unpickable().ignore_clip().layer(21),
        ]
    }
}

/// A mate glyph's drawing unit on screen (px): its icons span about ±0.75 of it, ~27 px across face-on (live2
/// mate-glyphs 11-17, the 4x crops).
pub const MATE_GLYPH_UNIT_PX: f64 = 18.0;
/// The radius round a glyph's centre that takes the pointer before the 5 px pick square (live2 asm-feel2 (b): hovered
/// to ~20 px from the centre).
const MATE_GLYPH_PICK_PX: f64 = 15.0;

/// Onshape's mate glyphs at screen size (live2 mate-glyphs 11-17): each (connector frame, pick id, highlighted, state,
/// mate type). A ~26 px icon per mate type lying in the connector's plane, light blue rgb(152,204,234) with a dark
/// outline and white parts: Fastened a white block in a U socket, Revolute and Cylindrical a ¾-turn arc arrow round a
/// dot, Slider a ring round a dot, Planar X and Y arrows from a white corner square, Pin slot the arc and a short +X
/// arrow, Ball a ring, a dot and a thin cross, Parallel two white plates with a double arrow. Hovered or selected: a
/// ~32 px translucent white square behind it. The glyph and a little round it take the pointer.
pub struct MateGlyphs {
    pub items: Vec<(Frame, u32, bool, MateGlyphState, noshape_model::assembly::MateType)>,
}

/// A glyph's strokes in its own units (half its side = 1, b up), drawn into the frame's plane.
struct GlyphPen<'a> {
    lines: &'a mut LineData,
    points: &'a mut PointData,
    at: &'a dyn Fn(f64, f64) -> [f32; 3],
    /// The light blue (grey when inactive) and the outline (red when failing).
    blue: [u8; 4],
    ink: [u8; 4],
}

impl GlyphPen<'_> {
    fn seg(&mut self, a: (f64, f64), b: (f64, f64), color: [u8; 4], width: f32) {
        self.lines.segments.push(LineSegment { a: (self.at)(a.0, a.1), b: (self.at)(b.0, b.1), color, pick_id: 0, width, style: LineSegment::ON_TOP, dash_from: 0.0 });
    }
    fn poly(&mut self, pts: &[(f64, f64)], color: [u8; 4], width: f32) {
        for w in pts.windows(2) {
            self.seg(w[0], w[1], color, width);
        }
    }
    /// A light-blue stroke in a dark outline.
    fn stroke(&mut self, pts: &[(f64, f64)]) {
        self.poly(pts, self.ink, 6.0);
        self.poly(pts, self.blue, 4.0);
    }
    /// A filled quadrilateral (corners in order), by rows of strokes between its left and right sides.
    fn fill(&mut self, q: [(f64, f64); 4], color: [u8; 4]) {
        let lerp = |p: (f64, f64), r: (f64, f64), t: f64| (p.0 + (r.0 - p.0) * t, p.1 + (r.1 - p.1) * t);
        for k in 0..=12 {
            let t = f64::from(k) / 12.0;
            self.seg(lerp(q[0], q[3], t), lerp(q[1], q[2], t), color, 2.5);
        }
    }
    fn outline(&mut self, q: [(f64, f64); 4]) {
        self.poly(&[q[0], q[1], q[2], q[3], q[0]], self.ink, 1.2);
    }
    fn rect(&mut self, a0: f64, b0: f64, a1: f64, b1: f64, color: [u8; 4]) {
        let q = [(a0, b0), (a1, b0), (a1, b1), (a0, b1)];
        self.fill(q, color);
        self.outline(q);
    }
    /// An arrow from `from` to `to` with a filled head at `to`.
    fn arrow(&mut self, from: (f64, f64), to: (f64, f64)) {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let l = dx.hypot(dy).max(1e-9);
        let (u, n) = ((dx / l, dy / l), (-dy / l, dx / l));
        let base = (to.0 - u.0 * 0.32, to.1 - u.1 * 0.32);
        self.stroke(&[from, base]);
        self.head(base, u, n);
    }
    /// A filled arrowhead from `base` along `u` (`n` across it).
    fn head(&mut self, base: (f64, f64), u: (f64, f64), n: (f64, f64)) {
        let (l, r, tip) = ((base.0 + n.0 * 0.26, base.1 + n.1 * 0.26), (base.0 - n.0 * 0.26, base.1 - n.1 * 0.26), (base.0 + u.0 * 0.34, base.1 + u.1 * 0.34));
        self.poly(&[l, tip, r, l], self.ink, 1.6);
        self.fill([l, tip, tip, r], self.blue);
    }
    fn arc(&mut self, r: f64, from_deg: f64, to_deg: f64) -> Vec<(f64, f64)> {
        (0..=24).map(|k| (from_deg + (to_deg - from_deg) * f64::from(k) / 24.0).to_radians()).map(|t| (r * t.cos(), r * t.sin())).collect()
    }
    /// The ¾-turn arc arrow round the centre (Revolute, Cylindrical, Pin slot).
    fn turn(&mut self) {
        let pts = self.arc(0.55, 45.0, 230.0);
        self.stroke(&pts);
        let t = 230f64.to_radians();
        self.head(pts[pts.len() - 1], (-t.sin(), t.cos()), (-t.cos(), -t.sin()));
    }
    fn ring(&mut self, r: f64) {
        let pts = self.arc(r, 0.0, 360.0);
        self.poly(&pts, self.ink, 8.0);
        self.poly(&pts, self.blue, 6.0);
    }
    fn dot(&mut self) {
        let p = (self.at)(0.0, 0.0);
        self.points.points.push(PointSprite { position: p, color: self.ink, pick_id: 0, size: 8.0, style: LineSegment::ON_TOP, _pad: 0 });
        self.points.points.push(PointSprite { position: p, color: [255, 255, 255, 255], pick_id: 0, size: 6.0, style: LineSegment::ON_TOP, _pad: 0 });
    }
}

/// The glyphs' light blue (live2 mate-glyphs: rgb(152,204,234)).
const GLYPH_BLUE: [u8; 4] = [0x98, 0xcc, 0xea, 255];

impl MateGlyphs {
    fn geometry(&self, view: &noshape_render::camera_geometry::View) -> (LineData, PointData, Vec<MeshData>) {
        use noshape_model::assembly::MateType;
        let (mut ol, mut pts, mut picks) = (LineData::default(), PointData::default(), Vec::new());
        for (f, id, hl, st, ty) in &self.items {
            let Some(px) = view.world_per_px(f.origin) else { continue };
            let half = MATE_GLYPH_UNIT_PX * px;
            let (u, v, n) = (f.x(), f.y(), f.z());
            let lift = n * (half * 0.02);
            let at = |a: f64, b: f64| -> [f32; 3] { (f.origin + lift + u * (a * half) + v * (b * half)).as_vec3().into() };
            let blue = if *st == MateGlyphState::Inactive { [0xc8, 0xc8, 0xc8, 255] } else { GLYPH_BLUE };
            let ink = if *st == MateGlyphState::Error { theme::rgb8(theme::viewport::ERROR) } else { DARK };
            let mut pen = GlyphPen { lines: &mut ol, points: &mut pts, at: &at, blue, ink };
            const WHITE: [u8; 4] = [255, 255, 255, 255];
            if *hl {
                // A ~32 px translucent white square behind it (live2: 45 %).
                let h = 16.0 / MATE_GLYPH_UNIT_PX;
                pen.fill([(-h, -h), (h, -h), (h, h), (-h, h)], [255, 255, 255, 115]);
            }
            match ty {
                MateType::Fastened => {
                    pen.fill([(-0.62, -0.62), (0.62, -0.62), (0.62, 0.0), (-0.62, 0.0)], pen.blue);
                    pen.fill([(-0.62, 0.0), (0.62, 0.0), (0.62, 0.62), (-0.62, 0.62)], WHITE);
                    pen.fill([(-0.33, -0.37), (0.33, -0.37), (0.33, 0.0), (-0.33, 0.0)], WHITE);
                    pen.outline([(-0.62, -0.62), (0.62, -0.62), (0.62, 0.62), (-0.62, 0.62)]);
                    pen.poly(&[(-0.62, 0.0), (-0.33, 0.0), (-0.33, -0.37), (0.33, -0.37), (0.33, 0.0), (0.62, 0.0)], DARK, 1.2);
                }
                MateType::Revolute | MateType::Cylindrical => {
                    pen.turn();
                    pen.dot();
                }
                MateType::PinSlot => {
                    pen.turn();
                    pen.arrow((0.15, 0.0), (0.8, 0.0));
                    pen.dot();
                }
                MateType::Slider => {
                    pen.ring(0.6);
                    pen.dot();
                }
                MateType::Ball => {
                    pen.ring(0.6);
                    pen.poly(&[(-0.6, 0.0), (0.6, 0.0)], pen.ink, 1.0);
                    pen.poly(&[(0.0, -0.6), (0.0, 0.6)], pen.ink, 1.0);
                    pen.dot();
                }
                MateType::Planar => {
                    pen.rect(-0.6, -0.65, 0.05, 0.0, WHITE);
                    pen.arrow((-0.25, -0.3), (-0.25, 0.6));
                    pen.arrow((-0.25, -0.3), (0.7, -0.3));
                }
                MateType::Parallel => {
                    for dy in [0.0, -0.95] {
                        let q = [(-0.75, 0.25 + dy), (0.3, 0.25 + dy), (0.75, 0.65 + dy), (-0.3, 0.65 + dy)];
                        pen.fill(q, WHITE);
                        pen.outline(q);
                    }
                    pen.arrow((0.0, -0.15), (0.0, 0.5));
                    pen.arrow((0.0, -0.15), (0.0, -0.8));
                }
                _ => {
                    pen.ring(0.6);
                    pen.dot();
                }
            }
            // A disc round it in its plane, both sides (the connector's Z may face away from the viewer).
            let r = MATE_GLYPH_PICK_PX / MATE_GLYPH_UNIT_PX;
            let mut positions = vec![at(0.0, 0.0)];
            let mut indices = Vec::new();
            for k in 0..32u32 {
                let t = f64::from(k) / 32.0 * std::f64::consts::TAU;
                positions.push(at(r * t.cos(), r * t.sin()));
                let (a, b) = (k + 1, (k + 1) % 32 + 1);
                indices.extend([0, a, b, 0, b, a]);
            }
            picks.push(MeshData { normals: vec![n.as_vec3().into(); positions.len()], pick_ids: vec![*id; positions.len()], indices, positions });
        }
        (ol, pts, picks)
    }
}

impl noshape_render::camera_geometry::CameraGeometry for MateGlyphs {
    fn key(&self) -> Option<u64> {
        Some(noshape_render::camera_geometry::key_of("MateGlyphs", &self.items))
    }

    fn build(&self, view: &noshape_render::camera_geometry::View, up: &Uploader) -> Vec<Instance> {
        let (lines, points, picks) = self.geometry(view);
        if lines.segments.is_empty() {
            return Vec::new();
        }
        let mut out: Vec<Instance> = picks.iter().map(|m| Instance::new(Drawable::Mesh(up.upload_mesh(m))).pick_only().ignore_clip().on_top().layer(20)).collect();
        out.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(20));
        out.push(Instance::new(Drawable::Points(up.upload_points(&points))).per_primitive_color().unpickable().ignore_clip().layer(21));
        out
    }
}

/// The ground glyph on a fixed instance's face while it is dragged (live asm/drag-feel-1122 fixed-glyph-x3.png, BaseR_hold
/// unprojected onto the face): lying in the face's plane at `frame` (z the face's normal toward the viewer, x the face's
/// own x), from 8 px past the pointer along x: a white box outlined dark (26 x 12 px in the plane) over a band of
/// diagonal ground strokes, haloed white.
pub struct GroundGlyph {
    pub frame: Frame,
}

impl GroundGlyph {
    fn geometry(&self, view: &noshape_render::camera_geometry::View) -> (LineData, PointData) {
        let (mut lines, mut points) = (LineData::default(), PointData::default());
        let f = &self.frame;
        let Some(px) = view.world_per_px(f.origin) else { return (lines, points) };
        let (u, v, n) = (f.x(), f.y(), f.z());
        let lift = n * (0.3 * px);
        let at = |a: f64, b: f64| -> [f32; 3] { (f.origin + lift + u * (a * px) + v * (b * px)).as_vec3().into() };
        const WHITE: [u8; 4] = [255, 255, 255, 255];
        let mut pen = GlyphPen { lines: &mut lines, points: &mut points, at: &at, blue: WHITE, ink: DARK };
        pen.fill([(8.5, -1.5), (37.5, -1.5), (37.5, 13.5), (8.5, 13.5)], WHITE);
        let strokes: Vec<[(f64, f64); 2]> = [14.0, 19.0, 24.0, 29.0, 34.0].iter().map(|&x| [(x, 0.0), (x - 4.0, -10.0)]).collect();
        for s in &strokes {
            pen.poly(s, WHITE, 3.5);
        }
        for s in &strokes {
            pen.poly(s, DARK, 1.4);
        }
        pen.outline([(10.0, 0.0), (36.0, 0.0), (36.0, 12.0), (10.0, 12.0)]);
        // Smooth edges as live's (qa FEEL-ASM-FIXED-GLYPH-AA).
        for s in &mut lines.segments {
            s.style |= LineSegment::SMOOTH;
        }
        (lines, points)
    }
}

impl noshape_render::camera_geometry::CameraGeometry for GroundGlyph {
    fn key(&self) -> Option<u64> {
        Some(noshape_render::camera_geometry::key_of("GroundGlyph", &self.frame))
    }

    fn build(&self, view: &noshape_render::camera_geometry::View, up: &Uploader) -> Vec<Instance> {
        let (lines, _) = self.geometry(view);
        if lines.segments.is_empty() {
            return Vec::new();
        }
        vec![Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(20)]
    }
}

fn push_triad(lines: &mut LineData, f: &Frame, size: f64, width: f32, pick: u32, highlight: bool) {
    let o: [f32; 3] = f.origin.as_vec3().into();
    for (k, axis) in [f.x(), f.y(), f.z()].into_iter().enumerate() {
        let tip: [f32; 3] = (f.origin + axis * size).as_vec3().into();
        let c = if highlight { HANDLE_HL } else { axis_colors()[k] };
        lines.segments.push(LineSegment { a: o, b: tip, color: c, pick_id: pick, width, style: LineSegment::ON_TOP, dash_from: 0.0 });
    }
}

pub fn with_highlights(built: &AsmBuilt, result: &AssemblyResult, h: &Highlights, up: &Uploader) -> Scene {
    let mut scene = Scene { instances: built.scene.instances[..built.static_instances].to_vec(), states: vec![0; built.picks.len() + 1], clip: built.scene.clip, clip_extra: built.scene.clip_extra.clone(), clip_selected: built.scene.clip_selected, clip_hovered: [false; 3], compare: built.scene.compare, preview_mix: built.scene.preview_mix, section_interference: built.scene.section_interference, hover_outline: false, hover_outline_rgb: None, body_outline: false, body_outline_rgb: None, no_occlusion: built.scene.no_occlusion };
    for top in &h.accent_tops {
        for &id in built.picks.faces_of(*top) {
            scene.add_flag(id, state::ACCENT);
        }
    }
    for top in &h.hover_tops {
        for &id in built.picks.faces_of(*top) {
            scene.add_flag(id, state::HOVER | state::OUTLINE);
            scene.hover_outline = true;
        }
    }
    // A selected (or just inserted) instance: Onshape's whole-part amber,
    // lighter than a selected face, outlined in light amber; its edges go
    // dark amber (live ins-902, qa5 A15).
    // Its outline shows through the parts in front (live2 11-14, explode edit mode too).
    for span in built.spans.iter().filter(|s| h.selected_tops.contains(&s.top)) {
        for inst in &mut scene.instances[span.start..span.end.min(built.static_instances)] {
            inst.outline_through = true;
        }
    }
    // An instance selected through a face, edge or vertex: a #ffa033 band over its silhouette line, 2 px, through the
    // parts in front, its fill its own (live triad-depth-1122 t1, t2, e1: the part grey with an orange outline; hover
    // matrix batch 4).
    for (p, _) in &h.selected_entities {
        if h.entities_only || h.selected_tops.contains(&p[0]) || h.mc_owner.as_ref().is_some_and(|o| o[0] == p[0]) {
            continue;
        }
        for &id in built.picks.faces_of(p[0]) {
            scene.add_flag(id, state::BAND | state::INSTANCE);
            scene.body_outline = true;
        }
    }
    for top in &h.selected_tops {
        for &id in built.picks.faces_of(*top) {
            scene.add_flag(id, state::SELECTED | state::BODY | state::OUTLINE);
        }
        for id in built.picks.edges_of(*top) {
            scene.add_flag(id, state::BODY);
        }
    }
    let mut pts = PointData::default();
    // A selected vertex's disc over the move manipulator, as its edge's band (live triad-depth-1122 t6).
    let mut sel_pts = PointData::default();
    let sel = theme::lin(theme::viewport::SELECTION);
    let hov = theme::lin(theme::viewport::PREHIGHLIGHT);
    let mark = |path: &InstancePath, r: &Reference, flag: u32, scene: &mut Scene, pts: &mut PointData, sel_pts: &mut PointData| {
        for id in built.picks.ids_for(path, r) {
            scene.add_flag(id, flag);
            if let Some(t) = built.picks.get(id)
                && t.kind == PickKind::Vertex
                && let (Some(a), Some(o)) = (t.anchor, result.occurrences.get(t.occ))
            {
                let w = o.transform.transform_point3(a);
                // As the Part Studio's (qa FEEL-ASM-VERTEX-LOOK).
                let disc = crate::builder::vertex_disc(w.as_vec3().into(), lin_to_u8(if flag == state::HOVER { hov } else { sel }));
                // Under a mate field's corner mark: depth-tested, so the mark (nearer) covers its middle.
                let under = flag == state::HOVER && h.hover_vertex_open;
                let out = if flag & state::SELECTED != 0 { &mut *sel_pts } else { &mut *pts };
                out.points.extend(disc.map(|p| PointSprite { style: if under { p.style & !LineSegment::ON_TOP } else { p.style }, ..p }));
            }
        }
    };
    for (p, r) in &h.selected_entities {
        mark(p, r, state::SELECTED, &mut scene, &mut pts, &mut sel_pts);
    }
    // A selected face's boundary: a 2 device px #ffa90a line, as a Part Studio's (live triad-depth-1122 t1).
    for (p, r) in &h.selected_entities {
        for id in built.picks.boundary_ids(p, r) {
            let own = scene.states.get(id as usize).is_some_and(|s| s & (state::HOVER | state::SELECTED | state::ERROR) != 0);
            scene.add_flag(id, if own { state::SELECTED } else { state::SELECTED | state::OUTLINE });
        }
    }
    if let Some((p, r)) = &h.hover_entity {
        mark(p, r, state::HOVER, &mut scene, &mut pts, &mut sel_pts);
    }
    for (p, r) in &h.query_entities {
        for id in built.picks.ids_for(p, r) {
            scene.add_flag(id, state::QUERY);
        }
    }
    // A face whose boundary line the picks' band leaves alone.
    let line_faces = |(p, r): &(InstancePath, Reference), scene: &mut Scene| {
        for id in built.picks.ids_for(p, r) {
            scene.add_flag(id, state::LINED);
        }
    };
    for p in &h.picked_parts {
        for id in built.picks.faces_at(p) {
            scene.add_flag(id, state::BAND | state::PICKED);
            scene.body_outline = true;
        }
    }
    // (A pick under the still pointer, plain with the hover look, is left out of these by the view: live workflows-1122
    // 08 s06 / s07; re-hovered later it stays yellow, mc-infer-1122 picked-asm.)
    for e @ (p, r) in &h.picked_entities {
        mark(p, r, state::SELECTED | state::PICKED, &mut scene, &mut pts, &mut sel_pts);
        for id in built.picks.boundary_ids(p, r) {
            scene.add_flag(id, state::HOVER | state::OUTLINE | state::PICKED);
        }
        if !h.picked_parts.is_empty() {
            line_faces(e, &mut scene);
        }
    }
    // A curved face's whole image outlined, its hidden part too: a cylinder side's two silhouettes and its bottom rim, the
    // top rim's front arc left dark (live mc-infer-1122 asm-cyl10, qa FEEL-MC-ASM-CYL-SIDE-OUTLINE). Its boundary lines
    // when the hover band already outlines a part (one mask) or the face is selected.
    let banded = |(p, r): &(InstancePath, Reference), scene: &Scene| {
        h.hover_outline_image && !scene.hover_outline && h.hover_part.is_none() && h.crossing_tops.is_empty() && !built.picks.ids_for(p, r).any(|id| scene.states.get(id as usize).is_some_and(|s| s & state::SELECTED != 0))
    };
    let face_band = h.hover_outline.as_ref().is_some_and(|e| banded(e, &scene));
    if let Some(e @ (p, r)) = h.hover_outline.as_ref().filter(|_| face_band) {
        if !h.picked_parts.is_empty() {
            line_faces(e, &mut scene);
        }
        for id in built.picks.ids_for(p, r) {
            scene.add_flag(id, state::HOVER | state::OUTLINE);
        }
        scene.hover_outline = true;
    } else if let Some(e @ (p, r)) = &h.hover_outline {
        // A solid 2 device px line, not the band of an edge hovered itself (live2 asm-dialogs-1119 hz: outlined once).
        if !h.picked_parts.is_empty() {
            line_faces(e, &mut scene);
        }
        for id in built.picks.boundary_ids(p, r) {
            let own = scene.states.get(id as usize).is_some_and(|s| s & (state::HOVER | state::SELECTED | state::ERROR) != 0);
            scene.add_flag(id, if own { state::HOVER } else { state::HOVER | state::OUTLINE });
        }
    }
    // The silhouette in #FFC20E, the hovered face's edges in the hover colour (live2 asm-feel2 (c)).
    let crossing: Vec<InstancePath> = h.crossing_tops.iter().map(|t| vec![*t]).collect();
    for p in h.hover_part.iter().chain(&crossing) {
        for id in built.picks.faces_at(p) {
            scene.add_flag(id, state::HOVER | state::OUTLINE);
            scene.hover_outline = true;
            scene.hover_outline_rgb = Some([0xff, 0xc2, 0x0e]);
        }
    }
    // A part's own mate connector hovered or selected: the part banded as a mate pick's, (242,195,108) at 85 %.
    if let Some(p) = h.mc_owner.as_ref().filter(|_| !scene.hover_outline || face_band) {
        for id in built.picks.faces_at(p) {
            scene.add_flag(id, state::BAND | state::PICKED);
            scene.body_outline = true;
        }
    }
    for d in &h.dots {
        pts.points.push(PointSprite { position: d.as_vec3().into(), color: DARK, pick_id: 0, size: 6.0, style: LineSegment::ON_TOP, _pad: 0 });
    }
    let mut lines = LineData::default();
    for t in &h.triads {
        let w = if t.strong { 2.5 } else { 1.5 };
        push_triad(&mut lines, &t.frame, t.size, w, 0, false);
        pts.points.push(PointSprite { position: t.frame.origin.as_vec3().into(), color: DARK, pick_id: 0, size: if t.strong { 7.0 } else { 5.0 }, style: LineSegment::ON_TOP, _pad: 0 });
    }
    for (a, b, strong) in &h.mate_lines {
        lines.segments.push(LineSegment { a: a.as_vec3().into(), b: b.as_vec3().into(), color: theme::rgb8(theme::viewport::SELECTION), pick_id: 0, width: if *strong { 2.0 } else { 1.2 }, style: LineSegment::ON_TOP, dash_from: 0.0 });
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
            lines.segments.push(LineSegment { a: at(a0).as_vec3().into(), b: at(a1).as_vec3().into(), color: blue, pick_id: 0, width: 1.5, style: LineSegment::DASHED | LineSegment::ON_TOP, dash_from: 0.0 });
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
                    lines.segments.push(LineSegment { a: tip.as_vec3().into(), b: base.as_vec3().into(), color: blue, pick_id: 0, width: 2.0, style: LineSegment::ON_TOP, dash_from: 0.0 });
                }
            }
        }
    }
    if let Some(dim) = h.drag_dim {
        let blue = [26, 53, 184, 255];
        let seg = |lines: &mut LineData, a: DVec3, b: DVec3| lines.segments.push(LineSegment { a: a.as_vec3().into(), b: b.as_vec3().into(), color: blue, pick_id: 0, width: 2.0, style: LineSegment::ON_TOP, dash_from: 0.0 });
        match dim {
            DragDim::Line { start, end, tick } => {
                seg(&mut lines, start, end);
                seg(&mut lines, start - tick, start + tick);
            }
            DragDim::Arc { center, from, axis, sweep, tick } => {
                let n = ((sweep.abs() / std::f64::consts::TAU * 64.0).ceil() as usize).max(1);
                let at = |t: f64| center + glam::DQuat::from_axis_angle(axis, sweep * t) * (from - center);
                for i in 0..n {
                    seg(&mut lines, at(i as f64 / n as f64), at((i + 1) as f64 / n as f64));
                }
                seg(&mut lines, from - tick, from + tick);
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
            lines.segments.push(LineSegment { a: at(t0).as_vec3().into(), b: at(t1).as_vec3().into(), color: blue, pick_id: 0, width: 1.5, style: LineSegment::DASHED | LineSegment::ON_TOP, dash_from: 0.0 });
        }
        let dir = (l.b - l.a).normalize_or_zero();
        if dir != DVec3::ZERO {
            let side = l.up.normalize_or_zero();
            for (tip, out) in [(l.a, -dir), (l.b, dir)] {
                for s in [1.0, -1.0] {
                    let base = tip - out * l.head + side * (s * l.head * 0.4);
                    lines.segments.push(LineSegment { a: tip.as_vec3().into(), b: base.as_vec3().into(), color: blue, pick_id: 0, width: 2.0, style: LineSegment::ON_TOP, dash_from: 0.0 });
                }
            }
        }
    }
    // Interference detection: the hovered overlap's box (live analysis/83:
    // the query-hover yellow, ~6 px).
    for (lo, hi) in &h.boxes {
        let c = |k: usize| DVec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z });
        for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
            lines.segments.push(LineSegment { a: c(a).as_vec3().into(), b: c(b).as_vec3().into(), color: [242, 255, 128, 255], pick_id: 0, width: 6.0, style: LineSegment::ON_TOP, dash_from: 0.0 });
        }
    }
    for (a, b) in &h.trails {
        lines.segments.push(LineSegment { a: a.as_vec3().into(), b: b.as_vec3().into(), color: DARK, pick_id: 0, width: 1.5, style: LineSegment::EXPLODE_LINE | LineSegment::ON_TOP, dash_from: 0.0 });
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
        if h.origin_triad {
            push_triad(&mut ol, &Frame::default(), size, 1.5, id, hl);
            op.points.push(PointSprite { position: [0.0; 3], color: if hl { HANDLE_HL } else { DARK }, pick_id: id, size: 9.0, style: LineSegment::ON_TOP, _pad: 0 });
        } else {
            // Live keys-1118: the assembly's origin is the Part Studio's 11 px ring with a centre dot (point style 4).
            let [disc, ring, _] = theme::viewport::POINT_HOVER;
            // (qa FEEL-ASM-ORIGIN-ROW-HOVER, live picture a03: an 18 px disc under the ring in the hover's orange.)
            if h.origin_row_hover {
                op.points.push(PointSprite { position: [0.0; 3], color: theme::rgb8(disc), pick_id: id, size: 18.0, style: LineSegment::ON_TOP, _pad: 0 });
            }
            let color = if hl { HANDLE_HL } else if h.origin_row_hover { theme::rgb8(ring) } else { theme::rgb8(theme::pick(theme::viewport::ORIGIN, theme::viewport_dark::ORIGIN)) };
            op.points.push(PointSprite { position: [0.0; 3], color, pick_id: id, size: 11.0, style: 4 | LineSegment::ON_TOP, _pad: 0 });
        }
        scene.set_state(id, 0);
    }
    // Assembly mate connectors: Onshape's screen-size disc (`connector_glyphs`), drawn and picked as camera geometry
    // by the view; their pick ids here.
    for i in 0..h.connectors.len() {
        scene.set_state(overlay_id(built, OverlayPick::Connector(i)), 0);
    }
    // Mate glyphs: drawn and picked at screen size by the view ([`MateGlyphs`]); their pick ids here.
    for i in 0..h.mate_glyphs.len() {
        scene.set_state(overlay_id(built, OverlayPick::Mate(i)), 0);
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
        const CENTER_HOVER: [u8; 4] = [0x3a, 0x6b, 0xa8, 255];
        const CENTER_HOVER_FLAT: [u8; 4] = [0x60, 0x87, 0xbd, 255];
        const EXPLODE_CORE: [u8; 4] = [0xcd, 0xd0, 0xd3, 255];
        // A hovered explode handle keeps its hollow stroke, slate edged with a blue core (live workflow 24 s04).
        const EXPLODE_HOVER: ([u8; 4], [u8; 4]) = ([92, 114, 131, 255], [117, 152, 200, 255]);
        // While a handle is dragged the others fade to a faint grey (live manip-feel-1118 m9).
        const FAINT: u8 = 60;
        let size = *size;
        // Explode mode (live 802): a 2 px #cdd0d3 core between the grey edges.
        let (core, core_w) = if h.explode_triad { (EXPLODE_CORE, 2.0) } else { (CORE, 1.6) };
        let (edge, core) = if h.manip_dragging { ([EDGE[0], EDGE[1], EDGE[2], FAINT], [core[0], core[1], core[2], FAINT]) } else { (EDGE, core) };
        let p3 = |p: DVec3| -> [f32; 3] { p.as_vec3().into() };
        let stroke = |ol: &mut LineData, pts: &[DVec3], closed: bool, id: u32, hl: bool| {
            let n = pts.len();
            let segs = if closed { n } else { n.saturating_sub(1) };
            let (hover_edge, hover_core) = if h.explode_triad { EXPLODE_HOVER } else { (HOVER, HOVER) };
            for (w, c) in [(4.0, if hl { hover_edge } else { edge }), (core_w, if hl { hover_core } else { core })] {
                for i in 0..segs {
                    ol.segments.push(LineSegment { a: p3(pts[i]), b: p3(pts[(i + 1) % n]), color: c, pick_id: id, width: w, style: LineSegment::ON_TOP, dash_from: 0.0 });
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
            // (The shaft starts outside the centre ring, 9 px foreshortened: none of its core in the ring's hole, live
            // hover-matrix b4-as-face-selhover_f.)
            let (r0, base, tip) = (size * 0.11, size * 0.62, size * 0.86);
            stroke(&mut ol, &[f.origin + axis * r0, f.origin + axis * base], false, id, hl);
            let w = size * 0.07;
            stroke(&mut ol, &[f.origin + axis * base + side * w, f.origin + axis * tip, f.origin + axis * base - side * w], true, id, hl);
            scene.set_state(id, 0);
            // Rotation handle at the tip: a small ring in the plane of this axis and the next (it turns about the third,
            // ManipHandle::ring_axis) and a tick across it.
            let rid = overlay_id(built, OverlayPick::Manip(ManipHandle::Ring(k)));
            let rhl = *hovered == Some(ManipHandle::Ring(k));
            let rc = f.origin + axis * (tip + size * 0.08);
            stroke(&mut ol, &circle(rc, axis, side, size * 0.08), true, rid, rhl);
            stroke(&mut ol, &[rc - side * (size * 0.20), rc + side * (size * 0.20)], false, rid, rhl);
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
            let fill = if phl { [HOVER[0], HOVER[1], HOVER[2], 70] } else if h.manip_dragging { [0xf7, 0xf7, 0xf8, 15] } else { [0xf7, 0xf7, 0xf8, 40] };
            for i in 1..6 {
                let y = a0 + (a1 - a0) * i as f64 / 6.0;
                ol.segments.push(LineSegment { a: p3(q(a0, y)), b: p3(q(a1, y)), color: fill, pick_id: pid, width: 4.0, style: LineSegment::ON_TOP, dash_from: 0.0 });
            }
            scene.set_state(pid, 0);
        }
        // Center ring (moves the triad itself).
        let cid = overlay_id(built, OverlayPick::Manip(ManipHandle::Center));
        let chl = *hovered == Some(ManipHandle::Center);
        let o: [f32; 3] = p3(f.origin);
        // Live hover-matrix-1121 as2 (qa FEEL-ASM-MANIPULATOR-RING): a hollow ring, ~18 px, the pick showing through
        // its ~12 px middle; hovered a 20 px blue stroke, #3a6ba8 rims round a #99b2da core (qa FEEL-ASM-MOVE-RING-RIMS).
        // On a selected mate connector, hovered, the 18 px ring stays one #6087bd, the connector's blue through its
        // middle (live as-mc-selhover: 144 px of #0101d0; qa 7e58aaae8).
        let on_connector = h.connectors.iter().any(|(c, _, look, _)| {
            matches!(look, crate::builder::ConnectorLook::Selected | crate::builder::ConnectorLook::Hovered) && c.origin.distance(f.origin) < 1e-9
        });
        let (color, style, ring_px) = match (chl, on_connector) {
            (true, true) => (CENTER_HOVER_FLAT, PointSprite::STROKE_RING | PointSprite::FLAT_RING, 18.0),
            (true, false) => (CENTER_HOVER, PointSprite::STROKE_RING | PointSprite::SOFT_CORE, 20.0),
            _ => (edge, PointSprite::STROKE_RING, 18.0),
        };
        op.points.push(PointSprite { position: o, color, pick_id: cid, size: ring_px, style: LineSegment::ON_TOP | style, _pad: 0 });
        // Over the selected edges' band (below): idle it shows through a fifth, hovered it lies over the band (live
        // as2-edge-selected_s / selhover_f, qa FEEL-ASM-RING-OVER-BAND).
        if !h.selected_entities.is_empty() {
            let a = if chl { 255 } else { 51 };
            sel_pts.points.push(PointSprite { position: o, color: [color[0], color[1], color[2], a], pick_id: 0, size: ring_px, style: LineSegment::ON_TOP | style, _pad: 0 });
        }
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
                    ol.segments.push(LineSegment { a: p3(pts[i]), b: p3(pts[(i + 1) % n]), color: c, pick_id: 0, width: w, style: LineSegment::ON_TOP, dash_from: 0.0 });
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
        push_section_glyph(&mut ol, built, &g, d);
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
    // The selected edges' band over the move manipulator: it hides the shaft along the edge and crosses the centre
    // ring and arrowhead (live triad-depth-1122 e1; the triad itself draws over the parts, t2).
    if h.manipulator.is_some() {
        let tops: Vec<_> = h.selected_entities.iter().map(|(p, _)| p[0]).collect();
        for span in built.spans.iter().filter(|s| tops.contains(&s.top)) {
            let edges = scene.instances[span.start..span.end.min(built.static_instances)]
                .iter()
                .filter(|i| i.pickable && !i.pick_only && !i.back_silhouette && matches!(i.drawable, Drawable::Lines(_)))
                .map(|i| i.clone().unpickable().selected_only().on_top().over_lines())
                .collect::<Vec<_>>();
            scene.instances.extend(edges);
        }
    }
    if !sel_pts.points.is_empty() {
        scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&sel_pts))).per_primitive_color().unpickable().ignore_clip().over_lines().layer(24));
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
            lines.segments.push(LineSegment { a: at(rad, a0), b: at(rad, a1), color, pick_id: 0, width, style: LineSegment::ON_TOP, dash_from: 0.0 });
        }
    };
    // Filled with concentric strokes (meshes can't draw over the parts).
    let rings = 6;
    for k in 1..=rings {
        arc(lines, r * k as f64 / rings as f64 * 0.95, FILL, 3.0);
    }
    arc(lines, r, RIM, 1.5);
    for a in [gap0, gap1] {
        lines.segments.push(LineSegment { a: at(0.0, a), b: at(r, a), color: RIM, pick_id: 0, width: 1.5, style: LineSegment::ON_TOP, dash_from: 0.0 });
    }
    let o = f.origin;
    let seg = |lines: &mut LineData, a: DVec3, b: DVec3, color: [u8; 4]| lines.segments.push(LineSegment { a: a.as_vec3().into(), b: b.as_vec3().into(), color, pick_id: 0, width: 2.5, style: LineSegment::ON_TOP, dash_from: 0.0 });
    seg(lines, o, o + n * (r * 1.3), [0, 0, 208, 255]);
    seg(lines, o + u * (r * 1.25), o + u * (r * 1.9), [208, 24, 24, 255]);
    seg(lines, o + v * (r * 1.25), o + v * (r * 1.9), [24, 168, 24, 255]);
}

/// Live's section manipulator (assembly/2368–2370): idle strokes light grey
/// (core ≈ 230,232,233, edge ≈ 194,199,203); a hovered handle blue (≈ 70,123,177);
/// the dragged one deeper blue (≈ 40,97,182). The base circle faces the
/// viewer; the shaft and hollow head; two small rings at the tip, each with
/// its axis line (ring 0 about the plane's Y, ring 1 about its X).
fn push_section_glyph(ol: &mut LineData, built: &AsmBuilt, g: &SectionGlyph, d: DVec3) {
    const CORE: [u8; 4] = [230, 232, 233, 255];
    const EDGE: [u8; 4] = [194, 199, 203, 255];
    const HOVER: [u8; 4] = [70, 123, 177, 255];
    const DRAG: [u8; 4] = [40, 97, 182, 255];
    let p3 = |p: DVec3| -> [f32; 3] { p.as_vec3().into() };
    let w = g.wpp;
    let side = d.cross(g.view).try_normalize().unwrap_or_else(|| d.any_orthonormal_vector());
    let up = g.view.cross(side).try_normalize().unwrap_or(d);
    let stroke = |ol: &mut LineData, handle: SectionHandle, pts: &[DVec3], closed: bool| {
        let pick_id = overlay_id(built, OverlayPick::Section(handle));
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
                ol.segments.push(LineSegment { a: p3(pts[i]), b: p3(pts[(i + 1) % n]), color: *c, pick_id, width: *wd, style: LineSegment::ON_TOP, dash_from: 0.0 });
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
        let mut mark = |a: DVec3, b: DVec3| ol.segments.push(LineSegment { a: p3(a), b: p3(b), color: DRAG, pick_id: 0, width: 1.5, style: LineSegment::ON_TOP, dash_from: 0.0 });
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

#[cfg(test)]
mod screen_triad_tests {
    use super::*;
    use noshape_render::camera_geometry::View;

    /// Live mates.json: a mate's axes are 50 px long at the pixel size where they stand, whatever the zoom (shorter on
    /// screen only as they turn away, as the connector glyph's).
    #[test]
    fn mate_axes_stay_50_px_at_any_zoom() {
        let t = ScreenTriads { items: vec![(Frame::default(), MATE_AXIS_PX, false)] };
        for distance in [0.1, 2.0] {
            let camera = noshape_render::Camera { distance, ..Default::default() };
            let view = View { camera, size: glam::DVec2::new(1200.0, 800.0) };
            let (lines, _) = t.geometry(&view);
            let px = view.world_per_px(DVec3::ZERO).unwrap();
            for seg in &lines.segments {
                let len = glam::Vec3::from(seg.a).distance(glam::Vec3::from(seg.b)) as f64;
                assert!((len / px - 50.0).abs() < 0.01, "{} px at distance {distance}", len / px);
            }
        }
    }
}

#[cfg(test)]
mod display_level_tests {
    use super::*;
    use noshape_model::assembly::InstanceId;
    use noshape_model::configuration::ConfigValues;
    use noshape_model::document::TabId;
    use noshape_model::reference::{BodyId, FeatureId};

    /// A plate's top as `n` × `n` squares: 2 n² triangles in one face.
    fn grid(n: usize) -> MeshBuffers {
        let mut m = MeshBuffers::default();
        for j in 0..=n {
            for i in 0..=n {
                m.positions.push([i as f32, j as f32, 0.0]);
                m.normals.push([0.0, 0.0, 1.0]);
            }
        }
        let at = |i: usize, j: usize| (j * (n + 1) + i) as u32;
        for j in 0..n {
            for i in 0..n {
                m.indices.extend([at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
            }
        }
        m.face_ranges.push((0, m.indices.len() as u32));
        m
    }

    fn occurrence(coarse: MeshBuffers) -> PartOccurrence {
        let coarse = Arc::new(coarse);
        PartOccurrence {
            path: vec![InstanceId(1)],
            body: BodyId { feature: FeatureId(1), index: 0 },
            studio: TabId(1),
            config: ConfigValues::default(),
            instance_name: "Part 1 <1>".into(),
            body_name: "Part 1".into(),
            kind: BodyKind::Solid,
            color: [0.5; 4],
            mesh: Arc::new(grid(20)),
            coarser: vec![coarse],
            levels_from: 0,
            inner: DMat4::IDENTITY,
            transform: DMat4::IDENTITY,
            face_refs: Vec::new(),
            edge_refs: Vec::new(),
            vertex_refs: Vec::new(),
            hidden: false,
            local_bounds: None,
            context: Default::default(),
            mirrored: false,
            external: None,
            flex: None,
        }
    }

    /// HEAVY-ASM-SOFTWARE-FRAME: on a software rasteriser a dense body's coarsest level is its proxy (asm-xdev-subs'
    /// 10 482-face part: 72 k triangles at Coarse), on a GPU its own Coarse mesh; a light body keeps its own on both.
    #[test]
    fn a_software_scene_draws_a_dense_bodys_proxy_at_its_coarsest() {
        // (Its 1-unit squares under a cell of its 400-unit side's diagonal / 200, 2.8: they merge.)
        let dense = occurrence(grid(400));
        let tris = |m: &Arc<MeshBuffers>| m.indices.len() / 3;
        let gpu = display_levels(&dense, false);
        assert!(gpu.len() == 2 && Arc::ptr_eq(&gpu[0], &dense.coarser[0]) && Arc::ptr_eq(&gpu[1], &dense.mesh));
        let software = display_levels(&dense, true);
        assert_eq!(software.len(), 2);
        assert!(tris(&software[0]) * 2 <= tris(&dense.coarser[0]), "{} triangles of {}", tris(&software[0]), tris(&dense.coarser[0]));
        assert!(Arc::ptr_eq(&software[1], &dense.mesh), "the finest stays");
        let light = occurrence(grid(10));
        assert!(Arc::ptr_eq(&display_levels(&light, true)[0], &light.coarser[0]));
    }
}
