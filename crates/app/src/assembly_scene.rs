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
        Arc::new(move |id| if id == 0 { -1 } else { prio.get(id as usize - 1).copied().unwrap_or(100) })
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
    /// Instances drawn transparent and not pickable (being dragged in snap mode).
    pub ghost_tops: HashSet<noshape_model::assembly::InstanceId>,
    /// Section view planes (normal, d): geometry with dot(normal, p) > d is cut away.
    pub section: Vec<(DVec3, f64)>,
    /// Opacity of transparent instances (Isolate / Make transparent slider).
    pub dim_alpha: Option<f32>,
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
        if o.kind != BodyKind::Wire {
            let mut inst = Instance::new(Drawable::Mesh(cached.mesh.clone())).with_color(color).with_transform(t).pick_base(face_base);
            if dimmed {
                inst = inst.unpickable();
            }
            scene.instances.push(inst);
        }
        if !dimmed {
            let edge_color = theme::lin(theme::viewport::EDGE);
            scene.instances.push(Instance::new(Drawable::Lines(cached.edges.clone())).with_color(edge_color).with_transform(t).pick_base(edge_base).layer(1));
            scene.instances.push(Instance::new(Drawable::Points(cached.vertices.clone())).with_transform(t).pick_base(vert_base).pick_only().layer(2));
        }
    }
    let static_instances = scene.instances.len();
    scene.states = vec![0; picks.len() + 1];
    let clip = |(n, d): &(DVec3, f64)| noshape_render::ClipPlane { normal: n.as_vec3(), d: *d as f32 };
    scene.clip = opts.section.first().map(clip);
    scene.clip_extra = opts.section.iter().skip(1).map(clip).collect();
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
    /// Move manipulator (triad with pickable axis arrows and rotation rings)
    /// and the hovered handle.
    pub manipulator: Option<(Frame, f64, Option<ManipHandle>)>,
    /// Pickable assembly origin triad (size) and whether it is hovered/selected.
    pub origin: Option<(f64, bool)>,
    /// Pickable assembly mate connector triads: (frame, size, highlighted).
    pub connectors: Vec<(Frame, f64, bool)>,
    /// Mate glyph lines (between connector origins).
    pub mate_lines: Vec<(DVec3, DVec3, bool)>,
    /// Pickable mate indicators: (position, highlighted, state). Onshape
    /// colors: blue OK, red problem, grey suppressed / inactive.
    pub mate_glyphs: Vec<(DVec3, bool, MateGlyphState)>,
    /// Placement preview cross at the cursor (insert).
    pub cursor_mark: Option<(DVec3, f64)>,
    /// Explode lines (dashed), start to exploded position.
    pub trails: Vec<(DVec3, DVec3)>,
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

/// Pickable overlay items beyond the geometry picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayPick {
    Manip(ManipHandle),
    Origin,
    Connector(usize),
    /// Mate indicator (index into the view's displayed mates).
    Mate(usize),
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
        k => OverlayPick::Connector(k as usize - 11),
    })
}

const AXIS_COLORS: [[u8; 4]; 3] = [theme::rgb8(theme::viewport::AXIS_X), theme::rgb8(theme::viewport::AXIS_Y), theme::rgb8(theme::viewport::AXIS_Z)];
/// Hovered manipulator handles (Onshape UI_HIGHLIGHT_COLOR).
const HANDLE_HL: [u8; 4] = [0xff, 0xc2, 0x0e, 255];
const HANDLE_HL_FILL: [u8; 4] = [0xff, 0xc2, 0x0e, 150];
/// Dark points and lines on the white viewport.
const DARK: [u8; 4] = [0x33, 0x33, 0x33, 255];

fn push_triad(lines: &mut LineData, f: &Frame, size: f64, width: f32, pick: u32, highlight: bool) {
    let o: [f32; 3] = f.origin.as_vec3().into();
    for (k, axis) in [f.x(), f.y(), f.z()].into_iter().enumerate() {
        let tip: [f32; 3] = (f.origin + axis * size).as_vec3().into();
        let c = if highlight { HANDLE_HL } else { AXIS_COLORS[k] };
        lines.segments.push(LineSegment { a: o, b: tip, color: c, pick_id: pick, width, style: LineSegment::ON_TOP });
    }
}

pub fn with_highlights(built: &AsmBuilt, result: &AssemblyResult, h: &Highlights, up: &Uploader) -> Scene {
    let mut scene = Scene { instances: built.scene.instances[..built.static_instances].to_vec(), states: vec![0; built.picks.len() + 1], clip: built.scene.clip, clip_extra: built.scene.clip_extra.clone() };
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
        lines.segments.push(LineSegment { a: a.as_vec3().into(), b: b.as_vec3().into(), color: theme::rgb8(theme::viewport::SELECTION), pick_id: 0, width: if *strong { 2.0 } else { 1.2 }, style: LineSegment::ON_TOP });
    }
    for (a, b) in &h.trails {
        lines.segments.push(LineSegment { a: a.as_vec3().into(), b: b.as_vec3().into(), color: DARK, pick_id: 0, width: 1.5, style: LineSegment::DASHED | LineSegment::ON_TOP });
    }
    if let Some((p, size)) = h.cursor_mark {
        for axis in [DVec3::X, DVec3::Y, DVec3::Z] {
            lines.segments.push(LineSegment { a: (p - axis * size).as_vec3().into(), b: (p + axis * size).as_vec3().into(), color: theme::rgb8(theme::viewport::PLANE_TEXT), pick_id: 0, width: 1.5, style: LineSegment::ON_TOP });
        }
    }
    if !lines.segments.is_empty() {
        scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(20));
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
    for (i, (f, size, hl)) in h.connectors.iter().enumerate() {
        let id = overlay_id(built, OverlayPick::Connector(i));
        push_triad(&mut ol, f, *size, 2.0, id, *hl);
        op.points.push(PointSprite { position: f.origin.as_vec3().into(), color: if *hl { HANDLE_HL } else { DARK }, pick_id: id, size: 9.0, style: LineSegment::ON_TOP, _pad: 0 });
        scene.set_state(id, 0);
    }
    for (i, (p, hl, st)) in h.mate_glyphs.iter().enumerate() {
        let id = overlay_id(built, OverlayPick::Mate(i));
        let color = match (hl, st) {
            (true, _) => HANDLE_HL,
            (_, MateGlyphState::Error) => theme::rgb8(theme::viewport::ERROR),
            (_, MateGlyphState::Inactive) => theme::rgb8(theme::viewport::INACTIVE),
            (_, MateGlyphState::Ok) => theme::rgb8(theme::viewport::PLANE_OUTLINE),
        };
        op.points.push(PointSprite { position: p.as_vec3().into(), color, pick_id: id, size: 12.0, style: LineSegment::ON_TOP, _pad: 0 });
        scene.set_state(id, 0);
    }
    if let Some((f, size, hovered)) = &h.manipulator {
        let o: [f32; 3] = f.origin.as_vec3().into();
        for (k, axis) in [f.x(), f.y(), f.z()].into_iter().enumerate() {
            let tip: [f32; 3] = (f.origin + axis * *size).as_vec3().into();
            let c = if *hovered == Some(ManipHandle::Axis(k)) { HANDLE_HL } else { AXIS_COLORS[k] };
            let id = overlay_id(built, OverlayPick::Manip(ManipHandle::Axis(k)));
            ol.segments.push(LineSegment { a: o, b: tip, color: c, pick_id: id, width: 5.0, style: LineSegment::ON_TOP });
            op.points.push(PointSprite { position: tip, color: c, pick_id: id, size: 10.0, style: LineSegment::ON_TOP, _pad: 0 });
            scene.set_state(id, 0);
            // Rotation ring about axis k (in the plane of the other two axes).
            let (u, v) = match k {
                0 => (f.y(), f.z()),
                1 => (f.z(), f.x()),
                _ => (f.x(), f.y()),
            };
            let rid = overlay_id(built, OverlayPick::Manip(ManipHandle::Ring(k)));
            let rc = if *hovered == Some(ManipHandle::Ring(k)) { HANDLE_HL } else { AXIS_COLORS[k] };
            let r = *size * 0.72;
            let n = 48;
            for i in 0..n {
                let (a0, a1) = (i as f64 / n as f64 * std::f64::consts::TAU, (i + 1) as f64 / n as f64 * std::f64::consts::TAU);
                let p0 = f.origin + (u * a0.cos() + v * a0.sin()) * r;
                let p1 = f.origin + (u * a1.cos() + v * a1.sin()) * r;
                ol.segments.push(LineSegment { a: p0.as_vec3().into(), b: p1.as_vec3().into(), color: rc, pick_id: rid, width: 3.0, style: LineSegment::ON_TOP });
            }
            scene.set_state(rid, 0);
            // Plane square for the plane normal to axis k (between the other two axes).
            let pid = overlay_id(built, OverlayPick::Manip(ManipHandle::Plane(k)));
            let hl = *hovered == Some(ManipHandle::Plane(k));
            let pc = if hl { HANDLE_HL } else { let c = AXIS_COLORS[k]; [c[0], c[1], c[2], 200] };
            let (a0, a1) = (*size * 0.24, *size * 0.5);
            let q = |x: f64, y: f64| -> [f32; 3] { (f.origin + u * x + v * y).as_vec3().into() };
            for (pa, pb) in [((a0, a0), (a1, a0)), ((a1, a0), (a1, a1)), ((a1, a1), (a0, a1)), ((a0, a1), (a0, a0))] {
                ol.segments.push(LineSegment { a: q(pa.0, pa.1), b: q(pb.0, pb.1), color: pc, pick_id: pid, width: 2.5, style: LineSegment::ON_TOP });
            }
            // Hatch fill (pickable over the whole square).
            let fill = if hl { HANDLE_HL_FILL } else { let c = AXIS_COLORS[k]; [c[0], c[1], c[2], 90] };
            for i in 1..8 {
                let y = a0 + (a1 - a0) * i as f64 / 8.0;
                ol.segments.push(LineSegment { a: q(a0, y), b: q(a1, y), color: fill, pick_id: pid, width: 4.0, style: LineSegment::ON_TOP });
            }
            scene.set_state(pid, 0);
        }
        let cid = overlay_id(built, OverlayPick::Manip(ManipHandle::Center));
        let cc = if *hovered == Some(ManipHandle::Center) { HANDLE_HL } else { DARK };
        op.points.push(PointSprite { position: o, color: cc, pick_id: cid, size: 14.0, style: LineSegment::ON_TOP, _pad: 0 });
        scene.set_state(cid, 0);
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
