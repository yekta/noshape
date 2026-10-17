//! Render scene for an evaluated assembly: every part occurrence is drawn
//! from GPU buffers cached per `Arc<MeshBuffers>` with its own instance
//! transform, so moving instances (drag, animation, explode) never
//! re-uploads geometry. Pick ids map back to (instance path, reference).

use crate::scene_builder::PickKind;
use crate::theme;
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
    pub fn faces_of(&self, top: noshape_model::assembly::InstanceId) -> &[u32] {
        self.by_top.get(&top).map(|v| v.as_slice()).unwrap_or(&[])
    }
    pub fn ids_for(&self, path: &InstancePath, r: &Reference) -> &[u32] {
        self.by_ref.get(&(path.clone(), r.clone())).map(|v| v.as_slice()).unwrap_or(&[])
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

#[derive(Default)]
pub struct AsmGpuCache {
    bodies: HashMap<usize, (Weak<MeshBuffers>, Arc<CachedBody>)>,
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
}

fn to_mat4(m: &DMat4) -> Mat4 {
    m.as_mat4()
}

pub fn build(result: &AssemblyResult, opts: &AsmSceneOptions, cache: &mut AsmGpuCache, up: &Uploader) -> AsmBuilt {
    let mut scene = Scene::default();
    let mut picks = AsmPickMap::default();
    cache.bodies.retain(|_, (w, _)| w.strong_count() > 0);
    for (oi, o) in result.occurrences.iter().enumerate() {
        if o.hidden {
            continue;
        }
        let key = Arc::as_ptr(&o.mesh) as usize;
        let cached = match cache.bodies.get(&key) {
            Some((_, c)) => c.clone(),
            None => {
                let c = Arc::new(upload_body(&o.mesh, up));
                cache.bodies.insert(key, (Arc::downgrade(&o.mesh), c.clone()));
                c
            }
        };
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
        let mut color = o.color;
        let dimmed = opts.isolate.as_ref().is_some_and(|s| !s.contains(&o.top()));
        if dimmed {
            color[3] = 0.12;
        } else if o.kind == BodyKind::Surface {
            color[3] = 0.85;
        }
        if opts.error_tops.contains(&o.top()) {
            let e = theme::lin(0xff5c5c);
            color = [color[0] * 0.4 + e[0] * 0.6, color[1] * 0.4 + e[1] * 0.6, color[2] * 0.4 + e[2] * 0.6, color[3]];
        }
        if o.kind != BodyKind::Wire {
            let mut inst = Instance::new(Drawable::Mesh(cached.mesh.clone())).with_color(color).with_transform(t).pick_base(face_base);
            if dimmed {
                inst = inst.unpickable();
            }
            scene.instances.push(inst);
        }
        if !dimmed {
            let edge_color = if o.kind == BodyKind::Wire { theme::lin(0xd6dae2) } else { theme::lin(0x0b0d10) };
            scene.instances.push(Instance::new(Drawable::Lines(cached.edges.clone())).with_color(edge_color).with_transform(t).pick_base(edge_base).layer(1));
            scene.instances.push(Instance::new(Drawable::Points(cached.vertices.clone())).with_transform(t).pick_base(vert_base).pick_only().layer(2));
        }
    }
    let static_instances = scene.instances.len();
    scene.states = vec![0; picks.len() + 1];
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
    CachedBody { mesh: up.upload_mesh(&mesh), edges: up.upload_lines(&lines), vertices: up.upload_points(&points) }
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
}

pub fn with_highlights(built: &AsmBuilt, result: &AssemblyResult, h: &Highlights, up: &Uploader) -> Scene {
    let mut scene = Scene { instances: built.scene.instances[..built.static_instances].to_vec(), states: vec![0; built.picks.len() + 1], clip: built.scene.clip };
    for top in &h.accent_tops {
        for &id in built.picks.faces_of(*top) {
            scene.add_flag(id, state::ACCENT);
        }
    }
    for top in &h.selected_tops {
        for &id in built.picks.faces_of(*top) {
            scene.add_flag(id, state::SELECTED);
        }
    }
    let mut pts = PointData::default();
    let sel = theme::lin(0xffb020);
    let hov = theme::lin(0x4cc9f0);
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
    for d in &h.dots {
        pts.points.push(PointSprite { position: d.as_vec3().into(), color: [0xe7, 0xe9, 0xee, 255], pick_id: 0, size: 6.0, style: LineSegment::ON_TOP, _pad: 0 });
    }
    let mut lines = LineData::default();
    for t in &h.triads {
        let o: [f32; 3] = t.frame.origin.as_vec3().into();
        let w = if t.strong { 2.5 } else { 1.5 };
        for (axis, color) in [(t.frame.x(), [0xff, 0x5c, 0x5c, 255]), (t.frame.y(), [0x3e, 0xcf, 0x8e, 255]), (t.frame.z(), [0x5a, 0xa9, 0xff, 255])] {
            let tip: [f32; 3] = (t.frame.origin + axis * t.size).as_vec3().into();
            lines.segments.push(LineSegment { a: o, b: tip, color, pick_id: 0, width: w, style: LineSegment::ON_TOP });
        }
        pts.points.push(PointSprite { position: o, color: [0xff, 0xff, 0xff, 255], pick_id: 0, size: if t.strong { 7.0 } else { 5.0 }, style: LineSegment::ON_TOP, _pad: 0 });
    }
    if !lines.segments.is_empty() {
        scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(20));
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
