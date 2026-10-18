//! Drawing view geometry: orientations and projection chains, hidden-line
//! projection of Part Studio bodies (via the kernel's HLR), section cuts,
//! detail crops, snap points and associative point references.
//!
//! Coordinates: every view has its own 2D *view coordinates* in meters
//! (x right, y up in the view plane, i.e. the HLR output frame). A view's
//! placement maps view coordinates to sheet millimeters (see
//! [`super::build::ViewPlacement`]); annotations store view coordinates.

use super::{Drawing, PointRef, Projection, Sheet, ViewId, ViewKind, ViewOrientation};
use crate::configuration::ConfigValues;
use crate::display::BodyKind;
use crate::document::{Document, TabContent, TabId};
use crate::naming::Name;
use crate::partstudio::PartStudio;
use crate::reference::BodyId;
use crate::regen::context::{Body, Context};
use crate::regen::{RegenOptions, Regenerator};
use glam::DVec3;
use noshape_kernel::{Curve, EdgeClass, Plane, Shape, ShapeType, Visibility};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

// ------------------------------------------------------------ sources

/// A regenerated Part Studio referenced by drawing views.
#[derive(Clone)]
pub struct SourceModel {
    /// Content hash of the studio (+ configuration); keys the HLR cache.
    pub hash: u64,
    pub context: Arc<Context>,
}

pub type Sources = HashMap<TabId, SourceModel>;

/// Content hash of a Part Studio definition and configuration.
pub fn studio_hash(studio: &PartStudio, config: &ConfigValues) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_vec(studio).unwrap_or_default().hash(&mut h);
    config.hash(&mut h);
    h.finish()
}

/// Part Studio tabs referenced by a drawing's base views (with the
/// configuration of the first view that uses each).
pub fn referenced_studios(drawing: &Drawing) -> Vec<(TabId, ConfigValues)> {
    let mut out: Vec<(TabId, ConfigValues)> = Vec::new();
    for sheet in &drawing.sheets {
        for v in &sheet.views {
            if let ViewKind::Base { source, config, .. } = &v.kind
                && !out.iter().any(|(t, _)| t == source)
            {
                out.push((*source, config.clone()));
            }
        }
    }
    out
}

/// Regenerate every Part Studio a drawing references (headless helper for
/// tests, export and the demo; the app regenerates in the background).
pub fn regenerate_sources(doc: &Document, drawing: &Drawing) -> Sources {
    let mut out = Sources::new();
    for (tab, config) in referenced_studios(drawing) {
        let Some(TabContent::PartStudio(ps)) = doc.tab(tab).map(|t| &t.content) else { continue };
        let mut reg = Regenerator::new();
        let r = reg.regenerate(ps, &config, &doc.settings.units, &RegenOptions::default());
        out.insert(tab, SourceModel { hash: studio_hash(ps, &config), context: r.context });
    }
    out
}

// ------------------------------------------------------------ frames

/// Orthonormal view frame: `dir` points from the eye into the scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewFrame {
    pub dir: DVec3,
    pub up: DVec3,
    pub right: DVec3,
}

impl ViewFrame {
    /// Same convention as the kernel HLR projector.
    pub fn new(dir: DVec3, up: DVec3) -> ViewFrame {
        let dir = dir.normalize();
        let z = -dir;
        let mut y = up - z * up.dot(z);
        if y.length_squared() < 1e-18 {
            y = z.any_orthonormal_vector();
        }
        let y = y.normalize();
        let x = y.cross(z);
        ViewFrame { dir, up: y, right: x }
    }

    pub fn project(&self, p: DVec3) -> [f64; 2] {
        [p.dot(self.right), p.dot(self.up)]
    }

    /// 3D direction of a 2D view-plane direction.
    pub fn lift(&self, v: [f64; 2]) -> DVec3 {
        self.right * v[0] + self.up * v[1]
    }

    fn key(&self, h: &mut impl Hasher) {
        for v in [self.dir, self.up] {
            for c in v.to_array() {
                ((c * 1e9).round() as i64).hash(h);
            }
        }
    }
}

pub fn orientation_frame(o: ViewOrientation) -> ViewFrame {
    let (dir, up) = match o {
        ViewOrientation::Front => (DVec3::Y, DVec3::Z),
        ViewOrientation::Back => (DVec3::NEG_Y, DVec3::Z),
        ViewOrientation::Top => (DVec3::NEG_Z, DVec3::Y),
        ViewOrientation::Bottom => (DVec3::Z, DVec3::NEG_Y),
        ViewOrientation::Left => (DVec3::X, DVec3::Z),
        ViewOrientation::Right => (DVec3::NEG_X, DVec3::Z),
        ViewOrientation::Isometric => (DVec3::new(-1.0, 1.0, -1.0), DVec3::Z),
    };
    ViewFrame::new(dir, up)
}

/// Snap a sheet direction to the nearest multiple of 45 degrees.
pub fn snap_direction(d: [f64; 2]) -> [f64; 2] {
    let a = d[1].atan2(d[0]);
    let k = (a / std::f64::consts::FRAC_PI_4).round();
    let a = k * std::f64::consts::FRAC_PI_4;
    let (s, c) = a.sin_cos();
    let r = |x: f64| if x.abs() < 1e-9 { 0.0 } else { x };
    [r(c), r(s)]
}

/// Frame of a view projected from `parent` and placed in sheet direction `m`.
/// Axis directions fold the view 90 degrees (third angle: the view shows the
/// side facing its placement; first angle: the opposite side); diagonal
/// directions give an isometric view.
pub fn fold(parent: &ViewFrame, m: [f64; 2], projection: Projection) -> ViewFrame {
    let l = (m[0] * m[0] + m[1] * m[1]).sqrt().max(1e-12);
    let m = [m[0] / l, m[1] / l];
    let sign = match projection {
        Projection::ThirdAngle => -1.0,
        Projection::FirstAngle => 1.0,
    };
    if m[0].abs() > 0.38 && m[1].abs() > 0.38 {
        let dir = parent.dir + (parent.right * m[0].signum() + parent.up * m[1].signum()) * sign;
        return ViewFrame::new(dir, parent.up);
    }
    fold_toward(parent, parent.lift(m) * sign)
}

/// Rotate the frame 90 degrees so that its viewing direction becomes `t`
/// (a unit vector in the parent's view plane).
fn fold_toward(parent: &ViewFrame, t: DVec3) -> ViewFrame {
    let t = t.normalize();
    let d = parent.dir;
    let rot = |v: DVec3| v - d * v.dot(d) - t * v.dot(t) + t * v.dot(d) - d * v.dot(t);
    ViewFrame::new(t, rot(parent.up))
}

fn view_center(sheet: &Sheet, id: ViewId) -> Option<[f64; 2]> {
    sheet.view(id).map(|v| v.position)
}

fn side_of(sheet: &Sheet, child: ViewId, parent: ViewId, n: [f64; 2]) -> f64 {
    match (view_center(sheet, child), view_center(sheet, parent)) {
        (Some(c), Some(p)) => {
            let s = (c[0] - p[0]) * n[0] + (c[1] - p[1]) * n[1];
            if s < 0.0 { -1.0 } else { 1.0 }
        }
        _ => 1.0,
    }
}

/// The sheet direction from parent to child used for projected views.
pub fn projected_direction(sheet: &Sheet, id: ViewId) -> Option<[f64; 2]> {
    let v = sheet.view(id)?;
    let ViewKind::Projected { parent, direction } = &v.kind else { return None };
    if let Some(d) = direction {
        return Some(snap_direction(*d));
    }
    let p = sheet.view(*parent)?;
    let d = [v.position[0] - p.position[0], v.position[1] - p.position[1]];
    if d[0].abs() + d[1].abs() < 1e-9 {
        return Some([1.0, 0.0]);
    }
    Some(snap_direction(d))
}

/// Section cut data in the parent's view coordinates.
#[derive(Clone, Copy, Debug)]
pub struct SectionCut {
    pub p0: [f64; 2],
    pub p1: [f64; 2],
    /// Unit sheet/view-plane direction pointing toward the section view
    /// (the removed side); the line of sight is the opposite direction.
    pub toward_view: [f64; 2],
}

pub fn section_cut(sheet: &Sheet, id: ViewId) -> Option<SectionCut> {
    let v = sheet.view(id)?;
    let ViewKind::Section { parent, line, .. } = &v.kind else { return None };
    if line.len() < 2 {
        return None;
    }
    let (p0, p1) = (line[0], line[line.len() - 1]);
    let e = [p1[0] - p0[0], p1[1] - p0[1]];
    let l = (e[0] * e[0] + e[1] * e[1]).sqrt();
    if l < 1e-12 {
        return None;
    }
    let n = [-e[1] / l, e[0] / l];
    let s = side_of(sheet, id, *parent, n);
    Some(SectionCut { p0, p1, toward_view: [n[0] * s, n[1] * s] })
}

/// The frame of any view on a sheet (follows the parent chain).
pub fn view_frame(drawing: &Drawing, sheet: &Sheet, id: ViewId) -> Option<ViewFrame> {
    view_frame_depth(drawing, sheet, id, 0)
}

fn view_frame_depth(drawing: &Drawing, sheet: &Sheet, id: ViewId, depth: usize) -> Option<ViewFrame> {
    if depth > 32 {
        return None;
    }
    let v = sheet.view(id)?;
    match &v.kind {
        ViewKind::Base { orientation, .. } => Some(orientation_frame(*orientation)),
        ViewKind::Projected { parent, .. } => {
            let pf = view_frame_depth(drawing, sheet, *parent, depth + 1)?;
            Some(fold(&pf, projected_direction(sheet, id)?, drawing.projection))
        }
        ViewKind::Auxiliary { parent, edge_dir } => {
            let pf = view_frame_depth(drawing, sheet, *parent, depth + 1)?;
            let l = (edge_dir[0].powi(2) + edge_dir[1].powi(2)).sqrt().max(1e-12);
            let n = [-edge_dir[1] / l, edge_dir[0] / l];
            let s = side_of(sheet, id, *parent, n);
            let sign = if drawing.projection == Projection::ThirdAngle { -1.0 } else { 1.0 };
            Some(fold_toward(&pf, pf.lift([n[0] * s, n[1] * s]) * sign))
        }
        ViewKind::Section { parent, .. } => {
            let pf = view_frame_depth(drawing, sheet, *parent, depth + 1)?;
            let cut = section_cut(sheet, id)?;
            // Line of sight is away from the view's placement side.
            Some(fold_toward(&pf, -pf.lift(cut.toward_view)))
        }
        ViewKind::Detail { parent, .. } => view_frame_depth(drawing, sheet, *parent, depth + 1),
    }
}

/// The root base view of a view (follows parents).
pub fn root_view(sheet: &Sheet, id: ViewId) -> Option<ViewId> {
    let mut cur = id;
    for _ in 0..64 {
        match sheet.parent_of(cur) {
            Some(p) => cur = p,
            None => return sheet.view(cur).map(|v| v.id),
        }
    }
    None
}

/// Source tab and part filter of a view.
pub fn view_source(sheet: &Sheet, id: ViewId) -> Option<(TabId, Option<BodyId>)> {
    let root = sheet.view(root_view(sheet, id)?)?;
    match &root.kind {
        ViewKind::Base { source, part, .. } => Some((*source, part.clone())),
        _ => None,
    }
}

/// Effective scale (paper length / model length) of a view.
pub fn view_scale(sheet: &Sheet, id: ViewId) -> f64 {
    let mut cur = id;
    for _ in 0..64 {
        let Some(v) = sheet.view(cur) else { break };
        if let Some(s) = v.scale
            && s > 0.0
        {
            return s;
        }
        match sheet.parent_of(cur) {
            Some(p) => cur = p,
            None => break,
        }
    }
    if sheet.scale > 0.0 { sheet.scale } else { 1.0 }
}

/// The solid bodies a view shows.
pub fn view_bodies<'a>(ctx: &'a Context, part: Option<&BodyId>) -> Vec<&'a Arc<Body>> {
    ctx.bodies.iter().filter(|b| b.kind == BodyKind::Solid && part.is_none_or(|p| &b.id == p)).collect()
}

// ------------------------------------------------------------ geometry

/// Classification of a model edge for dimensioning.
#[derive(Clone, Debug, PartialEq)]
pub enum EdgeGeom {
    /// Straight edge (projected endpoints).
    Line { a: [f64; 2], b: [f64; 2] },
    /// Circle or arc. `face_on` when the circle's axis is parallel to the
    /// viewing direction (it projects to a true circle).
    Circle { center: [f64; 2], radius: f64, face_on: bool },
    Other,
}

/// A model edge that appears in a view.
#[derive(Clone, Debug)]
pub struct ModelEdge {
    pub body: BodyId,
    pub name: Name,
    pub geom: EdgeGeom,
    /// Endpoint references (start, end) with their projected positions.
    pub ends: [Option<(PointRef, [f64; 2])>; 2],
}

impl ModelEdge {
    pub fn center_ref(&self) -> Option<PointRef> {
        matches!(self.geom, EdgeGeom::Circle { .. }).then(|| PointRef::Center { body: self.body.clone(), name: (*self.name).clone() })
    }
}

#[derive(Clone, Debug)]
pub struct ViewEdge {
    pub points: Vec<[f64; 2]>,
    pub visible: bool,
    pub class: EdgeClass,
    /// Index into [`ViewGeometry::model_edges`].
    pub model: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    Vertex,
    Center,
}

/// A point the dimension tools can snap to.
#[derive(Clone, Debug)]
pub struct Snap {
    pub p: [f64; 2],
    pub kind: SnapKind,
    pub point: Option<PointRef>,
}

/// Projected geometry of one view, in view coordinates (meters).
#[derive(Clone, Debug, Default)]
pub struct ViewGeometry {
    pub id: ViewId,
    pub frame: Option<ViewFrame>,
    /// View coordinates that sit at the view's sheet position.
    pub origin: [f64; 2],
    pub edges: Vec<ViewEdge>,
    pub model_edges: Vec<ModelEdge>,
    pub snaps: Vec<Snap>,
    /// Closed boundary loops (as segment soups) of section cut faces.
    pub section_segments: Vec<[[f64; 2]; 2]>,
    /// Detail crop circle (center, radius) in view coordinates.
    pub clip: Option<([f64; 2], f64)>,
    pub min: [f64; 2],
    pub max: [f64; 2],
    /// Exact HLR (false = fast polygonal preview).
    pub exact: bool,
    pub error: Option<String>,
}

impl ViewGeometry {
    fn update_bounds(&mut self) {
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for e in &self.edges {
            for p in &e.points {
                for k in 0..2 {
                    min[k] = min[k].min(p[k]);
                    max[k] = max[k].max(p[k]);
                }
            }
        }
        if let Some((c, r)) = self.clip {
            min = [c[0] - r, c[1] - r];
            max = [c[0] + r, c[1] + r];
        }
        if !min[0].is_finite() {
            min = self.origin;
            max = self.origin;
        }
        self.min = min;
        self.max = max;
    }

    /// Edge whose polyline passes closest to `p` (view coordinates), within `tol`.
    pub fn edge_near(&self, p: [f64; 2], tol: f64, include_hidden: bool) -> Option<(usize, f64)> {
        let mut best: Option<(usize, f64)> = None;
        for (i, e) in self.edges.iter().enumerate() {
            if !include_hidden && !e.visible {
                continue;
            }
            for w in e.points.windows(2) {
                let d = dist_to_segment(p, w[0], w[1]);
                if d <= tol && best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((i, d));
                }
            }
        }
        best
    }

    pub fn snap_near(&self, p: [f64; 2], tol: f64) -> Option<&Snap> {
        self.snaps
            .iter()
            .map(|s| (s, ((s.p[0] - p[0]).powi(2) + (s.p[1] - p[1]).powi(2)).sqrt()))
            .filter(|(_, d)| *d <= tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(s, _)| s)
    }

    /// Face-on circles (center, radius, reference) for radial dimensions
    /// and centermarks.
    pub fn circles(&self) -> Vec<([f64; 2], f64, Option<PointRef>)> {
        let mut out: Vec<([f64; 2], f64, Option<PointRef>)> = Vec::new();
        for m in &self.model_edges {
            if let EdgeGeom::Circle { center, radius, face_on: true } = m.geom
                && !out.iter().any(|(c, r, _)| (c[0] - center[0]).abs() < 1e-9 && (c[1] - center[1]).abs() < 1e-9 && (r - radius).abs() < 1e-9)
            {
                out.push((center, radius, m.center_ref()));
            }
        }
        out
    }
}

pub fn dist_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l2 = d[0] * d[0] + d[1] * d[1];
    let t = if l2 > 0.0 { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let q = [a[0] + d[0] * t, a[1] + d[1] * t];
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()
}

/// Sample an edge into 3D points.
pub fn sample_edge(e: &Shape, segments: usize) -> Vec<DVec3> {
    let Ok(info) = e.edge_info() else { return vec![] };
    if info.degenerated {
        return vec![];
    }
    if matches!(info.curve, Curve::Line { .. }) {
        return vec![info.start, info.end];
    }
    let n = segments.max(2);
    (0..=n).filter_map(|i| e.point_at(info.first_param + (info.last_param - info.first_param) * i as f64 / n as f64).ok()).collect()
}

/// Bounding box center of a set of bodies.
fn bodies_center(bodies: &[&Arc<Body>]) -> DVec3 {
    let mut acc: Option<(DVec3, DVec3)> = None;
    for b in bodies {
        if let Ok(bb) = b.shape.bounding_box(false) {
            acc = Some(match acc {
                None => (bb.min, bb.max),
                Some((a, c)) => (a.min(bb.min), c.max(bb.max)),
            });
        }
    }
    acc.map(|(a, b)| (a + b) * 0.5).unwrap_or(DVec3::ZERO)
}

fn bodies_diagonal(bodies: &[&Arc<Body>]) -> f64 {
    let mut acc: Option<(DVec3, DVec3)> = None;
    for b in bodies {
        if let Ok(bb) = b.shape.bounding_box(false) {
            acc = Some(match acc {
                None => (bb.min, bb.max),
                Some((a, c)) => (a.min(bb.min), c.max(bb.max)),
            });
        }
    }
    acc.map(|(a, b)| (b - a).length()).unwrap_or(0.1).max(1e-6)
}

/// Resolve a point reference: projected position and, for circle centers,
/// the radius.
pub fn resolve_point(ctx: &Context, frame: &ViewFrame, r: &PointRef) -> Option<([f64; 2], Option<f64>)> {
    match r {
        PointRef::Vertex { body, name } => {
            let b = ctx.body(body)?;
            let i = *b.find_vertices(name).first()?;
            let p = b.shape.sub_shape(ShapeType::Vertex, i).ok()?.vertex_point().ok()?;
            Some((frame.project(p), None))
        }
        PointRef::Center { body, name } => {
            let b = ctx.body(body)?;
            let i = *b.find_edges(name).first()?;
            match b.shape.sub_shape(ShapeType::Edge, i).ok()?.edge_curve().ok()? {
                Curve::Circle { center, radius, .. } => Some((frame.project(center), Some(radius))),
                _ => None,
            }
        }
    }
}

/// Reference to the vertex nearest to a 3D point (within `tol`).
pub fn vertex_ref_near(ctx: &Context, p: DVec3, tol: f64) -> Option<PointRef> {
    let mut best: Option<(f64, PointRef)> = None;
    for b in &ctx.bodies {
        for (i, v) in b.shape.vertices().iter().enumerate() {
            let Ok(q) = v.vertex_point() else { continue };
            let d = q.distance(p);
            if d <= tol && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best = Some((d, PointRef::Vertex { body: b.id.clone(), name: (*b.vertices[i]).clone() }));
            }
        }
    }
    best.map(|(_, r)| r)
}

/// Reference to the center of a circular edge whose center is near `p`
/// (the largest such circle when `largest`, else the smallest).
pub fn circle_ref_near(ctx: &Context, p: DVec3, tol: f64, largest: bool) -> Option<PointRef> {
    let mut best: Option<(f64, PointRef)> = None;
    for b in &ctx.bodies {
        for (i, e) in b.shape.edges().iter().enumerate() {
            if let Ok(Curve::Circle { center, radius, .. }) = e.edge_curve()
                && center.distance(p) <= tol
            {
                let better = match &best {
                    None => true,
                    Some((r, _)) => (largest && radius > *r) || (!largest && radius < *r),
                };
                if better {
                    best = Some((radius, PointRef::Center { body: b.id.clone(), name: (*b.edges[i]).clone() }));
                }
            }
        }
    }
    best.map(|(_, r)| r)
}

// ------------------------------------------------------------ computation

/// Cache of computed view geometry keyed by (source content, view
/// parameters, exactness); shared between sheets and rebuilds.
#[derive(Default)]
pub struct ViewCache {
    map: Mutex<HashMap<u64, Arc<ViewGeometry>>>,
}

impl ViewCache {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, key: u64) -> Option<Arc<ViewGeometry>> {
        self.map.lock().get(&key).cloned()
    }
    fn put(&self, key: u64, g: Arc<ViewGeometry>) {
        let mut m = self.map.lock();
        if m.len() > 512 {
            m.clear();
        }
        m.insert(key, g);
    }
    pub fn len(&self) -> usize {
        self.map.lock().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Cache key of a view's geometry; `None` when the view cannot be computed
/// (missing parent or source).
pub fn view_key(drawing: &Drawing, sheet: &Sheet, id: ViewId, sources: &Sources, exact: bool) -> Option<u64> {
    let v = sheet.view(id)?;
    let frame = view_frame(drawing, sheet, id)?;
    let (tab, part) = view_source(sheet, id)?;
    let src = sources.get(&tab)?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    src.hash.hash(&mut h);
    part.hash(&mut h);
    frame.key(&mut h);
    exact.hash(&mut h);
    match &v.kind {
        ViewKind::Section { parent, .. } => {
            1u8.hash(&mut h);
            let cut = section_cut(sheet, id)?;
            let pf = view_frame(drawing, sheet, *parent)?;
            pf.key(&mut h);
            for x in [cut.p0[0], cut.p0[1], cut.p1[0], cut.p1[1], cut.toward_view[0], cut.toward_view[1]] {
                ((x * 1e9).round() as i64).hash(&mut h);
            }
        }
        ViewKind::Detail { parent, center, radius, .. } => {
            2u8.hash(&mut h);
            view_key(drawing, sheet, *parent, sources, exact)?.hash(&mut h);
            for x in [center[0], center[1], *radius] {
                ((x * 1e9).round() as i64).hash(&mut h);
            }
        }
        _ => 0u8.hash(&mut h),
    }
    Some(h.finish())
}

/// Compute (or fetch from the cache) a view's geometry.
pub fn compute_view(drawing: &Drawing, sheet: &Sheet, id: ViewId, sources: &Sources, cache: &ViewCache, exact: bool) -> Arc<ViewGeometry> {
    let fail = |msg: &str| Arc::new(ViewGeometry { id, error: Some(msg.to_string()), ..Default::default() });
    let Some(key) = view_key(drawing, sheet, id, sources, exact) else {
        return fail("The view's source is not available");
    };
    if let Some(g) = cache.get(key) {
        return g;
    }
    let v = sheet.view(id).expect("view exists (key computed)");
    let frame = view_frame(drawing, sheet, id).expect("frame exists (key computed)");
    let (tab, part) = view_source(sheet, id).expect("source exists (key computed)");
    let ctx = sources.get(&tab).expect("source exists (key computed)").context.clone();
    let g = match &v.kind {
        ViewKind::Detail { parent, center, radius, .. } => {
            let pg = compute_view(drawing, sheet, *parent, sources, cache, exact);
            detail_geometry(id, &pg, *center, *radius)
        }
        ViewKind::Section { parent, .. } => {
            let cut = section_cut(sheet, id).expect("cut exists (key computed)");
            let pf = view_frame(drawing, sheet, *parent).expect("parent frame");
            section_geometry(id, &ctx, part.as_ref(), &frame, &pf, &cut, exact)
        }
        _ => projected_geometry(id, &ctx, part.as_ref(), &frame, exact, None),
    };
    let g = Arc::new(g);
    cache.put(key, g.clone());
    g
}

/// Compute every view of a sheet (parents first).
pub fn compute_sheet_views(drawing: &Drawing, sheet: &Sheet, sources: &Sources, cache: &ViewCache, exact: bool) -> HashMap<ViewId, Arc<ViewGeometry>> {
    sheet.views.iter().map(|v| (v.id, compute_view(drawing, sheet, v.id, sources, cache, exact))).collect()
}

/// Per-body edge -> (start vertex, end vertex) indices.
fn edge_vertices(shape: &Shape) -> Vec<Vec<usize>> {
    let ne = shape.count(ShapeType::Edge);
    let mut out = vec![Vec::new(); ne];
    if let Ok(ve) = shape.vertex_edges() {
        for (vi, edges) in ve.iter().enumerate() {
            for &e in edges {
                if let Some(slot) = out.get_mut(e as usize) {
                    slot.push(vi);
                }
            }
        }
    }
    out
}

/// One body as projected: the body (for names) and the shape to project
/// (the body itself or a section cut of it), with an output-edge -> body-edge map.
struct ProjBody<'a> {
    body: &'a Arc<Body>,
    shape: Shape,
    edge_map: Option<HashMap<usize, usize>>,
}

fn model_edge(body: &Arc<Body>, edge_index: usize, frame: &ViewFrame, ev: &[Vec<usize>]) -> Option<ModelEdge> {
    let e = body.shape.sub_shape(ShapeType::Edge, edge_index).ok()?;
    let info = e.edge_info().ok()?;
    let geom = match info.curve {
        Curve::Line { .. } => EdgeGeom::Line { a: frame.project(info.start), b: frame.project(info.end) },
        Curve::Circle { center, axis, radius, .. } => {
            EdgeGeom::Circle { center: frame.project(center), radius, face_on: axis.normalize().dot(frame.dir).abs() > 1.0 - 1e-6 }
        }
        _ => EdgeGeom::Other,
    };
    let mut ends: [Option<(PointRef, [f64; 2])>; 2] = [None, None];
    if let Some(vs) = ev.get(edge_index) {
        for &vi in vs {
            let Ok(p) = body.shape.sub_shape(ShapeType::Vertex, vi).and_then(|v| v.vertex_point()) else { continue };
            let r = PointRef::Vertex { body: body.id.clone(), name: (*body.vertices[vi]).clone() };
            let slot = if p.distance(info.start) <= p.distance(info.end) { 0 } else { 1 };
            if ends[slot].is_none() {
                ends[slot] = Some((r, frame.project(p)));
            } else if ends[1 - slot].is_none() {
                ends[1 - slot] = Some((r, frame.project(p)));
            }
        }
    }
    Some(ModelEdge { body: body.id.clone(), name: body.edges[edge_index].clone(), geom, ends })
}

/// HLR projection of bodies (optionally pre-cut) into a view.
fn project_bodies(id: ViewId, bodies: &[ProjBody], frame: &ViewFrame, exact: bool, keep: Option<(&Plane, f64)>) -> ViewGeometry {
    let mut g = ViewGeometry { id, frame: Some(*frame), exact, ..Default::default() };
    if bodies.is_empty() {
        g.error = Some("No parts to show".into());
        g.update_bounds();
        return g;
    }
    let shapes: Vec<Shape> = bodies.iter().map(|b| b.shape.clone()).collect();
    let compound = if shapes.len() == 1 { shapes[0].clone() } else { Shape::compound(&shapes) };
    // Compound edge index -> (body index, body edge index).
    let mut cmap: HashMap<usize, (usize, usize)> = HashMap::new();
    for (bi, pb) in bodies.iter().enumerate() {
        for (i, e) in pb.shape.edges().iter().enumerate() {
            let src = match &pb.edge_map {
                Some(m) => match m.get(&i) {
                    Some(s) => *s,
                    None => continue,
                },
                None => i,
            };
            if let Some(ci) = compound.index_of(e, ShapeType::Edge) {
                cmap.insert(ci, (bi, src));
            }
        }
    }
    let diag = compound.bounding_box(false).map(|b| b.diagonal()).unwrap_or(0.1).max(1e-6);
    let defl = if exact { diag / 1500.0 } else { diag / 500.0 };
    let projected = match noshape_kernel::project_hlr_with(&compound, frame.dir, frame.up, exact, defl) {
        Ok(p) => p,
        Err(e) => {
            g.error = Some(format!("Hidden line removal failed: {e}"));
            g.update_bounds();
            return g;
        }
    };
    let mut ev_cache: HashMap<usize, Vec<Vec<usize>>> = HashMap::new();
    let mut me_index: HashMap<(usize, usize), u32> = HashMap::new();
    for pe in projected {
        if pe.polyline.len() < 2 {
            continue;
        }
        let model = pe.source_edge.and_then(|ci| cmap.get(&(ci as usize)).copied()).and_then(|(bi, ei)| {
            if let Some(ix) = me_index.get(&(bi, ei)) {
                return Some(*ix);
            }
            let body = bodies[bi].body;
            let ev = ev_cache.entry(bi).or_insert_with(|| edge_vertices(&body.shape));
            let me = model_edge(body, ei, frame, ev)?;
            g.model_edges.push(me);
            let ix = (g.model_edges.len() - 1) as u32;
            me_index.insert((bi, ei), ix);
            Some(ix)
        });
        g.edges.push(ViewEdge {
            points: pe.polyline.iter().map(|p| [p.x, p.y]).collect(),
            visible: pe.visibility == Visibility::Visible,
            class: pe.class,
            model,
        });
    }
    // Snap points: vertices (on the kept side of a section) and centers of
    // face-on circles.
    for pb in bodies {
        let b = pb.body;
        for (i, v) in b.shape.vertices().iter().enumerate() {
            let Ok(p) = v.vertex_point() else { continue };
            if let Some((plane, tol)) = keep
                && plane.signed_distance(p) < -tol
            {
                continue;
            }
            g.snaps.push(Snap { p: frame.project(p), kind: SnapKind::Vertex, point: Some(PointRef::Vertex { body: b.id.clone(), name: (*b.vertices[i]).clone() }) });
        }
    }
    let centers: Vec<Snap> = g
        .model_edges
        .iter()
        .filter_map(|m| match m.geom {
            EdgeGeom::Circle { center, face_on: true, .. } => Some(Snap { p: center, kind: SnapKind::Center, point: m.center_ref() }),
            _ => None,
        })
        .collect();
    g.snaps.extend(centers);
    g
}

fn projected_geometry(id: ViewId, ctx: &Context, part: Option<&BodyId>, frame: &ViewFrame, exact: bool, keep: Option<(&Plane, f64)>) -> ViewGeometry {
    let bodies = view_bodies(ctx, part);
    let pbs: Vec<ProjBody> = bodies.iter().map(|b| ProjBody { body: b, shape: b.shape.clone(), edge_map: None }).collect();
    let mut g = project_bodies(id, &pbs, frame, exact, keep);
    g.origin = frame.project(bodies_center(&bodies));
    g.update_bounds();
    g
}

fn section_geometry(id: ViewId, ctx: &Context, part: Option<&BodyId>, frame: &ViewFrame, parent: &ViewFrame, cut: &SectionCut, exact: bool) -> ViewGeometry {
    let bodies = view_bodies(ctx, part);
    let diag = bodies_diagonal(&bodies);
    let center = bodies_center(&bodies);
    // Cutting plane: contains the line (extruded along the parent's view
    // direction); its normal is the line of sight.
    let sight = frame.dir;
    let along = parent.lift([cut.p1[0] - cut.p0[0], cut.p1[1] - cut.p0[1]]).normalize();
    let p0 = parent.lift(cut.p0) + parent.dir * center.dot(parent.dir);
    let plane = Plane::new(p0, sight, along);
    // Box covering the removed half-space (between the viewer and the plane).
    let big = diag * 4.0 + 1.0;
    let side = along.cross(sight).normalize();
    let corner = p0 - along * big - side * big;
    let removed = noshape_kernel::make_box(&Plane::new(corner, -sight, along), big * 2.0, big * 2.0, big);
    let mut pbs: Vec<ProjBody> = Vec::new();
    let mut caps: Vec<Shape> = Vec::new();
    let mut error = None;
    for b in &bodies {
        let cut_result = match &removed {
            Ok(tool) => noshape_kernel::cut(&b.shape, tool),
            Err(e) => Err(e.clone()),
        };
        match cut_result {
            Ok(r) => {
                if r.shape.count(ShapeType::Face) == 0 {
                    continue;
                }
                let mut map = HashMap::new();
                for i in 0..b.shape.count(ShapeType::Edge) {
                    for o in r.history.modified(0, ShapeType::Edge, i as u32) {
                        map.entry(o as usize).or_insert(i);
                    }
                }
                pbs.push(ProjBody { body: b, shape: r.shape, edge_map: Some(map) });
                if let Ok(c) = noshape_kernel::section_caps(&b.shape, &plane) {
                    caps.push(c);
                }
            }
            Err(e) => error = Some(format!("Section cut failed: {e}")),
        }
    }
    let mut g = project_bodies(id, &pbs, frame, exact, Some((&plane, diag * 1e-7)));
    if g.error.is_none() {
        g.error = error;
    }
    for c in caps {
        for e in c.edges() {
            let pts: Vec<[f64; 2]> = sample_edge(&e, 48).into_iter().map(|p| frame.project(p)).collect();
            for w in pts.windows(2) {
                g.section_segments.push([w[0], w[1]]);
            }
        }
    }
    g.origin = frame.project(center);
    g.update_bounds();
    g
}

/// Crop a parent view's geometry to a circle.
fn detail_geometry(id: ViewId, parent: &ViewGeometry, center: [f64; 2], radius: f64) -> ViewGeometry {
    let mut g = ViewGeometry { id, frame: parent.frame, origin: center, exact: parent.exact, error: parent.error.clone(), clip: Some((center, radius)), ..Default::default() };
    let mut used: HashMap<u32, u32> = HashMap::new();
    for e in &parent.edges {
        for piece in clip_polyline_circle(&e.points, center, radius) {
            let model = e.model.map(|m| {
                *used.entry(m).or_insert_with(|| {
                    g.model_edges.push(parent.model_edges[m as usize].clone());
                    (g.model_edges.len() - 1) as u32
                })
            });
            g.edges.push(ViewEdge { points: piece, visible: e.visible, class: e.class, model });
        }
    }
    let inside = |p: [f64; 2]| (p[0] - center[0]).powi(2) + (p[1] - center[1]).powi(2) <= radius * radius * (1.0 + 1e-9);
    g.snaps = parent.snaps.iter().filter(|s| inside(s.p)).cloned().collect();
    for s in &parent.section_segments {
        for piece in clip_polyline_circle(&[s[0], s[1]], center, radius) {
            for w in piece.windows(2) {
                g.section_segments.push([w[0], w[1]]);
            }
        }
    }
    g.update_bounds();
    g
}

/// Parts of a polyline inside a circle.
pub fn clip_polyline_circle(pts: &[[f64; 2]], c: [f64; 2], r: f64) -> Vec<Vec<[f64; 2]>> {
    let inside = |p: [f64; 2]| (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) <= r * r;
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut cur: Vec<[f64; 2]> = Vec::new();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        // Parameter interval of the segment inside the circle.
        let d = [b[0] - a[0], b[1] - a[1]];
        let f = [a[0] - c[0], a[1] - c[1]];
        let qa = d[0] * d[0] + d[1] * d[1];
        let qb = 2.0 * (f[0] * d[0] + f[1] * d[1]);
        let qc = f[0] * f[0] + f[1] * f[1] - r * r;
        let disc = qb * qb - 4.0 * qa * qc;
        if qa < 1e-30 {
            continue;
        }
        if disc <= 0.0 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        let sq = disc.sqrt();
        let t0 = ((-qb - sq) / (2.0 * qa)).max(0.0);
        let t1 = ((-qb + sq) / (2.0 * qa)).min(1.0);
        if t0 >= t1 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        let pa = [a[0] + d[0] * t0, a[1] + d[1] * t0];
        let pb = [a[0] + d[0] * t1, a[1] + d[1] * t1];
        if cur.is_empty() || !inside(a) || t0 > 1e-12 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(pa);
        }
        cur.push(pb);
        if t1 < 1.0 - 1e-12 {
            out.push(std::mem::take(&mut cur));
        }
    }
    if cur.len() >= 2 {
        out.push(cur);
    }
    out.retain(|p| p.len() >= 2);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_frames_follow_onshape_axes() {
        let f = orientation_frame(ViewOrientation::Front);
        assert!((f.right - DVec3::X).length() < 1e-12 && (f.up - DVec3::Z).length() < 1e-12);
        let t = orientation_frame(ViewOrientation::Top);
        assert!((t.right - DVec3::X).length() < 1e-12 && (t.up - DVec3::Y).length() < 1e-12);
        let r = orientation_frame(ViewOrientation::Right);
        assert!((r.right - DVec3::Y).length() < 1e-12 && (r.up - DVec3::Z).length() < 1e-12);
    }

    #[test]
    fn third_angle_folds() {
        let front = orientation_frame(ViewOrientation::Front);
        let top = orientation_frame(ViewOrientation::Top);
        let right = orientation_frame(ViewOrientation::Right);
        let up = fold(&front, [0.0, 1.0], Projection::ThirdAngle);
        assert!((up.dir - top.dir).length() < 1e-9 && (up.up - top.up).length() < 1e-9, "{up:?}");
        let rt = fold(&front, [1.0, 0.0], Projection::ThirdAngle);
        assert!((rt.dir - right.dir).length() < 1e-9 && (rt.up - right.up).length() < 1e-9);
        // First angle: the view to the right shows the left side.
        let lf = fold(&front, [1.0, 0.0], Projection::FirstAngle);
        assert!((lf.dir - orientation_frame(ViewOrientation::Left).dir).length() < 1e-9);
        // Diagonal: isometric.
        let iso = fold(&front, [1.0, 1.0], Projection::ThirdAngle);
        assert!((iso.dir - orientation_frame(ViewOrientation::Isometric).dir).length() < 1e-9);
    }

    #[test]
    fn circle_clipping() {
        let pieces = clip_polyline_circle(&[[-2.0, 0.0], [2.0, 0.0]], [0.0, 0.0], 1.0);
        assert_eq!(pieces.len(), 1);
        assert!((pieces[0][0][0] + 1.0).abs() < 1e-9 && (pieces[0][1][0] - 1.0).abs() < 1e-9);
        let pieces = clip_polyline_circle(&[[-2.0, 0.0], [0.0, 0.0], [0.0, 2.0], [2.0, 2.0]], [0.0, 0.0], 1.0);
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].len(), 3);
        assert!(clip_polyline_circle(&[[-2.0, 3.0], [2.0, 3.0]], [0.0, 0.0], 1.0).is_empty());
    }
}
