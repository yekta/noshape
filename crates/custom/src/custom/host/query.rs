//! Query evaluation (`@evaluateQuery`): every `QueryType` of query.fs over
//! the regeneration context.
//!
//! Entities are evaluated as [`Ent`] (indices into the current context) and
//! handed to FeatureScript as transient queries whose ids map to persistent
//! [`Reference`]s, so they stay valid across later operations as long as
//! the entity's name survives.

use super::{CtxState, FsContext, FsId, context_arg, convert::*, is_marker, std_enum, std_tag};
use crate::display::{BodyKind, PlaneFrame};
use crate::reference::{BodyId, DatumPlane, FeatureId, Reference, Role, TopoName};
use crate::regen::eval::{decode_region_key, encode_region_key};
use glam::{DMat4, DVec3};
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::{Map, Value};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub const NATIVES: &[(&str, NativeFn)] = &[
    ("evaluateQuery", evaluate_query),
    ("evaluateQueryCount", evaluate_query_count),
    ("isQueryEmpty", is_query_empty),
    ("transientIdToString", transient_id_to_string),
    ("getQueryVariable", get_query_variable),
    ("setQueryVariable", set_query_variable),
    ("queryContainsFlattenedSheetMetal", |_, _| Ok(Value::bool(false))),
];

/// An entity of the current context.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ent {
    Body(usize),
    Face(usize, usize),
    Edge(usize, usize),
    Vertex(usize, usize),
    Origin,
    Datum(DatumPlane),
    /// `ctx.planes[i]` (construction planes): its face.
    Plane(usize),
    /// `ctx.planes[i]`'s body (Onshape: a construction plane is a body owning one face; `qCreatedBy(id, BODY)`
    /// and `opDeleteBodies` take it, the user's "Y Ribs on Faces" 11-02).
    PlaneBody(usize),
    /// `ctx.connectors[i]` (mate connectors).
    Connector(usize),
    /// `ctx.points[i]`.
    Point(usize),
    /// `ctx.sketches[i]`'s sheet body (its regions; live: one per sketch), and the regions, curves and points.
    SketchSheet(usize),
    /// A sketch curve's wire body (live: one per curve).
    SketchWire(usize, u32),
    /// A sketch point's point body (live: a point that is no curve's end: centres, interpolation points, points).
    SketchPointBody(usize, u32),
    SketchRegion(usize, Vec<u32>),
    SketchEdge(usize, u32),
    SketchVertex(usize, u32),
    /// A sketch's region edge and vertex ([`RegionPieces`]' `edges[i]` / `verts[i]`): the regions' own split edges,
    /// apart from the entities' whole ones (live 2059).
    SketchRegionEdge(usize, u32),
    SketchRegionVertex(usize, u32),
    /// An open fit spline's start (0) or end (1) vertex: its edge's own, apart from the interpolation points there
    /// (live 2060).
    SketchCurveEnd(usize, u32, u8),
}

/// Whether sketch entity `id` is an open fit spline (its edge has end vertices of its own, live 2060).
pub fn open_fit_spline(sk: &noshape_sketch::Sketch, id: u32) -> bool {
    sk.entity(noshape_sketch::EntityId(id)).is_some_and(|e| matches!(&e.geometry, noshape_sketch::Geometry::FitSpline { points, .. } if points.first() != points.last()))
}

/// A sketch curve whose edge has end vertices of its own (live: each curve is a wire body of its own, a rectangle's
/// lines 8 vertices): lines, arcs, elliptical arcs, conics and open fit splines.
pub fn open_curve(sk: &noshape_sketch::Sketch, id: u32) -> bool {
    use noshape_sketch::Geometry as G;
    sk.entity(noshape_sketch::EntityId(id)).is_some_and(|e| match &e.geometry {
        G::Line { .. } | G::Arc { .. } | G::EllipticalArc { .. } | G::Conic { .. } => true,
        G::FitSpline { points, .. } => points.first() != points.last(),
        _ => false,
    })
}

/// A sketch piece's bodies: its curves' wires, its points' point bodies, its sheet.
fn sketch_bodies(sk: &crate::regen::context::SketchResult, s: usize) -> Vec<Ent> {
    let held = sk.held();
    let mut out: Vec<Ent> = held.entities.iter().filter_map(|&e| match sk.sketch.kind(noshape_sketch::EntityId(e))? {
        noshape_sketch::EntityKind::Point => Some(Ent::SketchPointBody(s, e)),
        _ => Some(Ent::SketchWire(s, e)),
    }).collect();
    if held.sheet && !sk.regions.is_empty() {
        out.push(Ent::SketchSheet(s));
    }
    out
}

/// A sketch's region edges: each region face's edges (as the regions extrude), one per piece the regions share, and
/// their end vertices (live 2059: `qCreatedBy(sketch, EDGE)` gives the entities' whole edges and these, which bound
/// the regions; a region's spline edge is the spline trimmed where it crosses the circle).
#[derive(Default)]
pub struct RegionPieces {
    pub edges: Vec<RegionPiece>,
    pub verts: Vec<(DVec3, u32, u8)>,
    /// Each region's face edges, as pieces.
    pub of_region: HashMap<Vec<u32>, Vec<u32>>,
}

pub struct RegionPiece {
    pub shape: Shape,
    /// The first region holding it and its index among that region's face edges (its reference).
    pub region: Vec<u32>,
    pub at: u32,
    pub regions: Vec<Vec<u32>>,
    pub ends: Vec<u32>,
}

impl RegionPieces {
    fn of(sk: &crate::regen::context::SketchResult) -> RegionPieces {
        let edges = crate::regen::eval::region_edges(sk);
        let plane = crate::regen::eval::kernel_plane(&sk.frame);
        let mut out = RegionPieces::default();
        for (g, _) in &edges.pieces {
            let Ok(shape) = k::make_edge_2d(&plane, &crate::regen::eval::to_segment(g)) else { return RegionPieces::default() };
            out.edges.push(RegionPiece { shape, region: Vec::new(), at: 0, regions: Vec::new(), ends: Vec::new() });
        }
        for (key, list) in &edges.of_region {
            for (at, &j) in list.iter().enumerate() {
                let p = &mut out.edges[j as usize];
                if p.regions.is_empty() {
                    p.region = key.clone();
                    p.at = at as u32;
                }
                if !p.regions.contains(key) {
                    p.regions.push(key.clone());
                }
            }
            out.of_region.insert(key.clone(), list.clone());
        }
        for j in 0..out.edges.len() {
            // (A closed piece, a circle's ring, has no vertex: live, a circle's sketch 1 vertex, its centre.)
            let Ok((a, b)) = out.edges[j].shape.endpoints() else { continue };
            if a.distance(b) < 1e-9 {
                continue;
            }
            for (end, p) in [(0u8, a), (1u8, b)] {
                let v = match out.verts.iter().position(|(q, ..)| q.distance(p) < 1e-9) {
                    Some(v) => v,
                    None => {
                        out.verts.push((p, j as u32, end));
                        out.verts.len() - 1
                    }
                };
                if !out.edges[j].ends.contains(&(v as u32)) {
                    out.edges[j].ends.push(v as u32);
                }
            }
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EType {
    Vertex,
    Edge,
    Face,
    Body,
}

impl EType {
    pub fn from_name(s: &str) -> Option<EType> {
        Some(match s {
            "VERTEX" => EType::Vertex,
            "EDGE" => EType::Edge,
            "FACE" => EType::Face,
            "BODY" => EType::Body,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            EType::Vertex => "VERTEX",
            EType::Edge => "EDGE",
            EType::Face => "FACE",
            EType::Body => "BODY",
        }
    }
}

pub fn entity_type_of(e: &Ent) -> EType {
    match e {
        Ent::Body(_) | Ent::SketchSheet(_) | Ent::SketchWire(..) | Ent::SketchPointBody(..) | Ent::Connector(_) | Ent::PlaneBody(_) => EType::Body,
        Ent::Face(..) | Ent::Datum(_) | Ent::Plane(_) | Ent::SketchRegion(..) => EType::Face,
        Ent::Edge(..) | Ent::SketchEdge(..) | Ent::SketchRegionEdge(..) => EType::Edge,
        Ent::Vertex(..) | Ent::Origin | Ent::Point(_) | Ent::SketchVertex(..) | Ent::SketchRegionVertex(..) | Ent::SketchCurveEnd(..) => EType::Vertex,
    }
}

/// Body/sub-shape topology of one body, computed on demand.
#[derive(Default)]
pub struct Topo {
    edge_faces: Vec<Vec<u32>>,
    vertex_faces: Vec<Vec<u32>>,
    vertex_edges: Vec<Vec<u32>>,
    edge_vertices: Vec<Vec<u32>>,
    /// Edges and vertices Onshape's (Parasolid) topology has: no seams, no
    /// degenerate edges, no vertices on closed curves only (live
    /// dom/fs-stdref.txt: a cylinder is 3 faces, 2 edges, 0 vertices).
    shown_edges: Vec<bool>,
    shown_vertices: Vec<bool>,
}

/// Evaluation state of one query.
/// An entity's type and a few points on it (see [`CtxState::snapshots`]).
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub etype: EType,
    pub points: Vec<DVec3>,
    /// The owner's body type ("SHEET", "SOLID", ...): a sheet's edge is not
    /// followed onto a solid lying on it (sheet metal part vs definition).
    pub body_type: &'static str,
    /// Taken from a sketch entity.
    pub sketch: bool,
    /// A face's whole shape: its split pieces (and copies of them) lie on it.
    pub face: Option<k::Shape>,
}

pub struct QueryCx<'a> {
    pub st: &'a CtxState,
    topo: RefCell<HashMap<usize, Arc<Topo>>>,
}

fn kerr(e: k::KernelError) -> String {
    e.to_string()
}

impl<'a> QueryCx<'a> {
    pub fn new(st: &'a CtxState) -> Self {
        QueryCx { st, topo: RefCell::new(HashMap::new()) }
    }

    fn topo(&self, b: usize) -> Arc<Topo> {
        if let Some(t) = self.topo.borrow().get(&b) {
            return t.clone();
        }
        // Kept for the body itself across queries (std's per-entity attribute loops build a QueryCx a call).
        let body = self.st.ctx.bodies.untracked()[b].clone();
        let key = Arc::as_ptr(&body) as usize;
        if let Some((_, t)) = self.st.topo_cache.lock().get(&key) {
            self.topo.borrow_mut().insert(b, t.clone());
            return t.clone();
        }
        let shape = &body.shape;
        let vertex_edges = shape.vertex_edges().unwrap_or_default();
        let mut edge_vertices = vec![Vec::new(); shape.count(ShapeType::Edge)];
        for (v, es) in vertex_edges.iter().enumerate() {
            for e in es {
                if let Some(ev) = edge_vertices.get_mut(*e as usize) {
                    ev.push(v as u32);
                }
            }
        }
        let edge_faces = shape.edge_faces().unwrap_or_default();
        // Onshape's topology (the kernel's shown_topology): no seams or
        // degenerated edges; a vertex counts if it ends a shown open edge, is
        // a point body's, or is a cone's apex (live dom/fs-ophole.txt: a drill
        // tip is a vertex; a sphere's poles and a cylinder's seam ends are not).
        let vertex_faces = shape.vertex_faces().unwrap_or_default();
        let shown = shape.shown_topology().unwrap_or_default();
        let (shown_edges, shown_vertices) = (shown.edges, shown.vertices);
        let t = Arc::new(Topo { edge_faces, vertex_faces, vertex_edges, edge_vertices, shown_edges, shown_vertices });
        self.topo.borrow_mut().insert(b, t.clone());
        let mut cache = self.st.topo_cache.lock();
        if cache.len() >= 256 {
            cache.clear();
        }
        cache.insert(key, (body.clone(), t.clone()));
        t
    }

    /// Body `b`'s [`NameIndex`], kept for the body itself across queries.
    fn name_index(&self, b: usize) -> Arc<NameIndex> {
        let body = &self.st.ctx.bodies.untracked()[b];
        let key = Arc::as_ptr(body) as usize;
        if let Some((_, ix)) = self.st.name_index.lock().get(&key) {
            return ix.clone();
        }
        let ix = Arc::new(NameIndex::of(body));
        let mut cache = self.st.name_index.lock();
        if cache.len() >= 8192 {
            cache.clear();
        }
        cache.insert(key, (body.clone(), ix.clone()));
        ix
    }

    /// Whether an entity exists in Onshape's topology (see [`Topo::shown_edges`]).
    pub fn shown(&self, e: &Ent) -> bool {
        match e {
            Ent::Edge(b, i) => self.topo(*b).shown_edges.get(*i).copied().unwrap_or(true),
            Ent::Vertex(b, i) => self.topo(*b).shown_vertices.get(*i).copied().unwrap_or(true),
            _ => true,
        }
    }

    // ------------------------------------------------------------ enumeration

    /// Every entity of a type (or of all types), in a stable order.
    pub fn all(&self, t: Option<EType>) -> Vec<Ent> {
        self.all_with(t, false)
    }

    /// [`Self::all`], with the hidden bodies (sheet metal definitions) if
    /// `hidden` (attribute queries find them; live dom/fs-smprobe2-*.txt).
    pub fn all_with(&self, t: Option<EType>, hidden: bool) -> Vec<Ent> {
        let ctx = &self.st.ctx;
        let shown = |b: &usize| hidden || !self.st.hidden.contains(&ctx.bodies[*b].id);
        let mut out = Vec::new();
        let want = |x: EType| t.is_none_or(|t| t == x);
        if want(EType::Body) {
            out.extend((0..ctx.bodies.len()).filter(shown).map(Ent::Body));
            for (s, sk) in ctx.sketches.iter().enumerate() {
                out.extend(sketch_bodies(sk, s));
            }
            out.extend((0..ctx.connectors.len()).map(Ent::Connector));
            out.extend((0..ctx.planes.len()).map(Ent::PlaneBody));
        }
        if want(EType::Face) {
            out.extend([DatumPlane::Top, DatumPlane::Front, DatumPlane::Right].map(Ent::Datum));
            out.extend((0..ctx.planes.len()).map(Ent::Plane));
            for (b, body) in ctx.bodies.iter().enumerate().filter(|(b, _)| shown(b)) {
                out.extend((0..body.faces.len()).map(|f| Ent::Face(b, f)));
            }
            for (s, sk) in ctx.sketches.iter().enumerate().filter(|(_, sk)| sk.holds_sheet()) {
                out.extend(sk.regions.iter().map(|r| Ent::SketchRegion(s, encode_region_key(&r.id))));
            }
        }
        if want(EType::Edge) {
            for (b, body) in ctx.bodies.iter().enumerate().filter(|(b, _)| shown(b)) {
                out.extend((0..body.edges.len()).map(|e| Ent::Edge(b, e)));
            }
            for s in 0..ctx.sketches.len() {
                out.extend(self.sketch_owned(s, EType::Edge));
            }
        }
        if want(EType::Vertex) {
            out.push(Ent::Origin);
            out.extend((0..ctx.points.len()).map(Ent::Point));
            for (b, body) in ctx.bodies.iter().enumerate().filter(|(b, _)| shown(b)) {
                out.extend((0..body.vertices.len()).map(|v| Ent::Vertex(b, v)));
            }
            for s in 0..ctx.sketches.len() {
                out.extend(self.sketch_owned(s, EType::Vertex));
            }
        }
        out
    }

    /// Entities owned by a body entity.
    fn owned(&self, owner: &Ent, t: Option<EType>) -> Vec<Ent> {
        let want = |x: EType| t.is_none_or(|t| t == x);
        let mut out = Vec::new();
        match owner {
            Ent::Body(b) => {
                let body = &self.st.ctx.bodies[*b];
                if want(EType::Body) {
                    out.push(Ent::Body(*b));
                }
                if want(EType::Face) {
                    out.extend(creation_order(&body.faces).into_iter().map(|f| Ent::Face(*b, f)));
                }
                if want(EType::Edge) {
                    out.extend((0..body.edges.len()).map(|e| Ent::Edge(*b, e)));
                }
                if want(EType::Vertex) {
                    out.extend((0..body.vertices.len()).map(|v| Ent::Vertex(*b, v)));
                }
            }
            Ent::SketchSheet(s) => {
                if want(EType::Body) {
                    out.push(owner.clone());
                }
                let pieces = self.region_pieces(*s);
                if want(EType::Face) {
                    out.extend(self.st.ctx.sketches[*s].regions.iter().map(|r| Ent::SketchRegion(*s, encode_region_key(&r.id))));
                }
                if want(EType::Edge) {
                    out.extend((0..pieces.edges.len() as u32).map(|i| Ent::SketchRegionEdge(*s, i)));
                }
                if want(EType::Vertex) {
                    out.extend((0..pieces.verts.len() as u32).map(|i| Ent::SketchRegionVertex(*s, i)));
                }
            }
            Ent::SketchWire(s, c) => {
                if want(EType::Body) {
                    out.push(owner.clone());
                }
                if want(EType::Edge) {
                    out.push(Ent::SketchEdge(*s, *c));
                }
                if want(EType::Vertex) && open_curve(&self.st.ctx.sketches[*s].sketch, *c) {
                    out.extend([Ent::SketchCurveEnd(*s, *c, 0), Ent::SketchCurveEnd(*s, *c, 1)]);
                }
            }
            Ent::SketchPointBody(s, p) => {
                if want(EType::Body) {
                    out.push(owner.clone());
                }
                if want(EType::Vertex) {
                    out.push(Ent::SketchVertex(*s, *p));
                }
            }
            other => {
                if want(entity_type_of(other)) {
                    out.push(other.clone());
                }
            }
        }
        out
    }

    pub fn owner(&self, e: &Ent) -> Option<Ent> {
        match e {
            // Construction planes and points are bodies of their own in
            // Onshape (std draft splits by `qOwnerBody` of an `opPlane`).
            Ent::Body(_) | Ent::SketchSheet(_) | Ent::SketchWire(..) | Ent::SketchPointBody(..) | Ent::Connector(_) | Ent::PlaneBody(_) | Ent::Point(_) | Ent::Datum(_) | Ent::Origin => Some(e.clone()),
            Ent::Plane(i) => Some(Ent::PlaneBody(*i)),
            Ent::Face(b, _) | Ent::Edge(b, _) | Ent::Vertex(b, _) => Some(Ent::Body(*b)),
            Ent::SketchRegion(s, _) | Ent::SketchRegionEdge(s, _) | Ent::SketchRegionVertex(s, _) => Some(Ent::SketchSheet(*s)),
            Ent::SketchEdge(s, c) | Ent::SketchCurveEnd(s, c, _) => Some(Ent::SketchWire(*s, *c)),
            // A point that is only curves' ends (picked as a sketch point): the first of those curves' wires.
            Ent::SketchVertex(s, p) => {
                if !self.curve_ends(*s).contains(p) {
                    return Some(Ent::SketchPointBody(*s, *p));
                }
                let sk = &self.st.ctx.sketches[*s];
                sk.sketch.entities().find(|c| sk.holds(c.id.0) && c.endpoints().is_some_and(|(a, b)| a.0 == *p || b.0 == *p)).map(|c| Ent::SketchWire(*s, c.id.0))
            }
        }
    }

    /// A sketch piece's points that are only its curves' ends (no bodies of their own), once a piece.
    pub fn curve_ends(&self, s: usize) -> Arc<HashSet<u32>> {
        let sk = &self.st.ctx.sketches[s];
        let key = (sk.feature, Arc::as_ptr(sk) as usize);
        if let Some(e) = self.st.curve_ends.lock().get(&key) {
            return e.clone();
        }
        let e = Arc::new(sk.curve_end_points());
        self.st.curve_ends.lock().insert(key, e.clone());
        e
    }

    /// A sketch piece's faces, edges or vertices: its regions' (if it holds the sheet), its curves' and points'.
    pub fn sketch_owned(&self, s: usize, t: EType) -> Vec<Ent> {
        let sk = &self.st.ctx.sketches[s];
        let mut out = Vec::new();
        let ends = self.curve_ends(s);
        match t {
            EType::Edge => out.extend(sk.sketch.entities().filter(|e| e.kind().is_curve() && sk.holds(e.id.0)).map(|e| Ent::SketchEdge(s, e.id.0))),
            EType::Vertex => {
                out.extend(sk.sketch.entities().filter(|e| open_curve(&sk.sketch, e.id.0) && sk.holds(e.id.0)).flat_map(|e| [Ent::SketchCurveEnd(s, e.id.0, 0), Ent::SketchCurveEnd(s, e.id.0, 1)]));
                out.extend(sk.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point && !ends.contains(&e.id.0) && sk.holds(e.id.0)).map(|e| Ent::SketchVertex(s, e.id.0)));
            }
            _ => {}
        }
        if sk.holds_sheet() {
            out.extend(self.owned(&Ent::SketchSheet(s), Some(t)).into_iter().filter(|e| entity_type_of(e) == t));
        }
        out
    }

    // ------------------------------------------------------------ references

    pub fn to_ref(&self, e: &Ent) -> Reference {
        let ctx = &self.st.ctx;
        match e {
            Ent::Body(b) => Reference::Body(ctx.bodies[*b].id.clone()),
            Ent::Face(b, f) => Reference::Face((*ctx.bodies[*b].faces[*f]).clone()),
            Ent::Edge(b, i) => Reference::Edge((*ctx.bodies[*b].edges[*i]).clone()),
            Ent::Vertex(b, i) => Reference::Vertex((*ctx.bodies[*b].vertices[*i]).clone()),
            Ent::Origin => Reference::Origin,
            Ent::Datum(p) => Reference::Plane(*p),
            Ent::Plane(i) | Ent::PlaneBody(i) => ctx.planes[*i].reference.clone(),
            Ent::Connector(i) => ctx.connectors[*i].reference.clone(),
            Ent::Point(i) => ctx.points[*i].reference.clone(),
            Ent::SketchSheet(s) => Reference::SketchBody { sketch: ctx.sketches[*s].feature, entity: None },
            Ent::SketchWire(s, id) | Ent::SketchPointBody(s, id) => Reference::SketchBody { sketch: ctx.sketches[*s].feature, entity: Some(*id) },
            Ent::SketchRegion(s, key) => Reference::SketchRegion { sketch: ctx.sketches[*s].feature, key: key.clone() },
            Ent::SketchEdge(s, id) | Ent::SketchVertex(s, id) => Reference::SketchEntity { sketch: ctx.sketches[*s].feature, entity: *id },
            Ent::SketchRegionEdge(s, i) => {
                let pieces = self.region_pieces(*s);
                let p = &pieces.edges[*i as usize];
                Reference::SketchRegionEdge { sketch: ctx.sketches[*s].feature, region: p.region.clone(), edge: p.at, end: None }
            }
            Ent::SketchCurveEnd(s, id, end) => Reference::SketchCurveEnd { sketch: ctx.sketches[*s].feature, entity: *id, end: *end },
            Ent::SketchRegionVertex(s, i) => {
                let pieces = self.region_pieces(*s);
                let (_, j, end) = pieces.verts[*i as usize];
                let p = &pieces.edges[j as usize];
                Reference::SketchRegionEdge { sketch: ctx.sketches[*s].feature, region: p.region.clone(), edge: p.at, end: Some(end) }
            }
        }
    }

    /// Entities a persistent reference resolves to now.
    pub fn from_ref(&self, r: &Reference) -> Vec<Ent> {
        let ctx = &self.st.ctx;
        // A sketch's piece holding its regions, or an entity's body.
        let sheet_piece = |f: crate::reference::FeatureId| ctx.sketches.iter().position(|s| s.feature == f && s.holds_sheet());
        let holding = |f: crate::reference::FeatureId, e: u32| ctx.sketches.iter().position(|s| s.feature == f && s.holds(e));
        match r {
            Reference::Body(id) => {
                let mut out: Vec<Ent> = ctx.bodies.iter().position(|b| &b.id == id).map(Ent::Body).into_iter().collect();
                // The pieces a split made of it.
                if !self.st.split_from.is_empty() {
                    let from = |b: &BodyId| std::iter::successors(self.st.split_from.get(b), |x| self.st.split_from.get(*x)).take(64).any(|x| x == id);
                    out.extend(ctx.bodies.iter().enumerate().filter(|(_, b)| &b.id != id && from(&b.id)).map(|(i, _)| Ent::Body(i)));
                }
                out
            }
            // Only the bodies that may hold it ([`NameIndex`]): a lookup asks every body otherwise, each without the
            // name scanning all its entities for split relatives (thousands of bodies after a pattern).
            Reference::Face(n) => {
                let want = Wanted::of(n, NameKind::Face);
                ctx.bodies.iter().enumerate().filter(|(b, _)| self.name_index(*b).may_hold(&want)).flat_map(|(b, body)| body.find_faces(n).into_iter().map(move |f| Ent::Face(b, f))).collect()
            }
            Reference::Edge(n) => {
                let want = Wanted::of(n, NameKind::Edge);
                ctx.bodies.iter().enumerate().filter(|(b, _)| self.name_index(*b).may_hold(&want)).flat_map(|(b, body)| body.find_edges(n).into_iter().map(move |f| Ent::Edge(b, f))).collect()
            }
            Reference::Vertex(n) => {
                let want = Wanted::of(n, NameKind::Vertex);
                ctx.bodies.iter().enumerate().filter(|(b, _)| self.name_index(*b).may_hold(&want)).flat_map(|(b, body)| body.find_vertices(n).into_iter().map(move |f| Ent::Vertex(b, f))).collect()
            }
            Reference::Origin => vec![Ent::Origin],
            Reference::Plane(p) => vec![Ent::Datum(*p)],
            // A whole sketch: its bodies (of all its pieces).
            Reference::Sketch(f) => ctx.sketches.iter().enumerate().filter(|(_, s)| s.feature == *f).flat_map(|(i, s)| sketch_bodies(s, i)).collect(),
            Reference::SketchBody { sketch, entity: None } => sheet_piece(*sketch).map(Ent::SketchSheet).into_iter().collect(),
            Reference::SketchBody { sketch, entity: Some(e) } => {
                let Some(s) = holding(*sketch, *e) else { return Vec::new() };
                match ctx.sketches[s].sketch.kind(noshape_sketch::EntityId(*e)) {
                    Some(noshape_sketch::EntityKind::Point) => vec![Ent::SketchPointBody(s, *e)],
                    Some(_) => vec![Ent::SketchWire(s, *e)],
                    None => Vec::new(),
                }
            }
            Reference::SketchRegion { sketch, key } => {
                let Some(s) = sheet_piece(*sketch) else { return Vec::new() };
                let sk = &ctx.sketches[s];
                if sk.regions.iter().any(|r| encode_region_key(&r.id) == *key) {
                    return vec![Ent::SketchRegion(s, key.clone())];
                }
                // A region whose boundary changed: match by boundary entities.
                let Some(want) = decode_region_key(key) else { return Vec::new() };
                let mut ents = want.entities();
                ents.sort();
                sk.regions
                    .iter()
                    .filter(|r| {
                        let mut e = r.id.entities();
                        e.sort();
                        e == ents
                    })
                    .map(|r| Ent::SketchRegion(s, encode_region_key(&r.id)))
                    .collect()
            }
            Reference::SketchEntity { sketch, entity } => {
                let Some(s) = holding(*sketch, *entity) else { return Vec::new() };
                match ctx.sketches[s].sketch.kind(noshape_sketch::EntityId(*entity)) {
                    Some(noshape_sketch::EntityKind::Point) => vec![Ent::SketchVertex(s, *entity)],
                    Some(_) => vec![Ent::SketchEdge(s, *entity)],
                    None => Vec::new(),
                }
            }
            Reference::SketchCurveEnd { sketch, entity, end } => {
                let Some(s) = holding(*sketch, *entity) else { return Vec::new() };
                if open_curve(&ctx.sketches[s].sketch, *entity) { vec![Ent::SketchCurveEnd(s, *entity, *end)] } else { Vec::new() }
            }
            Reference::SketchRegionEdge { sketch, region, edge, end } => {
                let Some(s) = sheet_piece(*sketch) else { return Vec::new() };
                let pieces = self.region_pieces(s);
                let Some(&j) = pieces.of_region.get(region).and_then(|l| l.get(*edge as usize)) else { return Vec::new() };
                match end {
                    None => vec![Ent::SketchRegionEdge(s, j)],
                    Some(end) => {
                        let Ok((a, b)) = pieces.edges[j as usize].shape.endpoints() else { return Vec::new() };
                        let at = if *end == 0 { a } else { b };
                        pieces.verts.iter().position(|(p, ..)| p.distance(at) < 1e-9).map(|v| Ent::SketchRegionVertex(s, v as u32)).into_iter().collect()
                    }
                }
            }
            Reference::QueryVariable(name) => ctx.query_variables.get(name).map(|v| v.iter().flat_map(|r| self.from_ref(r)).collect()).unwrap_or_default(),
            other => {
                if let Some(i) = ctx.planes.iter().position(|p| &p.reference == other) {
                    return vec![Ent::Plane(i)];
                }
                if let Some(i) = ctx.connectors.iter().position(|p| &p.reference == other) {
                    return vec![Ent::Connector(i)];
                }
                if let Some(i) = ctx.points.iter().position(|p| &p.reference == other) {
                    return vec![Ent::Point(i)];
                }
                Vec::new()
            }
        }
    }

    // ------------------------------------------------------------ geometry

    /// Kernel shape of a body, face, edge or vertex (sketch curves and
    /// regions are built on demand).
    pub fn shape(&self, e: &Ent) -> Option<Shape> {
        let ctx = &self.st.ctx;
        match e {
            Ent::Body(b) => Some(ctx.bodies[*b].shape.clone()),
            Ent::Face(b, f) => ctx.bodies[*b].shape.sub_shape(ShapeType::Face, *f).ok(),
            Ent::Edge(b, i) => ctx.bodies[*b].shape.sub_shape(ShapeType::Edge, *i).ok(),
            Ent::Vertex(b, i) => ctx.bodies[*b].shape.sub_shape(ShapeType::Vertex, *i).ok(),
            Ent::Origin => k::make_vertex(DVec3::ZERO).ok(),
            Ent::Point(i) => k::make_vertex(ctx.points[*i].position).ok(),
            Ent::SketchVertex(s, id) => {
                let sk = &ctx.sketches[*s];
                k::make_vertex(sk.frame.to_world(sk.sketch.point(noshape_sketch::EntityId(*id))?)).ok()
            }
            Ent::SketchEdge(s, id) => {
                let sk = &ctx.sketches[*s];
                sketch_edge_shape(sk, *id)
            }
            Ent::SketchRegionEdge(s, i) => self.region_pieces(*s).edges.get(*i as usize).map(|p| p.shape.clone()),
            Ent::SketchRegionVertex(s, i) => self.region_pieces(*s).verts.get(*i as usize).and_then(|(p, ..)| k::make_vertex(*p).ok()),
            Ent::SketchCurveEnd(s, id, end) => {
                let sk = &ctx.sketches[*s];
                let (a, b) = sk.sketch.endpoints(noshape_sketch::EntityId(*id))?;
                k::make_vertex(sk.frame.to_world(sk.sketch.point(if *end == 0 { a } else { b })?)).ok()
            }
            Ent::SketchRegion(s, key) => {
                let sk = &ctx.sketches[*s];
                let region = sk.regions.iter().find(|r| encode_region_key(&r.id) == *key)?;
                crate::regen::eval::region_profile(sk, region, crate::reference::Source::Index(0)).ok().map(|p| p.shape)
            }
            Ent::SketchSheet(s) => {
                let sk = &ctx.sketches[*s];
                let faces: Vec<Shape> = sk.regions.iter().filter_map(|r| crate::regen::eval::region_profile(sk, r, crate::reference::Source::Index(0)).ok().map(|p| p.shape)).collect();
                Some(Shape::compound(&faces))
            }
            Ent::SketchWire(s, id) => sketch_edge_shape(&ctx.sketches[*s], *id),
            Ent::SketchPointBody(s, id) => {
                let sk = &ctx.sketches[*s];
                k::make_vertex(sk.frame.to_world(sk.sketch.point(noshape_sketch::EntityId(*id))?)).ok()
            }
            Ent::Datum(_) | Ent::Plane(_) | Ent::PlaneBody(_) | Ent::Connector(_) => None,
        }
    }

    /// Plane of a planar entity (datum and construction planes, planar
    /// faces, sketch regions, mate connectors' xy plane).
    pub fn plane(&self, e: &Ent) -> Option<PlaneFrame> {
        let ctx = &self.st.ctx;
        match e {
            Ent::Datum(DatumPlane::Top) => Some(PlaneFrame::TOP),
            Ent::Datum(DatumPlane::Front) => Some(PlaneFrame::FRONT),
            Ent::Datum(DatumPlane::Right) => Some(PlaneFrame::RIGHT),
            Ent::Plane(i) | Ent::PlaneBody(i) => Some(ctx.planes[*i].frame),
            Ent::SketchRegion(s, _) | Ent::SketchSheet(s) => Some(ctx.sketches[*s].frame),
            Ent::Connector(i) => {
                let c = &ctx.connectors[*i];
                Some(PlaneFrame { origin: c.origin, x: c.x, y: c.z.cross(c.x), normal: c.z })
            }
            Ent::Face(..) => {
                let f = self.shape(e)?;
                let pl = f.face_plane().ok()??;
                let n = pl.unit_normal();
                // Onshape puts the origin at the middle of the face's parameter box (live FS probe:
                // the 20 mm cube's x=0 face gives (0, 10, 10) mm).
                let origin = f.uv_bounds().and_then(|[u0, u1, v0, v1]| f.point_at_uv(0.5 * (u0 + u1), 0.5 * (v0 + v1))).unwrap_or(pl.origin);
                let x = ctx.planar_face_x(&pl);
                Some(PlaneFrame { origin, x, y: n.cross(x), normal: n })
            }
            _ => None,
        }
    }

    /// `GeometryType` member name.
    pub fn geometry_type(&self, e: &Ent) -> Option<&'static str> {
        match e {
            Ent::Datum(_) | Ent::Plane(_) | Ent::SketchRegion(..) => Some("PLANE"),
            Ent::Face(..) => Some(match self.shape(e)?.face_surface().ok()? {
                k::Surface::Plane(_) => "PLANE",
                k::Surface::Cylinder { .. } => "CYLINDER",
                k::Surface::Cone { .. } => "CONE",
                k::Surface::Sphere { .. } => "SPHERE",
                k::Surface::Torus { .. } => "TORUS",
                k::Surface::Revolution { .. } => "REVOLVED",
                k::Surface::Extrusion { .. } => "EXTRUDED",
                _ => "OTHER_SURFACE",
            }),
            Ent::SketchEdge(s, id) => {
                let c = self.st.ctx.sketches[*s].sketch.curve(noshape_sketch::EntityId(*id))?;
                Some(match c {
                    noshape_sketch::Curve2::Line { .. } => "LINE",
                    noshape_sketch::Curve2::Circle { .. } => "CIRCLE",
                    noshape_sketch::Curve2::Arc { .. } => "ARC",
                    _ => "OTHER_CURVE",
                })
            }
            Ent::Edge(..) | Ent::SketchRegionEdge(..) => {
                let info = self.shape(e)?.edge_info().ok()?;
                Some(match info.curve {
                    k::Curve::Line { .. } => "LINE",
                    k::Curve::Circle { .. } if info.closed => "CIRCLE",
                    k::Curve::Circle { .. } => "ARC",
                    _ => "OTHER_CURVE",
                })
            }
            _ => None,
        }
    }

    /// `BodyType` member name of an entity's owner body.
    pub fn body_type(&self, e: &Ent) -> &'static str {
        match self.owner(e) {
            Some(Ent::Body(b)) => match self.st.ctx.bodies[b].kind {
                BodyKind::Solid => "SOLID",
                BodyKind::Surface => "SHEET",
                BodyKind::Wire => "WIRE",
                BodyKind::Point => "POINT",
            },
            Some(Ent::Connector(_)) => "MATE_CONNECTOR",
            Some(Ent::SketchSheet(_)) => "SHEET",
            Some(Ent::SketchWire(..)) => "WIRE",
            Some(Ent::SketchPointBody(..)) => "POINT",
            _ => match e {
                Ent::Datum(_) | Ent::Plane(_) | Ent::PlaneBody(_) => "SHEET",
                _ => "POINT",
            },
        }
    }

    pub fn is_construction(&self, e: &Ent) -> bool {
        match e {
            Ent::Datum(_) | Ent::Plane(_) | Ent::PlaneBody(_) | Ent::Origin | Ent::Point(_) => true,
            Ent::SketchEdge(s, id) | Ent::SketchVertex(s, id) | Ent::SketchWire(s, id) | Ent::SketchPointBody(s, id) => self.st.ctx.sketches[*s].sketch.entity(noshape_sketch::EntityId(*id)).is_some_and(|x| x.construction),
            _ => false,
        }
    }

    pub fn is_sketch(&self, e: &Ent) -> bool {
        matches!(e, Ent::SketchSheet(_) | Ent::SketchWire(..) | Ent::SketchPointBody(..) | Ent::SketchRegion(..) | Ent::SketchEdge(..) | Ent::SketchVertex(..) | Ent::SketchRegionEdge(..) | Ent::SketchRegionVertex(..) | Ent::SketchCurveEnd(..))
    }

    /// Sketch bodies too in a Part Studio loaded for a derive (std derive
    /// keeps them there: a form studio's flat sketch comes in with its
    /// tools, live 1616's Circle emboss flat); elsewhere std's sheet metal
    /// counts on them not being parts.
    pub fn is_modifiable(&self, e: &Ent) -> bool {
        // A sketch's regions, curves and points are this studio's own (live
        // G34 c2: thickening two sketches' regions reports nothing; std's
        // thicken counts unmodifiable ones as in-context faces). Sketch
        // bodies only in a studio loaded for a derive (else std's sheet
        // metal takes them for parts).
        matches!(e, Ent::Body(_) | Ent::Face(..) | Ent::Edge(..) | Ent::Vertex(..) | Ent::SketchRegion(..) | Ent::SketchEdge(..) | Ent::SketchVertex(..)) || (self.st.loaded && matches!(e, Ent::SketchSheet(_) | Ent::SketchWire(..) | Ent::SketchPointBody(..)))
    }

    /// A representative point (vertex position, curve / face / body centroid).
    pub fn point(&self, e: &Ent) -> Option<DVec3> {
        match e {
            Ent::Origin => Some(DVec3::ZERO),
            Ent::Point(i) => Some(self.st.ctx.points[*i].position),
            Ent::Connector(i) => Some(self.st.ctx.connectors[*i].origin),
            Ent::Datum(_) | Ent::Plane(_) | Ent::PlaneBody(_) => self.plane(e).map(|p| p.origin),
            _ => {
                let s = self.shape(e)?;
                match entity_type_of(e) {
                    EType::Vertex => s.vertex_point().ok(),
                    _ => s.centroid().ok().or_else(|| s.bounding_box(false).ok().map(|b| b.center())),
                }
            }
        }
    }

    /// Points on a face, edge or vertex (ends and middle of an edge, an
    /// inner point of a face).
    pub fn snapshot(&self, e: &Ent) -> Option<Snapshot> {
        let etype = entity_type_of(e);
        // Sketch curves too: std tracks a sketch line onto the edge a
        // surface made from it (sheet metal flange's bend line).
        let shape = match e {
            Ent::Face(..) | Ent::Edge(..) | Ent::Vertex(..) | Ent::SketchEdge(..) | Ent::SketchVertex(..) | Ent::SketchRegionEdge(..) | Ent::SketchRegionVertex(..) | Ent::SketchCurveEnd(..) => self.shape(e)?,
            _ => return None,
        };
        let points = match etype {
            EType::Vertex => vec![shape.vertex_point().ok()?],
            EType::Edge => {
                let i = shape.edge_info().ok()?;
                [0.0, 0.5, 1.0].iter().filter_map(|t| shape.point_at(i.first_param + (i.last_param - i.first_param) * t).ok()).collect()
            }
            _ => vec![self.face_point(&shape)?],
        };
        let face = (etype == EType::Face).then(|| shape.clone());
        Some(Snapshot { etype, points, body_type: self.body_type(e), sketch: self.is_sketch(e), face })
    }

    /// A point of face `f`: its parameters' middle when on it, else a few others, else the nearest to the middle
    /// (the projection's search along the edges: ~0.4 ms a face, std sheet metal's thousands), once a face
    /// (tracking looks for a split face's pieces among every face, each time: model-plus's Extrude 4).
    /// [`all_on`], a face's answers cached (tracking asks the same points of the same faces for every query).
    pub(super) fn all_on(&self, s: &k::Shape, points: &[DVec3], tol: f64) -> bool {
        if s.shape_type() != k::ShapeType::Face || tol != 1e-7 {
            return all_on(s, points, tol);
        }
        points.iter().all(|p| {
            let key = (s.clone(), [(p.x * 1e9).round() as i64, (p.y * 1e9).round() as i64, (p.z * 1e9).round() as i64]);
            if let Some(on) = self.st.on_face.lock().get(&key) {
                return *on;
            }
            let on = all_on(s, std::slice::from_ref(p), tol);
            self.st.on_face.lock().insert(key, on);
            on
        })
    }

    fn piece_on_old(&self, old: &k::Shape, piece: &k::Shape, m: DVec3, tol: f64) -> bool {
        let key = (old.clone(), piece.clone());
        if let Some(on) = self.st.pieces_on_old.lock().get(&key) {
            return *on;
        }
        let on = old.contains_point(m, tol).unwrap_or(false);
        self.st.pieces_on_old.lock().insert(key, on);
        on
    }

    pub fn face_point(&self, f: &k::Shape) -> Option<DVec3> {
        if let Some(p) = self.st.face_points.lock().get(f) {
            return *p;
        }
        let point = || {
            let [u0, u1, v0, v1] = f.uv_bounds().ok()?;
            let at = |a: f64, b: f64| f.point_at_uv(u0 + (u1 - u0) * a, v0 + (v1 - v0) * b).ok();
            let inside = [(0.5, 0.5), (0.25, 0.25), (0.75, 0.75), (0.25, 0.75), (0.75, 0.25)].into_iter().filter_map(|(a, b)| at(a, b)).find(|p| f.contains_point(*p, 1e-9).unwrap_or(false));
            match inside {
                Some(p) => Some(p),
                None => Some(f.project_point(at(0.5, 0.5)?).ok()?.point),
            }
        };
        let p = point();
        self.st.face_points.lock().insert(f.clone(), p);
        p
    }

    /// Sketch `s`'s region edges and vertices ([`RegionPieces`]), worked out once a sketch.
    pub fn region_pieces(&self, s: usize) -> Arc<RegionPieces> {
        let sk = &self.st.ctx.sketches[s];
        let key = (sk.feature, sk.regions.as_ptr() as usize, sk.regions.len());
        if let Some(p) = self.st.region_pieces.lock().get(&key) {
            return p.clone();
        }
        let p = Arc::new(RegionPieces::of(sk));
        self.st.region_pieces.lock().insert(key, p.clone());
        p
    }

    /// A body's face, edge or vertex box (as [`Self::shape_box`]) without making its shape: each body's boxes of a
    /// type at once, kept for the body itself. `None`: not a body's face, edge or vertex.
    pub fn sub_box(&self, e: &Ent) -> Option<Option<k::BoundingBox>> {
        let (b, i, t) = match *e {
            Ent::Face(b, i) => (b, i, ShapeType::Face),
            Ent::Edge(b, i) => (b, i, ShapeType::Edge),
            Ent::Vertex(b, i) => (b, i, ShapeType::Vertex),
            _ => return None,
        };
        let body = self.st.ctx.bodies.untracked().get(b)?;
        let key = (Arc::as_ptr(body) as usize, t as u8);
        if let Some((_, boxes)) = self.st.sub_boxes.lock().get(&key) {
            return boxes.get(i).copied();
        }
        let boxes: SubBoxes = Arc::new(body.shape.sub_shapes(t).iter().map(|s| s.bounding_box(false).ok()).collect());
        let found = boxes.get(i).copied();
        // (A scan over more bodies than it holds would make them all again each time: room for a pattern's thousands.)
        let mut cache = self.st.sub_boxes.lock();
        if cache.len() >= 8192 {
            cache.clear();
        }
        cache.insert(key, (body.clone(), boxes));
        found
    }

    /// A shape's box ([`k::Shape::bounding_box`]), once a shape.
    pub fn shape_box(&self, s: &k::Shape) -> Option<k::BoundingBox> {
        if let Some(b) = self.st.shape_boxes.lock().get(s) {
            return *b;
        }
        let b = s.bounding_box(false).ok();
        self.st.shape_boxes.lock().insert(s.clone(), b);
        b
    }

    /// What a transient id stands for now. Operations rename entities, and
    /// a merged sheet's edge may even take another edge's old name: prefer
    /// what lies where the entity was (its snapshot); an entity that moved
    /// (flange alignment) has nothing there, and its name is all there is.
    pub fn resolve_transient(&self, id: &str) -> Vec<Ent> {
        let found = self.st.transient_ref(id).map(|r| self.from_ref(r)).unwrap_or_default();
        let Some(snap) = self.st.snapshots.get(id) else { return found };
        if !found.is_empty() && found.iter().all(|e| snap_matches(self, snap, e)) {
            return found;
        }
        let tracked = self.tracked(snap, false, &[]);
        if tracked.is_empty() {
            // Nothing lies where it was: it moved, if its name still means it. An edge's or vertex's name is an
            // ordinal among its neighbours' (`Boundary`, `Vertex`): after a topology change it may name another (a
            // corner a cut took away named the new corner the cut made: std then took that one for an original).
            // (An edge named by its two faces keeps meaning it: std Jog's fixed edge, folded away from where its id
            // was given.)
            let ordinal = |r: &Reference| matches!(r, Reference::Edge(TopoName::Boundary { .. }) | Reference::Vertex(_));
            let renamed = matches!(snap.etype, EType::Edge | EType::Vertex) && self.st.transient_ref(id).is_some_and(|r| ordinal(r) && !self.st.same_topology(id, r));
            return if renamed { Vec::new() } else { found };
        }
        // A face's name lineage holds (only edges take each other's names):
        // its pieces by name, moved ones too (std Jog's rotated moving face),
        // with what lies where it was on their bodies (not a copy's face left there, nor another part's face it lay
        // on: Box Joint's pin face offset, then drafted).
        if snap.etype == EType::Face && !found.is_empty() {
            let bodies: Vec<Option<Ent>> = found.iter().map(|e| self.owner(e)).collect();
            let on_found = tracked.into_iter().filter(|e| bodies.contains(&self.owner(e)));
            return uniq(found.into_iter().chain(on_found).collect());
        }
        let kept: Vec<Ent> = found.into_iter().filter(|e| tracked.contains(e)).collect();
        if kept.is_empty() { tracked } else { kept }
    }

    /// What a `startTracking` id stands for now: the entity and what was
    /// made from it, e.g. the sheet edges std sheet metal CONVERT extracts
    /// from the part's bend edges (live dom/fs-smprobe3-4.txt).
    pub fn tracking(&self, id: &str, since: u32) -> Vec<Ent> {
        let mut out = self.resolve_transient(id);
        // The faces named after it: a shell's or thicken's offset of it, a region enclosed from those, their pieces
        // (Parasolid's offsets keep their faces' lineage). A face a shell moved off its plane has nothing where it
        // was: cf-lighten's Lighten tracks its tools' caps through shell, enclose and a Boolean.
        if let Some(Reference::Face(n)) = self.st.transient_ref(id) {
            for (bi, b) in self.st.ctx.bodies.iter().enumerate() {
                out.extend(b.faces.iter().enumerate().filter(|(_, m)| ***m != *n && derives_from(m, n)).map(|(fi, _)| Ent::Face(bi, fi)));
            }
        }
        if let Some(snap) = self.st.snapshots.get(id) {
            let copies: Vec<DMat4> = self.st.pattern_moves.iter().filter(|(seq, _)| *seq > since).map(|(_, m)| *m).collect();
            out.extend(self.tracked(snap, true, &copies));
            // A face used to split faces: the edges it cut.
            if snap.etype == EType::Face && let Some(p) = snap.points.first() {
                for (o, n, mids) in &self.st.split_tools {
                    if (*p - *o).dot(*n).abs() < 1e-7 {
                        let cut = |e: &Ent| self.snapshot(e).and_then(|s| s.points.get(1).copied()).is_some_and(|m| mids.iter().any(|x| x.distance(m) < 1e-7));
                        out.extend(self.all_with(Some(EType::Edge), true).into_iter().filter(cut));
                    }
                }
            }
            // A sketch vertex extruded: the straight edge it swept (std Hem
            // tracks its profile arc's end onto the arc sheet's side edge).
            if snap.sketch && snap.etype == EType::Vertex && let Some(p) = snap.points.first() {
                let swept = |e: &Ent| {
                    self.shape(e).is_some_and(|s| {
                        matches!(s.edge_info().map(|i| i.curve), Ok(k::Curve::Line { .. })) && self.all_on(&s, &[*p], 1e-7)
                    })
                };
                out.extend(self.all_with(Some(EType::Edge), true).into_iter().filter(|e| !self.is_sketch(e) && swept(e)));
            }
        }
        // What the entities became: nothing later makes a sketch entity (the tracked sketch line itself isn't one of
        // them; std writes qUnion([q, startTracking(q)]) for the originals): a Shaped Flange's tracked profile line
        // gives the edge extracted from it alone.
        out.retain(|e| !self.is_sketch(e));
        uniq(out)
    }

    /// Current entities a tracked entity became: those of its type lying on
    /// its old points (renamed or merged), or on the old edge between its
    /// ends (split pieces). With `to_sheets`, a solid's entity may also
    /// become a sheet's (extracted surfaces); also where each of `copies` (pattern transforms) put its points.
    pub(super) fn tracked(&self, snap: &Snapshot, to_sheets: bool, copies: &[DMat4]) -> Vec<Ent> {
        const TOL: f64 = 1e-7;
        // An edge opEdgeChange moved is looked for where it went too (and on through later moves).
        let mut wheres: Vec<Vec<DVec3>> = vec![snap.points.clone()];
        if snap.etype == EType::Edge {
            let same = |a: &[DVec3], b: &[DVec3]| a.len() == b.len() && (a.iter().zip(b).all(|(p, q)| p.distance(*q) < TOL) || a.iter().zip(b.iter().rev()).all(|(p, q)| p.distance(*q) < TOL));
            for (from, to) in &self.st.edge_moves {
                if wheres.iter().any(|w| same(w, from)) {
                    wheres.push(to.clone());
                }
            }
        }
        let moved: Vec<Vec<DVec3>> = copies.iter().flat_map(|m| wheres.iter().map(move |w| w.iter().map(|p| m.transform_point3(*p)).collect())).collect();
        wheres.extend(moved);
        // The old face's box and plane: a piece of it lies in its box (boxes are loose: a hundredth of its size to
        // spare) and on its plane, and its inner point is costly.
        let old_box = snap.face.as_ref().and_then(|f| f.bounding_box(false).ok());
        let old_plane = snap.face.as_ref().and_then(|f| f.face_plane().ok().flatten()).map(|p| (p.origin, p.normal.normalize()));
        let near_old = |e: &Ent| {
            let (Some(o), Some(b)) = (old_box, self.shape(e).and_then(|s| self.shape_box(&s))) else { return true };
            let slack = DVec3::splat(TOL + 0.01 * o.diagonal());
            (o.min - slack).cmple(b.min).all() && b.max.cmple(o.max + slack).all()
        };
        // What its box must reach to pass one of the tests below: hold one of `wheres`, lie by the old face, or meet
        // the old straight edge.
        let segment = (snap.etype == EType::Edge && snap.points.len() == 3).then(|| (snap.points[0].min(snap.points[2]), snap.points[0].max(snap.points[2])));
        let may_match = |e: &Ent| {
            let Some(Some(b)) = self.sub_box(e) else { return true };
            let (lo, hi) = (b.min - DVec3::splat(TOL), b.max + DVec3::splat(TOL));
            wheres.iter().any(|w| w.iter().all(|p| lo.cmple(*p).all() && p.cmple(hi).all()))
                || snap.face.is_some() && old_box.is_none_or(|o| {
                    let slack = DVec3::splat(TOL + 0.01 * o.diagonal());
                    (o.min - slack).cmple(b.min).all() && b.max.cmple(o.max + slack).all()
                })
                || segment.is_some_and(|(a, z)| lo.cmple(z).all() && a.cmple(hi).all())
        };
        // Hidden bodies too: std sheet metal tracks its (hidden) definition.
        self.all_with(Some(snap.etype), true)
            .into_iter()
            // Its body's box of it first: making each entity's shape cost more than the test (c2-sm-louver's
            // Extrude 3 tracks over thousands of faces, 172 times).
            .filter(|e| may_match(e))
            // Solids only to solids; sheets, wires and sketches among
            // themselves (std follows a sketch line onto a sheet's edge).
            .filter(|e| {
                let t = self.body_type(e);
                (t == "SOLID") == (snap.body_type == "SOLID") || (to_sheets && snap.body_type == "SOLID" && t == "SHEET")
            })
            // Nothing in a sketch is made from a body's entity: a sheet edge isn't followed onto the sketch line it
            // was built from (std Hem's moved edge would come back twice).
            .filter(|e| snap.sketch || !self.is_sketch(e))
            .filter(|e| {
                // In its box first (a cached box: tracking scans every face, often).
                let in_box = |b: Option<k::BoundingBox>, w: &[DVec3]| b.is_none_or(|b| w.iter().all(|p| (b.min - DVec3::splat(TOL)).cmple(*p).all() && p.cmple(b.max + DVec3::splat(TOL)).all()));
                if self.shape(e).is_some_and(|s| {
                    let b = self.shape_box(&s);
                    wheres.iter().any(|w| in_box(b, w) && self.all_on(&s, w, TOL))
                }) {
                    return true;
                }
                // A piece of a split face (std Jog clips its face away from the
                // old face's middle): an inner point on the old face.
                if let Some(old) = &snap.face
                    && near_old(e)
                    && let Some(piece) = self.shape(e)
                    && let Some(m) = self.face_point(&piece)
                    && old_box.is_none_or(|o| (o.min - DVec3::splat(TOL)).cmple(m).all() && m.cmple(o.max + DVec3::splat(TOL)).all())
                    && old_plane.is_none_or(|(o, n)| (m - o).dot(n).abs() <= TOL)
                    && self.piece_on_old(old, &piece, m, TOL)
                {
                    return true;
                }
                // A piece of a split straight edge: its middle on the old segment.
                if snap.etype == EType::Edge && snap.points.len() == 3 {
                    let (a, b) = (snap.points[0], snap.points[2]);
                    let straight = ((snap.points[1] - a).cross(b - a)).length() < TOL * (b - a).length().max(1.0);
                    if straight && let Some(m) = self.snapshot(e).and_then(|s| s.points.get(1).copied()) {
                        let t = (m - a).dot(b - a) / (b - a).length_squared();
                        return (0.0..=1.0).contains(&t) && (a + (b - a) * t).distance(m) < TOL;
                    }
                }
                false
            })
            .collect()
    }

    /// Size used by qLargest / qSmallest: length, area or volume.
    pub fn measure(&self, e: &Ent) -> Option<f64> {
        let s = self.shape(e)?;
        match entity_type_of(e) {
            EType::Edge => s.length().ok(),
            EType::Face => s.area().ok(),
            EType::Body => match self.body_type(e) {
                "SOLID" => s.volume().ok(),
                "SHEET" => k::surface_properties(&s).ok().map(|p| p.mass),
                "WIRE" => k::linear_properties(&s).ok().map(|p| p.mass),
                _ => Some(0.0),
            },
            EType::Vertex => Some(0.0),
        }
    }

    pub fn distance_to_point(&self, e: &Ent, p: DVec3) -> Option<f64> {
        if let Some(pl) = (matches!(e, Ent::Datum(_) | Ent::Plane(_) | Ent::PlaneBody(_))).then(|| self.plane(e)).flatten() {
            return Some((p - pl.origin).dot(pl.normal).abs());
        }
        let s = self.shape(e)?;
        let v = k::make_vertex(p).ok()?;
        k::distance(&s, &v).ok().map(|d| d.distance)
    }

    // ------------------------------------------------------------ creation

    /// The FeatureScript id that created an entity (`None`: unknown).
    pub fn created_by(&self, e: &Ent) -> Option<FsId> {
        let ctx = &self.st.ctx;
        let feature_id = |f: FeatureId| -> Option<FsId> {
            if is_marker(f) {
                return self.st.ops.get(&f).map(|o| o.id.clone());
            }
            Some(vec![Arc::from(f.to_string())])
        };
        let uncached = |n: &TopoName| -> Option<(u32, FsId)> {
            let own = match self.st.op_of_face(n) {
                Some(op) => Some((op.seq, op.id.clone())),
                None => n.creator().and_then(|f| feature_id(f).map(|i| (0, i))),
            };
            // A Boolean that brought it into its target, if later.
            match (own, self.st.adopted.get(n)) {
                (Some(o), Some(b)) if b.seq < o.0 => Some(o),
                (_, Some(b)) => Some((b.seq, b.id.clone())),
                (o, None) => o,
            }
        };
        let face_id = |n: &Arc<TopoName>| -> Option<(u32, FsId)> {
            let key = Arc::as_ptr(n) as usize;
            if let Some((_, found)) = self.st.face_creators.lock().get(&key) {
                return found.clone();
            }
            let found = uncached(n);
            self.st.face_creators.lock().insert(key, (n.clone(), found.clone()));
            found
        };
        let newest = |faces: &[Arc<TopoName>]| faces.iter().filter_map(face_id).max_by_key(|x| x.0).map(|x| x.1);
        match e {
            Ent::Datum(DatumPlane::Top) => Some(vec!["Top".into()]),
            Ent::Datum(DatumPlane::Front) => Some(vec!["Front".into()]),
            Ent::Datum(DatumPlane::Right) => Some(vec!["Right".into()]),
            Ent::Origin => Some(vec!["Origin".into()]),
            Ent::Body(b) => self.st.body_creator(&ctx.bodies[*b].id),
            Ent::Face(b, f) => face_id(&ctx.bodies[*b].faces[*f]).map(|x| x.1),
            // An edge an opSplitFace made (where it lies: its name follows
            // its faces).
            Ent::Edge(..) if let Some(op) = self.split_face_edge_op(e) => Some(op),
            // Edges and vertices named without faces (wire and point
            // bodies) were created with their body.
            Ent::Edge(b, i) => match &*ctx.bodies[*b].edges[*i] {
                TopoName::Edge { faces, .. } => newest(faces),
                TopoName::Boundary { face, .. } => face_id(face).map(|x| x.1),
                TopoName::Vertex { faces, .. } => newest(faces),
                other => uncached(other).map(|x| x.1),
            }
            .or_else(|| self.created_by(&Ent::Body(*b))),
            Ent::Vertex(..) if let Some(op) = self.made_vertex_op(e) => Some(op),
            Ent::Vertex(b, i) => match &*ctx.bodies[*b].vertices[*i] {
                TopoName::Vertex { faces, .. } => newest(faces),
                other => uncached(other).map(|x| x.1),
            }
            .or_else(|| self.created_by(&Ent::Body(*b))),
            Ent::SketchSheet(s) | Ent::SketchWire(s, _) | Ent::SketchPointBody(s, _) | Ent::SketchRegion(s, _) | Ent::SketchEdge(s, _) | Ent::SketchVertex(s, _) | Ent::SketchRegionEdge(s, _) | Ent::SketchRegionVertex(s, _) | Ent::SketchCurveEnd(s, ..) => feature_id(ctx.sketches[*s].feature),
            Ent::Plane(_) | Ent::PlaneBody(_) => {
                let r = self.to_ref(e);
                self.st.plane_ops.get(&r).cloned().or_else(|| r.depends_on().and_then(feature_id))
            }
            Ent::Connector(_) | Ent::Point(_) => self.to_ref(e).depends_on().and_then(feature_id),
        }
    }

    /// The last `opSplitFace` that made edge `e` (its midpoint on one it made, in a body it cut; not a tool lying
    /// on it).
    /// The last operation that made vertex `e` where it is ([`CtxState::made_vertices`]), in a body it changed.
    fn made_vertex_op(&self, e: &Ent) -> Option<FsId> {
        let Ent::Vertex(b, _) = e else { return None };
        let body = &self.st.ctx.bodies[*b].id;
        let mut made = self.st.made_vertices.iter().rev().filter(|(_, owners, _)| owners.contains(body)).peekable();
        made.peek()?;
        let p = self.shape(e)?.vertex_point().ok()?;
        made.find(|(_, _, points)| points.iter().any(|q| q.distance(p) < 1e-7)).map(|(id, _, _)| id.clone())
    }

    fn split_face_edge_op(&self, e: &Ent) -> Option<FsId> {
        let Ent::Edge(b, _) = e else { return None };
        let body = &self.st.ctx.bodies[*b].id;
        let touched = self.st.touched.get(body).copied();
        // A body another operation made after the split has its own edges, even where the split's lie (a fill
        // patching the hole its edges bound).
        let made = self.st.body_ops.get(body).and_then(|op| self.st.ops.values().find(|o| &o.id == op).map(|o| (op, o.seq)));
        let later = |id: &FsId, seq: u32| made.is_some_and(|(op, s)| op != id && s > seq);
        let mut splits = self.st.split_face_edges.iter().rev().filter(|(id, owners, _, seq)| owners.contains(body) || (touched.is_some_and(|t| t >= *seq) && !later(id, *seq))).peekable();
        splits.peek()?;
        let s = self.shape(e)?;
        let info = s.edge_info().ok()?;
        let m = s.point_at((info.first_param + info.last_param) * 0.5).ok()?;
        splits.find(|(_, _, mids, _)| mids.iter().any(|p| p.distance(m) < 1e-7)).map(|(id, ..)| id.clone())
    }

    /// Start/end cap role of a face created by an extrude-like operation.
    fn cap_of(&self, e: &Ent) -> Option<(&'static str, FsId)> {
        let Ent::Face(b, f) = e else { return None };
        let n = &self.st.ctx.bodies[*b].faces[*f];
        let op = self.st.op_of_face(n)?;
        let inner = match n.root() {
            TopoName::Face { role: Role::Generated(inner), .. } => inner.root().clone(),
            _ => return None,
        };
        match inner {
            TopoName::Face { role: Role::CapStart(_), .. } => Some(("START", op.id.clone())),
            TopoName::Face { role: Role::CapEnd(_), .. } => Some(("END", op.id.clone())),
            _ => None,
        }
    }

    // ------------------------------------------------------------ adjacency

    pub fn faces_of_edge(&self, b: usize, e: usize) -> Vec<Ent> {
        self.topo(b).edge_faces.get(e).map(|v| v.iter().map(|f| Ent::Face(b, *f as usize)).collect()).unwrap_or_default()
    }

    fn vertices_of_edge(&self, b: usize, e: usize) -> Vec<Ent> {
        self.topo(b).edge_vertices.get(e).map(|v| v.iter().map(|x| Ent::Vertex(b, *x as usize)).collect()).unwrap_or_default()
    }

    fn edges_of_face(&self, b: usize, f: usize) -> Vec<Ent> {
        let t = self.topo(b);
        t.edge_faces.iter().enumerate().filter(|(_, fs)| fs.contains(&(f as u32))).map(|(e, _)| Ent::Edge(b, e)).collect()
    }

    fn vertices_of_face(&self, b: usize, f: usize) -> Vec<Ent> {
        let t = self.topo(b);
        t.vertex_faces.iter().enumerate().filter(|(_, fs)| fs.contains(&(f as u32))).map(|(v, _)| Ent::Vertex(b, v)).collect()
    }

    /// Edges and vertices bounding an entity (faces: their edges and
    /// vertices; edges: their vertices; vertices: themselves).
    pub fn boundary(&self, e: &Ent) -> (Vec<Ent>, Vec<Ent>) {
        match e {
            Ent::Face(b, f) => (self.edges_of_face(*b, *f), self.vertices_of_face(*b, *f)),
            Ent::Edge(b, i) => (vec![e.clone()], self.vertices_of_edge(*b, *i)),
            Ent::Vertex(..) => (Vec::new(), vec![e.clone()]),
            Ent::Body(b) => ((0..self.st.ctx.bodies[*b].edges.len()).map(|x| Ent::Edge(*b, x)).collect(), (0..self.st.ctx.bodies[*b].vertices.len()).map(|x| Ent::Vertex(*b, x)).collect()),
            _ => (Vec::new(), Vec::new()),
        }
    }

    /// Entities of type `t` adjacent to `e` through a shared edge
    /// (`by_vertex`: through a shared vertex).
    pub(super) fn adjacent(&self, e: &Ent, t: EType, by_vertex: bool) -> Vec<Ent> {
        // A sketch region is bounded by its sketch curves (std thin
        // extrude takes a picked region's edges).
        // A sketch region is bounded by its own edges (live 2059: not the sketch's entity edges); a region edge
        // bounds the regions holding it, between its end vertices.
        if let Ent::SketchRegion(s, _) | Ent::SketchRegionEdge(s, _) | Ent::SketchRegionVertex(s, _) = e {
            let pieces = self.region_pieces(*s);
            let edges: Vec<u32> = match e {
                Ent::SketchRegion(_, key) => pieces.of_region.get(key).cloned().unwrap_or_default(),
                Ent::SketchRegionEdge(_, i) if !by_vertex => vec![*i],
                Ent::SketchRegionEdge(_, i) => {
                    let ends = pieces.edges.get(*i as usize).map(|p| p.ends.clone()).unwrap_or_default();
                    (0..pieces.edges.len() as u32).filter(|j| pieces.edges[*j as usize].ends.iter().any(|v| ends.contains(v))).collect()
                }
                Ent::SketchRegionVertex(_, v) => (0..pieces.edges.len() as u32).filter(|j| pieces.edges[*j as usize].ends.contains(v)).collect(),
                _ => Vec::new(),
            };
            let regions = |j: &u32| pieces.edges[*j as usize].regions.iter().map(|k| Ent::SketchRegion(*s, k.clone())).collect::<Vec<_>>();
            let mut out: Vec<Ent> = match t {
                EType::Edge if matches!(e, Ent::SketchRegionEdge(..)) && !by_vertex => Vec::new(),
                EType::Edge => edges.iter().map(|j| Ent::SketchRegionEdge(*s, *j)).collect(),
                EType::Vertex => match e {
                    Ent::SketchRegionVertex(..) => vec![e.clone()],
                    _ => edges.iter().flat_map(|j| pieces.edges[*j as usize].ends.iter().map(|v| Ent::SketchRegionVertex(*s, *v))).collect(),
                },
                EType::Face => edges.iter().flat_map(regions).collect(),
                EType::Body => vec![Ent::SketchSheet(*s)],
            };
            out.retain(|x| x != e);
            return uniq(out);
        }
        // An open curve's edge has end vertices of its own (std Bridging curve infers a side's vertex from a picked
        // sketch edge); curves don't meet there (each is a wire body of its own); a fit spline's interpolation points
        // are on no edge (live 2060).
        if let Ent::SketchCurveEnd(s, id, _) = e {
            let mut out: Vec<Ent> = match t {
                EType::Edge => vec![Ent::SketchEdge(*s, *id)],
                EType::Vertex if by_vertex => vec![Ent::SketchCurveEnd(*s, *id, 0), Ent::SketchCurveEnd(*s, *id, 1)],
                EType::Body => self.owner(e).into_iter().collect(),
                _ => Vec::new(),
            };
            out.retain(|x| x != e);
            return out;
        }
        if let Ent::SketchEdge(s, id) = e
            && open_curve(&self.st.ctx.sketches[*s].sketch, *id)
        {
            let mut out: Vec<Ent> = match t {
                EType::Vertex => vec![Ent::SketchCurveEnd(*s, *id, 0), Ent::SketchCurveEnd(*s, *id, 1)],
                EType::Body => self.owner(e).into_iter().collect(),
                _ => Vec::new(),
            };
            out.retain(|x| x != e);
            return out;
        }
        if let Ent::SketchEdge(s, id) | Ent::SketchVertex(s, id) = e {
            let sk = &self.st.ctx.sketches[*s];
            let ends = |c: u32| if open_curve(&sk.sketch, c) { None } else { sk.sketch.endpoints(noshape_sketch::EntityId(c)).map(|(a, b)| [a.0, b.0]) };
            let verts: Vec<u32> = match e {
                Ent::SketchEdge(..) => ends(*id).map(|v| v.to_vec()).unwrap_or_default(),
                _ => vec![*id],
            };
            let curves = || sk.sketch.entities().filter(|x| x.kind().is_curve()).map(|x| x.id.0).filter(|c| ends(*c).is_some_and(|v| v.iter().any(|p| verts.contains(p))));
            let mut out: Vec<Ent> = match t {
                EType::Vertex => verts.iter().map(|v| Ent::SketchVertex(*s, *v)).collect(),
                EType::Edge if by_vertex || matches!(e, Ent::SketchVertex(..)) => curves().map(|c| Ent::SketchEdge(*s, c)).collect(),
                EType::Edge => Vec::new(),
                // An entity's whole edge bounds no region (live 2059: the regions have edges of their own).
                EType::Face => Vec::new(),
                EType::Body => self.owner(e).into_iter().collect(),
            };
            out.retain(|x| x != e);
            return uniq(out);
        }
        let (edges, verts) = self.boundary(e);
        let Some(b) = (match e {
            Ent::Face(b, _) | Ent::Edge(b, _) | Ent::Vertex(b, _) | Ent::Body(b) => Some(*b),
            _ => None,
        }) else {
            return Vec::new();
        };
        let topo = self.topo(b);
        let mut out = Vec::new();
        let mut push = |x: Ent| {
            if !out.contains(&x) {
                out.push(x);
            }
        };
        if by_vertex {
            for v in &verts {
                let Ent::Vertex(_, vi) = v else { continue };
                match t {
                    EType::Face => topo.vertex_faces.get(*vi).into_iter().flatten().for_each(|f| push(Ent::Face(b, *f as usize))),
                    EType::Edge => topo.vertex_edges.get(*vi).into_iter().flatten().for_each(|x| push(Ent::Edge(b, *x as usize))),
                    EType::Vertex => push(v.clone()),
                    EType::Body => push(Ent::Body(b)),
                }
            }
        } else {
            for x in &edges {
                let Ent::Edge(_, ei) = x else { continue };
                match t {
                    EType::Face => topo.edge_faces.get(*ei).into_iter().flatten().for_each(|f| push(Ent::Face(b, *f as usize))),
                    EType::Edge => push(x.clone()),
                    EType::Vertex => self.vertices_of_edge(b, *ei).into_iter().for_each(&mut push),
                    EType::Body => push(Ent::Body(b)),
                }
            }
        }
        // An entity is not adjacent to itself.
        out.retain(|x| x != e);
        out
    }
}

// ---------------------------------------------------------------- evaluation

/// A large planar face on a plane (for distance and intersection tests).
pub fn plane_face(p: &PlaneFrame) -> Result<Shape, k::KernelError> {
    k::make_plane_face(&crate::regen::eval::kernel_plane(p), -1e3, 1e3, -1e3, 1e3)
}

/// A kernel edge for any sketch curve (construction curves included).
pub fn sketch_edge_shape(sk: &crate::regen::context::SketchResult, id: u32) -> Option<Shape> {
    let curve = sk.sketch.curve(noshape_sketch::EntityId(id))?;
    let seg = crate::regen::features::helpers_a::curve_segment(&curve)?;
    k::make_edge_2d(&crate::regen::eval::kernel_plane(&sk.frame), &seg).ok()
}

impl QueryCx<'_> {
    /// Each edge of body `b` that lies along a sketch curve: (the sketch's place among the context's sketches, the
    /// curve's place in its sketch). The first sketch that has it; kept for the body (and as many sketches).
    pub fn sketch_keys(&self, b: usize) -> Arc<SketchKeys> {
        let body = self.st.ctx.bodies.untracked()[b].clone();
        let key = Arc::as_ptr(&body) as usize;
        let sketches = &self.st.ctx.sketches;
        if let Some((_, k)) = self.st.sketch_order_cache.lock().get(&key).filter(|(_, k)| k.0 == sketches.len()) {
            return k.clone();
        }
        // Sheets only: a solid's edges along a sketch curve (a part thickened from its definition) were made by
        // its own operation, in an order not known here.
        if body.kind != BodyKind::Surface {
            let k = Arc::new((sketches.len(), vec![None; body.edges.len()]));
            self.st.sketch_order_cache.lock().insert(key, (body, k.clone()));
            return k;
        }
        const TOL: f64 = 1e-7;
        // The sketches its faces were made from, and their curves (only theirs: a document's other sketches may be big).
        let face_sketches: Vec<Option<crate::reference::FeatureId>> = body.faces.iter().map(profile_sketch).collect();
        if face_sketches.iter().all(Option::is_none) {
            let k = Arc::new((sketches.len(), vec![None; body.edges.len()]));
            self.st.sketch_order_cache.lock().insert(key, (body, k.clone()));
            return k;
        }
        let curves: Vec<Vec<(u32, noshape_sketch::geom::BBox, noshape_sketch::geom::Curve2)>> = sketches
            .iter()
            .map(|sk| {
                if !face_sketches.contains(&Some(sk.feature)) {
                    return Vec::new();
                }
                sk.sketch
                    .entities()
                    .enumerate()
                    .filter(|(_, e)| e.kind().is_curve() && !e.construction)
                    .filter_map(|(i, e)| sk.sketch.curve(e.id).map(|c| (i as u32, c.bbox(), c)))
                    .collect()
            })
            .collect();
        let edge_faces = body.shape.edge_faces().unwrap_or_default();
        let keys = body
            .shape
            .edges()
            .iter()
            .enumerate()
            .map(|(ei, e)| {
                // Only the sketches its faces were made from (an extrude's cap, the sheet extracted from it): a flange's
                // top edge over the plate's outline was made by the flange (in pick order).
                let from: Vec<crate::reference::FeatureId> = edge_faces.get(ei).into_iter().flatten().filter_map(|f| face_sketches.get(*f as usize).copied().flatten()).collect();
                if from.is_empty() {
                    return None;
                }
                let info = e.edge_info().ok()?;
                let mid = e.point_at((info.first_param + info.last_param) * 0.5).ok()?;
                let pts = [info.start, mid, info.end];
                sketches.iter().zip(&curves).enumerate().filter(|(_, (sk, _))| from.contains(&sk.feature)).find_map(|(si, (sk, cs))| {
                    let f = &sk.frame;
                    let h: Vec<f64> = pts.iter().map(|p| (*p - f.origin).dot(f.normal)).collect();
                    if (h[0] - h[1]).abs() > TOL || (h[0] - h[2]).abs() > TOL {
                        return None;
                    }
                    let flat: Vec<glam::DVec2> = pts.iter().map(|p| glam::DVec2::new((*p - f.origin).dot(f.x), (*p - f.origin).dot(f.y))).collect();
                    if flat[0].distance(flat[2]) < TOL && flat[0].distance(flat[1]) < TOL {
                        return None;
                    }
                    let inside = |bb: &noshape_sketch::geom::BBox, p: glam::DVec2| p.x >= bb.min.x - TOL && p.y >= bb.min.y - TOL && p.x <= bb.max.x + TOL && p.y <= bb.max.y + TOL;
                    cs.iter().find(|(_, bb, c)| flat.iter().all(|p| inside(bb, *p) && c.closest(*p).2 < TOL)).map(|(i, _, _)| (si as u32, *i))
                })
            })
            .collect();
        let k = Arc::new((sketches.len(), keys));
        self.st.sketch_order_cache.lock().insert(key, (body, k.clone()));
        k
    }
}

/// A body's edges' sketch order keys ([`QueryCx::sketch_keys`]): the number of sketches they were looked for among,
/// and each edge's (sketch, curve) if it has one.
pub type SketchKeys = (usize, Vec<Option<(u32, u32)>>);

/// The sketch whose region a face was made from: an extrude's caps, a region's own face, and what was generated,
/// offset or copied from them or split off them (a sheet metal definition: the cap extracted).
fn profile_sketch(n: &crate::naming::Name) -> Option<crate::reference::FeatureId> {
    use crate::reference::{Role, Source, TopoName};
    match &**n {
        TopoName::Face { role, .. } => match role {
            Role::CapStart(Some(Source::Region { sketch, .. })) | Role::CapEnd(Some(Source::Region { sketch, .. })) | Role::Profile(Source::Region { sketch, .. }) => Some(*sketch),
            Role::Generated(x) | Role::Offset(x) | Role::Copy { of: x, .. } => profile_sketch(x),
            _ => None,
        },
        TopoName::Split { parent, .. } => profile_sketch(parent),
        _ => None,
    }
}

/// Whether an entity still lies on all of a snapshot's points.
impl QueryCx<'_> {
    /// Where an attributed edge or vertex of a sheet (a sheet metal
    /// definition) is, with its body: [`CtxState::remap_attributes`].
    pub fn attribute_snapshot(&self, r: &Reference) -> Option<(BodyId, Snapshot)> {
        if !matches!(r, Reference::Edge(_) | Reference::Vertex(_)) {
            return None;
        }
        let e = self.from_ref(r).into_iter().next()?;
        let Some(Ent::Body(b)) = self.owner(&e) else { return None };
        let body = &self.st.ctx.bodies[b];
        if body.kind != BodyKind::Surface {
            return None;
        }
        Some((body.id.clone(), self.snapshot(&e)?))
    }
}

/// Where an attributed entity is now, if an operation renamed it (its name
/// gone, or now naming another entity): what of its kind lies where it was (none: it's gone),
/// its pieces when the operation split it (a definition edge a cut runs across: Parasolid gives
/// every piece the attribute, and std's assignSMAttributesToNewOrSplitEntities
/// sorts them out).
pub fn relocated_pieces(q: &QueryCx, r: &Reference, snap: &Snapshot) -> Option<Vec<Ent>> {
    let found = q.from_ref(r);
    if !found.is_empty() && found.iter().all(|e| snap_matches(q, snap, e)) {
        // Still there, but under another name now (its faces renamed): the
        // entity's own name, which the attribute lookups compare (qa4 S1's
        // x = 100 bend lost its joint after the slot split the plate).
        return match found.as_slice() {
            [e] if q.to_ref(e) != *r => Some(vec![e.clone()]),
            _ => None,
        };
    }
    let pieces = q.tracked(snap, false, &[]);
    // Its name names another entity now and nothing lies where it was: it is gone, and its attributes with it
    // (Parasolid's; else they'd sit on whatever took the name: cf-sheet-metal-overcut-dogbone's corner breaks on
    // a tangent point the rebuild left out landed on other corners).
    if pieces.is_empty() && !found.is_empty() {
        return Some(Vec::new());
    }
    (!pieces.is_empty()).then_some(pieces)
}

fn snap_matches(q: &QueryCx, snap: &Snapshot, e: &Ent) -> bool {
    if entity_type_of(e) != snap.etype {
        return false;
    }
    // An edge or vertex where it was: its snapshot points again (points on it, at the same parameters), so on it;
    // no distance needed (the common case: most tracked entities didn't move).
    if matches!(snap.etype, EType::Edge | EType::Vertex) && q.snapshot(e).is_some_and(|c| c.points.len() == snap.points.len() && c.points.iter().zip(&snap.points).all(|(a, b)| a.distance(*b) < 1e-12)) {
        return true;
    }
    let Some(s) = q.shape(e) else { return false };
    // The very face the snapshot was taken of (unchanged by the operations since): its point lies on it.
    if snap.face.as_ref().is_some_and(|f| *f == s) {
        return true;
    }
    q.all_on(&s, &snap.points, 1e-7)
}

/// Whether every point lies within `tol` of `s`: its bounding box first (it holds the shape: a point farther from it
/// than `tol` is farther from the shape), the exact distance only then (the kernel's point-to-face distance projects
/// edges onto planes and classifies: most of std sheet metal's query tracking).
pub(crate) fn all_on(s: &k::Shape, points: &[DVec3], tol: f64) -> bool {
    if let Ok(b) = s.bounding_box(false)
        && points.iter().any(|p| (b.min - DVec3::splat(tol)).cmpgt(*p).any() || p.cmpgt(b.max + DVec3::splat(tol)).any())
    {
        return false;
    }
    // A face: the foot on its surface classified (BRepExtrema's distance also measures every edge: ~0.2 ms a call,
    // and tracking asks it of every face nearby).
    if s.shape_type() == k::ShapeType::Face {
        return points.iter().all(|p| s.contains_point(*p, tol).unwrap_or(false));
    }
    points.iter().all(|p| k::make_vertex(*p).ok().and_then(|v| k::distance(&v, s).ok()).is_some_and(|d| d.distance < tol))
}

/// A body's faces in Onshape's evaluateQuery order (creation order, live
/// request 1608) where we know it: a body only one Extrude made gives the
/// first profile edge's side, the start cap, the end cap, then the other
/// sides in profile order (std's Convert numbers its walls in this order;
/// Butt joint - Direction 1 follows it). Other bodies: body order.
fn creation_order(faces: &[crate::naming::Name]) -> Vec<usize> {
    use crate::reference::{Role, TopoName};
    let plain: Option<Vec<(u8, Option<&crate::reference::Source>)>> = faces
        .iter()
        .map(|n| match &**n {
            TopoName::Face { feature, role: Role::Side(s) } => Some((*feature, 0, Some(s))),
            TopoName::Face { feature, role: Role::CapStart(_) } => Some((*feature, 1, None)),
            TopoName::Face { feature, role: Role::CapEnd(_) } => Some((*feature, 2, None)),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .filter(|v| v.windows(2).all(|w| w[0].0 == w[1].0) && v.iter().filter(|x| x.1 == 1).count() == 1 && v.iter().filter(|x| x.1 == 2).count() == 1)
        .map(|v| v.into_iter().map(|(_, k, s)| (k, s)).collect());
    let Some(plain) = plain else { return (0..faces.len()).collect() };
    let mut sides: Vec<usize> = (0..faces.len()).filter(|&i| plain[i].0 == 0).collect();
    sides.sort_by(|a, b| plain[*a].1.cmp(&plain[*b].1).then(a.cmp(b)));
    let cap = |k: u8| (0..faces.len()).find(|&i| plain[i].0 == k);
    let mut out: Vec<usize> = sides.first().copied().into_iter().chain(cap(1)).chain(cap(2)).collect();
    out.extend(sides.iter().skip(1));
    out
}

/// Hashes of a body's names, to tell without asking it whether a lookup could find something there: its face, edge
/// and vertex names (an exact match), and the roots (split wrappers stripped) of its faces' names and of the face
/// names inside its edge and vertex names (a split relative: `Body::find_*`'s fuzzy matches all share a root).
pub struct NameIndex {
    names: std::collections::HashSet<u64>,
    face_roots: std::collections::HashSet<u64>,
    edge_roots: std::collections::HashSet<u64>,
    vertex_roots: std::collections::HashSet<u64>,
}

#[derive(Clone, Copy, PartialEq)]
enum NameKind {
    Face,
    Edge,
    Vertex,
}

/// A looked-up name's hash and its related roots' hashes.
struct Wanted {
    name: u64,
    kind: NameKind,
    roots: Vec<u64>,
}

fn name_hash(n: &TopoName) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    n.hash(&mut h);
    h.finish()
}

/// The faces named inside an edge or vertex name.
fn named_faces(n: &TopoName) -> Vec<&TopoName> {
    match n {
        TopoName::Edge { faces, .. } => faces.iter().map(|f| &**f).collect(),
        TopoName::Boundary { face, .. } => vec![&**face],
        TopoName::Vertex { faces, .. } => faces.iter().map(|f| &**f).collect(),
        _ => Vec::new(),
    }
}

impl NameIndex {
    fn of(body: &crate::regen::context::Body) -> NameIndex {
        let root = |n: &TopoName| name_hash(n.root());
        let names = body.faces.iter().chain(&body.edges).chain(&body.vertices).map(|n| name_hash(n)).collect();
        let face_roots = body.faces.iter().map(|n| root(n)).collect();
        let edge_roots = body.edges.iter().flat_map(|n| named_faces(n).into_iter().map(root)).collect();
        let vertex_roots = body.vertices.iter().flat_map(|n| named_faces(n).into_iter().map(root)).collect();
        NameIndex { names, face_roots, edge_roots, vertex_roots }
    }

    fn may_hold(&self, w: &Wanted) -> bool {
        if self.names.contains(&w.name) {
            return true;
        }
        let roots = match w.kind {
            NameKind::Face => &self.face_roots,
            NameKind::Edge => &self.edge_roots,
            NameKind::Vertex => &self.vertex_roots,
        };
        w.roots.iter().any(|r| roots.contains(r))
    }
}

impl Wanted {
    fn of(n: &TopoName, kind: NameKind) -> Wanted {
        let roots = match kind {
            NameKind::Face => vec![name_hash(n.root())],
            NameKind::Edge | NameKind::Vertex => named_faces(n).into_iter().map(|f| name_hash(f.root())).collect(),
        };
        Wanted { name: name_hash(n), kind, roots }
    }
}

/// What qCreatedBy's answer depends on, as it was ([`CtxState::created_cache`]): the bodies and sketches themselves
/// (held, so a new one can't take an old one's address) and the sizes of the records it reads.
pub struct CreatedSeen {
    bodies: Vec<Arc<crate::regen::context::Body>>,
    sketches: Vec<Arc<crate::regen::context::SketchResult>>,
    counts: [usize; 11],
}

impl CreatedSeen {
    fn counts(st: &CtxState) -> [usize; 11] {
        [st.ops.len(), st.adopted.len(), st.merged_creators.len(), st.body_ops.len(), st.split_face_edges.len(), st.made_vertices.len(), st.plane_ops.len(), st.hidden.len(), st.ctx.planes.len(), st.ctx.connectors.len(), st.ctx.points.len()]
    }

    fn of(st: &CtxState) -> CreatedSeen {
        CreatedSeen { bodies: st.ctx.bodies.untracked().to_vec(), sketches: st.ctx.sketches.clone(), counts: Self::counts(st) }
    }

    fn same(&self, st: &CtxState) -> bool {
        self.counts == Self::counts(st)
            && self.bodies.len() == st.ctx.bodies.len()
            && self.bodies.iter().zip(st.ctx.bodies.untracked()).all(|(a, b)| Arc::ptr_eq(a, b))
            && self.sketches.len() == st.ctx.sketches.len()
            && self.sketches.iter().zip(&st.ctx.sketches).all(|(a, b)| Arc::ptr_eq(a, b))
    }
}

/// A body's boxes of its faces, edges or vertices, by index.
pub type SubBoxes = Arc<Vec<Option<k::BoundingBox>>>;

pub(super) fn uniq(v: Vec<Ent>) -> Vec<Ent> {
    let mut seen = std::collections::HashSet::new();
    v.into_iter().filter(|e| seen.insert(e.clone())).collect()
}

/// A query's `bodyType`: one BodyType or an array of them.
fn body_types(query: &Value) -> Vec<Arc<str>> {
    match query.get("bodyType") {
        Value::Array(a, _) => a.iter().filter_map(enum_str).collect(),
        v => enum_str(v).into_iter().collect(),
    }
}

fn enum_str(v: &Value) -> Option<Arc<str>> {
    v.as_str()
}

fn etype_of(v: &Value) -> Option<EType> {
    enum_str(v).and_then(|s| EType::from_name(&s))
}

/// Whether `id` starts with `prefix`.
fn has_prefix(id: &[Arc<str>], prefix: &[Arc<str>]) -> bool {
    id.len() >= prefix.len() && id.iter().zip(prefix).all(|(a, b)| a == b)
}

fn id_of(v: &Value) -> Option<FsId> {
    v.as_array().map(|a| a.iter().filter_map(|c| c.as_str()).collect())
}

/// Evaluate a query value to entities.
pub fn eval(i: &mut Interp, q: &QueryCx, query: &Value) -> R<Vec<Ent>> {
    if query.is_undefined() {
        return Ok(Vec::new());
    }
    if let Value::Array(items, _) = query {
        // Arrays of queries act as their union.
        let mut out = Vec::new();
        for x in items.iter() {
            out.extend(eval(i, q, x)?);
        }
        return Ok(uniq(out));
    }
    if !query.get("historyType").is_undefined() {
        return Ok(historical(q, query));
    }
    let Some(kind) = query.get("queryType").as_str() else {
        return Err(i.error(format!("Not a query: {}", noshape_featurescript::value::to_display(query))));
    };
    let entity_type = etype_of(query.get("entityType"));
    let sub = |i: &mut Interp, key: &str| -> R<Vec<Ent>> { eval(i, q, query.get(key)) };
    let filter = |v: Vec<Ent>| -> Vec<Ent> {
        match entity_type {
            Some(t) => v.into_iter().filter(|e| entity_type_of(e) == t).collect(),
            None => v,
        }
    };
    let r: Vec<Ent> = match &*kind {
        "NOTHING" => Vec::new(),
        "EVERYTHING" => {
            let mut all = q.all(entity_type);
            // std's qBodyType of qEverything is this query with its body types.
            let types = body_types(query);
            if !types.is_empty() {
                all.retain(|e| types.iter().any(|t| **t == *q.body_type(e)));
            }
            live_order(q, &mut all);
            all
        }
        "TRANSIENT" => {
            let id = query.get("transientId").as_str().unwrap_or_default();
            q.resolve_transient(&id)
        }
        "UNION" => {
            let mut out = Vec::new();
            if let Some(arr) = query.get("subqueries").as_array() {
                for s in arr.iter() {
                    out.extend(eval(i, q, s)?);
                }
            }
            uniq(out)
        }
        "INTERSECTION" => {
            let mut lists = Vec::new();
            if let Some(arr) = query.get("subqueries").as_array() {
                for s in arr.iter() {
                    lists.push(eval(i, q, s)?);
                }
            }
            match lists.split_first() {
                Some((first, rest)) => uniq(first.iter().filter(|e| rest.iter().all(|l| l.contains(e))).cloned().collect()),
                None => Vec::new(),
            }
        }
        "SUBTRACTION" => {
            let a = sub(i, "query1")?;
            let b = sub(i, "query2")?;
            a.into_iter().filter(|e| !b.contains(e)).collect()
        }
        "NTH_ELEMENT" => {
            let v = sub(i, "subquery")?;
            let n = query.get("n").as_number().unwrap_or(0.0) as i64;
            let idx = if n < 0 { v.len() as i64 + n } else { n };
            if idx >= 0 && (idx as usize) < v.len() { vec![v[idx as usize].clone()] } else { Vec::new() }
        }
        "ENTITY_FILTER" => filter(sub(i, "subquery")?),
        "CREATED_BY" => {
            let Some(id) = id_of(query.get("featureId")) else { return Ok(Vec::new()) };
            let merged = |e: &Ent| match e {
                Ent::Body(b) => q.st.merged_creators.get(&q.st.ctx.bodies[*b].id).is_some_and(|cs| cs.iter().any(|c| has_prefix(c, &id))),
                _ => false,
            };
            let key = (id.clone(), entity_type);
            if let Some((_, found)) = q.st.created_cache.lock().get(&key).filter(|(seen, _)| seen.same(q.st)) {
                return Ok(found.clone());
            }
            let visible: Vec<Ent> = q.all(entity_type).into_iter().filter(|e| q.created_by(e).is_some_and(|c| has_prefix(&c, &id)) || merged(e)).collect();
            let found: Vec<Ent> = if !visible.is_empty() {
                visible
            } else {
                // Nothing visible: the faces / edges it made may now be part of
                // a (hidden) sheet metal definition (std loft Add joins its sheet
                // to the definition, then finds the new joint by qCreatedBy).
                // Not the definition body itself (std flange's merge checks that
                // its boolean created no body).
                q.all_with(entity_type, true).into_iter().filter(|e| !matches!(e, Ent::Body(_)) && q.created_by(e).is_some_and(|c| has_prefix(&c, &id))).collect()
            };
            let mut cache = q.st.created_cache.lock();
            if cache.len() >= 64 {
                cache.clear();
            }
            cache.insert(key, (CreatedSeen::of(q.st), found.clone()));
            found
        }
        "SKETCH_REGION" => {
            let Some(id) = id_of(query.get("featureId")) else { return Ok(Vec::new()) };
            let filter_inner = query.get("filterInnerLoops").as_bool().unwrap_or(false);
            let mut out = Vec::new();
            for (s, sk) in q.st.ctx.sketches.iter().enumerate().filter(|(_, sk)| sk.holds_sheet()) {
                if q.created_by(&Ent::SketchSheet(s)).is_some_and(|c| has_prefix(&c, &id)) {
                    for (i, r) in sk.regions.iter().enumerate() {
                        // Not the face's imprinted pieces (sketch faces all the same).
                        if sk.imprinted.get(i).copied().unwrap_or(false) {
                            continue;
                        }
                        // Inner loops: regions lying in another region's hole (one of several inside a hole too: a
                        // bore with a keyway, the Spur Gear's).
                        if filter_inner && {
                            let p = r.interior_point();
                            sk.regions.iter().any(|o| o.holes.iter().any(|h| h.contains(p)))
                        } {
                            continue;
                        }
                        out.push(Ent::SketchRegion(s, encode_region_key(&r.id)));
                    }
                }
            }
            out
        }
        "CAP_ENTITY" => {
            let Some(id) = id_of(query.get("featureId")) else { return Ok(Vec::new()) };
            // (Before CapType a std wrote `startCap`, a boolean: std 355-1494's qCapEntity(id, isStartCap).)
            let cap = query.get("capType").as_str().unwrap_or_else(|| match query.get("startCap").as_bool() {
                Some(true) => "START".into(),
                Some(false) => "END".into(),
                None => "EITHER".into(),
            });
            let faces: Vec<Ent> = q.all(Some(EType::Face)).into_iter().filter(|e| q.cap_of(e).is_some_and(|(c, op)| has_prefix(&op, &id) && (&*cap == "EITHER" || c == &*cap))).collect();
            // Without an entity type: every cap entity, faces, their edges and vertices (qNonCapEntity subtracts
            // them from a feature's edges; the Spur Gear's root fillet takes the non-cap edges).
            match entity_type {
                Some(EType::Face) => faces,
                t => {
                    let mut out = if t.is_none() { faces.clone() } else { Vec::new() };
                    for f in &faces {
                        let (edges, verts) = q.boundary(f);
                        match t {
                            None => out.extend(edges.into_iter().chain(verts)),
                            Some(EType::Edge) => out.extend(edges),
                            Some(EType::Vertex) => out.extend(verts),
                            Some(_) => out.extend(q.owner(f)),
                        }
                    }
                    uniq(out)
                }
            }
        }
        "OWNED_BY_PART" => {
            // Only bodies own entities: evLength's std code passes
            // qOwnedByBody(<the edges>, EDGE), which must add nothing.
            let parts: Vec<Ent> = sub(i, "part")?.into_iter().filter(|p| entity_type_of(p) == EType::Body).collect();
            let mut out = Vec::new();
            if query.get("subquery").is_undefined() {
                for p in &parts {
                    if let Some(o) = q.owner(p) {
                        out.extend(q.owned(&o, entity_type));
                    }
                }
                live_order(q, &mut out);
            } else {
                let owners: Vec<Ent> = parts.iter().filter_map(|p| q.owner(p)).collect();
                out = sub(i, "subquery")?.into_iter().filter(|e| q.owner(e).is_some_and(|o| owners.contains(&o))).collect();
            }
            uniq(out)
        }
        "OWNER_PART" => uniq(eval(i, q, query.get("query"))?.iter().filter_map(|e| q.owner(e)).collect()),
        "BODY_TYPE" => {
            let types = body_types(query);
            sub(i, "subquery")?.into_iter().filter(|e| types.iter().any(|t| **t == *q.body_type(e))).collect()
        }
        "GEOMETRY" => {
            let g = enum_str(query.get("geometryType")).unwrap_or_default();
            sub(i, "subquery")?.into_iter().filter(|e| q.geometry_type(e).is_some_and(|t| t == &*g)).collect()
        }
        "CONSTRUCTION_FILTER" => {
            let yes = enum_str(query.get("constructionFilter")).is_some_and(|s| &*s == "YES");
            sub(i, "subquery")?.into_iter().filter(|e| q.is_construction(e) == yes).collect()
        }
        "SKETCH_OBJECT_FILTER" => {
            let yes = enum_str(query.get("sketchObjectFilter")).is_some_and(|s| &*s == "YES");
            sub(i, "subquery")?.into_iter().filter(|e| q.is_sketch(e) == yes).collect()
        }
        "MODIFIABLE_ENTITY_FILTER" => sub(i, "subquery")?.into_iter().filter(|e| q.is_modifiable(e)).collect(),
        "ACTIVE_SM_FILTER" => {
            let yes = enum_str(query.get("activeSheetMetal")).is_some_and(|s| &*s == "YES");
            let v = sub(i, "subquery")?;
            v.into_iter().filter(|e| is_sheet_metal(q, e) == yes).collect()
        }
        "SM_FLAT_FILTER" | "SM_FORM_FILTER" => {
            let key = if &*kind == "SM_FLAT_FILTER" { "flatFilter" } else { "formFilter" };
            let yes = enum_str(query.get(key)).is_some_and(|s| &*s == "YES");
            if yes { Vec::new() } else { sub(i, "subquery")? }
        }
        "MESH_GEOMETRY_FILTER" => {
            let yes = enum_str(query.get("meshGeometryFilter")).is_some_and(|s| &*s == "YES");
            if yes { Vec::new() } else { sub(i, "subquery")? }
        }
        "SM_DEFINITION_ENTITY_FILTER" | "CORRESPONDING_IN_FLAT" | "PARTS_ATTACHED_TO" | "SM_APPLICATION_TYPE_FILTER" => Vec::new(),
        // Consumed: a member of a closed composite part.
        "CONSUMED" => {
            let yes = enum_str(query.get("consumed")).is_some_and(|s| &*s == "YES");
            let bodies = &q.st.ctx.bodies;
            let consumed = |e: &Ent| match q.owner(e).unwrap_or_else(|| e.clone()) {
                Ent::Body(b) => bodies.iter().filter_map(|c| c.composite.as_ref()).any(|c| c.closed && c.members.contains(&bodies[b].id)),
                _ => false,
            };
            sub(i, "subquery")?.into_iter().filter(|e| consumed(e) == yes).collect()
        }
        "COMPOSITE_PART_TYPE_FITLER" | "TOLERANCE_FILTER" => sub(i, "subquery")?,
        "OP_HOLE_FACE" | "OP_HOLE_PROFILE" => filter(super::hole::op_hole_query(i, q, query, &*kind == "OP_HOLE_FACE")?),
        "PATTERN_INSTANCES" => {
            let Some(id) = id_of(query.get("featureId")) else { return Ok(Vec::new()) };
            let names: Vec<String> = query.get("instanceNames").as_array().map(|a| a.iter().filter_map(|n| n.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
            let bodies: Vec<BodyId> = names.into_iter().flat_map(|n| q.st.pattern_instances.get(&(id.clone(), n)).cloned().unwrap_or_default()).collect();
            let of = |e: &Ent| match q.owner(e).unwrap_or_else(|| e.clone()) {
                Ent::Body(b) => bodies.contains(&q.st.ctx.bodies[b].id),
                _ => false,
            };
            // (Only the entities of the type asked: std's Fill pattern asks once per instance.)
            filter(q.all(entity_type).into_iter().filter(of).collect())
        }
        "CONTAINED_IN_COMPOSITE" | "COMPOSITE_CONTAINING" | "SOURCE_MESH" => Vec::new(),
        "VERTEX_ADJACENT" | "EDGE_ADJACENT" => {
            let t = entity_type.unwrap_or(EType::Face);
            let mut out = Vec::new();
            for e in eval(i, q, query.get("query"))? {
                out.extend(q.adjacent(&e, t, &*kind == "VERTEX_ADJACENT"));
            }
            let mut out = uniq(out);
            live_order(q, &mut out);
            out
        }
        "EDGE_VERTEX" => {
            let at_start = query.get("atStart").as_bool().unwrap_or(true);
            let mut out = Vec::new();
            for e in eval(i, q, query.get("query"))? {
                if let Some(v) = edge_end_vertex(q, &e, at_start) {
                    out.push(v);
                }
            }
            uniq(out)
        }
        "EDGE_TOPOLOGY_FILTER" => {
            let t = enum_str(query.get("edgeTopologyType")).unwrap_or_default();
            sub(i, "subquery")?
                .into_iter()
                .filter(|e| {
                    let Ent::Edge(b, x) = e else { return &*t == "WIRE" && matches!(e, Ent::SketchEdge(..) | Ent::SketchRegionEdge(..)) };
                    let n = q.topo(*b).edge_faces.get(*x).map(|f| f.len()).unwrap_or(0);
                    match &*t {
                        "WIRE" => n == 0,
                        "ONE_SIDED" | "LAMINAR" => n == 1,
                        "TWO_SIDED" => n >= 2,
                        _ => false,
                    }
                })
                .collect()
        }
        "UNIQUE_VERTICES" => {
            let mut out: Vec<Ent> = Vec::new();
            let mut pts: Vec<DVec3> = Vec::new();
            for e in sub(i, "subquery")? {
                let Some(p) = q.point(&e) else { continue };
                if !pts.iter().any(|x| (*x - p).length() < 1e-8) {
                    pts.push(p);
                    out.push(e);
                }
            }
            out
        }
        "PLANE_NORMAL" => {
            let n = vec3_of(query.get("normal")).unwrap_or(DVec3::Z).normalize_or_zero();
            let anti = query.get("allowAntiparallel").as_bool().unwrap_or(true);
            sub(i, "subquery")?
                .into_iter()
                .filter(|e| {
                    q.plane(e).is_some_and(|p| {
                        let d = p.normal.dot(n);
                        if anti { d.abs() > 1.0 - 1e-9 } else { d > 1.0 - 1e-9 }
                    })
                })
                .collect()
        }
        "PLANE_PARALLEL_DIRECTION" => {
            let d = vec3_of(query.get("direction")).unwrap_or(DVec3::Z).normalize_or_zero();
            sub(i, "subquery")?.into_iter().filter(|e| q.plane(e).is_some_and(|p| p.normal.dot(d).abs() < 1e-9)).collect()
        }
        "FACE_PARALLEL_DIRECTION" => {
            let d = vec3_of(query.get("direction")).unwrap_or(DVec3::Z).normalize_or_zero();
            sub(i, "subquery")?.into_iter().filter(|e| face_parallel_to(q, e, d)).collect()
        }
        "PARALLEL_EDGES" => {
            let dirs: Vec<DVec3> = if let Some(d) = vec3_of(query.get("direction")) {
                vec![d.normalize_or_zero()]
            } else {
                let refs = if query.get("edges").is_undefined() { sub(i, "subquery")? } else { sub(i, "edges")? };
                refs.iter().filter_map(|e| line_dir(q, e)).collect()
            };
            let candidates = if query.get("queryToFilter").is_undefined() { q.all(Some(EType::Edge)) } else { sub(i, "queryToFilter")? };
            candidates.into_iter().filter(|e| line_dir(q, e).is_some_and(|d| dirs.iter().any(|x| x.dot(d).abs() > 1.0 - 1e-9))).collect()
        }
        "CONTAINS_POINT" => {
            let p = vec3_of(query.get("point")).unwrap_or_default();
            sub(i, "subquery")?.into_iter().filter(|e| contains_point(q, e, p)).collect()
        }
        "INTERSECTS_PLANE" => {
            let Some(pl) = plane_of(query.get("plane")) else { return Ok(Vec::new()) };
            sub(i, "subquery")?.into_iter().filter(|e| intersects_plane(q, e, &pl)).collect()
        }
        "IN_FRONT_OF_PLANE" => {
            let Some(pl) = plane_of(query.get("plane")) else { return Ok(Vec::new()) };
            sub(i, "subquery")?
                .into_iter()
                .filter(|e| match q.shape(e).and_then(|s| s.bounding_box(false).ok()) {
                    Some(b) => box_corners(b.min, b.max).iter().all(|c| (*c - pl.origin).dot(pl.normal) >= -1e-9),
                    None => q.point(e).is_some_and(|p| (p - pl.origin).dot(pl.normal) >= -1e-9),
                })
                .collect()
        }
        "COINCIDES_WITH_PLANE" => {
            let Some(pl) = plane_of(query.get("plane")) else { return Ok(Vec::new()) };
            sub(i, "subquery")?.into_iter().filter(|e| coincides_with_plane(q, e, &pl)).collect()
        }
        "INTERSECTS_LINE" => {
            let Some((o, d)) = line_of(query.get("line")) else { return Ok(Vec::new()) };
            let line = k::make_line(o - d * 1e4, o + d * 1e4).ok();
            sub(i, "subquery")?
                .into_iter()
                .filter(|e| match (q.shape(e), &line) {
                    (Some(s), Some(l)) => k::distance(&s, l).is_ok_and(|x| x.distance < 1e-8),
                    _ => false,
                })
                .collect()
        }
        "INTERSECTS_BALL" => {
            let p = vec3_of(query.get("point")).unwrap_or_default();
            let r = query.get("radius").as_number().unwrap_or(0.0);
            sub(i, "subquery")?.into_iter().filter(|e| q.distance_to_point(e, p).is_some_and(|d| d <= r + 1e-9)).collect()
        }
        "CLOSEST_TO" => {
            let p = vec3_of(query.get("point")).unwrap_or_default();
            let v: Vec<(Ent, f64)> = sub(i, "subquery")?.into_iter().filter_map(|e| q.distance_to_point(&e, p).map(|d| (e, d))).collect();
            extremes(v, false)
        }
        "FARTHEST_ALONG" => {
            let d = vec3_of(query.get("direction")).unwrap_or(DVec3::Z).normalize_or_zero();
            let v: Vec<(Ent, f64)> = sub(i, "subquery")?.into_iter().filter_map(|e| farthest_along(q, &e, d).map(|x| (e, x))).collect();
            extremes(v, true)
        }
        "LARGEST" | "SMALLEST" => {
            // (Cancelled: the rest unmeasured, each ~0.6 ms on a gear's B-spline faces; the script stops at its next call.)
            let v: Vec<(Ent, f64)> = sub(i, "subquery")?.into_iter().filter_map(|e| if k::thread_cancelled() { None } else { q.measure(&e).map(|m| (e, m)) }).collect();
            extremes(v, &*kind == "LARGEST")
        }
        "TANGENT_CONNECTED_FACES" | "TANGENT_FACES" => {
            let tol = query.get("angleTolerance").as_number().unwrap_or(1e-6).max(1e-9);
            connected_faces(q, sub(i, "subquery")?, |a, b, e| edge_tangent(q, a, b, e, tol))
        }
        "CONVEX_CONNECTED_FACES" => connected_faces(q, sub(i, "subquery")?, |a, b, e| edge_convexity(q, a, b, e) == Some(true)),
        "CONCAVE_CONNECTED_FACES" => connected_faces(q, sub(i, "subquery")?, |a, b, e| edge_convexity(q, a, b, e) == Some(false)),
        "TANGENT_CONNECTED_EDGES" | "TANGENT_EDGES" => tangent_edges(q, sub(i, "subquery")?),
        "LOOP_EDGES" | "LOOP_AROUND_FACE" => {
            let mut out = Vec::new();
            for e in sub(i, "subquery")? {
                if let Ent::Face(b, f) = e {
                    out.extend(q.edges_of_face(b, f));
                } else if matches!(e, Ent::Edge(..)) {
                    out.extend(tangent_edges(q, vec![e]));
                }
            }
            uniq(out)
        }
        "HOLE_FACES" => hole_faces(q, sub(i, "subquery")?),
        "FILLET_FACES" => fillet_faces(q, sub(i, "subquery")?, enum_str(query.get("compareType")).unwrap_or_else(|| "EQUAL".into())),
        "LOOP_BOUNDED_FACES" | "FACE_OR_EDGE_BOUNDED_FACES" | "SHELL_CONTAINING_FACE" => bounded_faces(q, sub(i, "subquery")?),
        "COEDGE" => {
            let faces = sub(i, "faceQuery")?;
            sub(i, "edgeQuery")?.into_iter().filter(|e| matches!(e, Ent::Edge(b, x) if faces.iter().any(|f| matches!(f, Ent::Face(fb, _) if fb == b) && q.faces_of_edge(*b, *x).contains(f)))).collect()
        }
        "MATE_CONNECTOR" => {
            let parts = sub(i, "subquery")?;
            let owners: Vec<Ent> = parts.iter().filter_map(|p| q.owner(p)).collect();
            (0..q.st.ctx.connectors.len())
                .map(Ent::Connector)
                .filter(|c| {
                    let r = q.to_ref(c);
                    owners.is_empty() || owners.iter().any(|o| matches!(o, Ent::Body(b) if r.depends_on() == Some(q.st.ctx.bodies[*b].id.feature)))
                })
                .collect()
        }
        "DEPENDENCY" => {
            // What the operation that made each entity was made from;
            // entities with nothing recorded stand for themselves.
            let mut out = Vec::new();
            for e in sub(i, "subquery")? {
                match q.created_by(&e).and_then(|op| q.st.dependencies.get(&op)) {
                    Some(refs) => out.extend(refs.iter().flat_map(|r| q.from_ref(r))),
                    None => out.push(e),
                }
            }
            uniq(out)
        }
        "LAMINAR_DEPENDENCY" | "PATTERN" => sub(i, "subquery")?,
        "TRACKING" => {
            // Only what operations after `lastOperationId` made or changed:
            // "entities derived from subquery in features between
            // startTracking and when query is evaluated" (std feature.fs).
            // Std Bend tracks its bend edges after copying the bend face;
            // the untouched copy lies on them too.
            let last: Vec<Arc<str>> = query.get("lastOperationId").as_array().map(|a| a.iter().filter_map(|c| c.as_str()).collect()).unwrap_or_default();
            let since = q.st.ops.values().find(|o| !last.is_empty() && o.id == last).map_or(0, |o| o.seq + 1);
            let mut out = Vec::new();
            for key in ["subquery1", "subquery2"] {
                let items: Vec<Value> = match query.get(key) {
                    Value::Array(a, _) => a.iter().cloned().collect(),
                    Value::Undefined => Vec::new(),
                    v => vec![v.clone()],
                };
                for item in items {
                    match item.get("queryType").as_str().as_deref() {
                        Some("TRANSIENT") => out.extend(q.tracking(&item.get("transientId").as_str().unwrap_or_default(), since)),
                        _ => out.extend(eval(i, q, &item)?),
                    }
                }
            }
            // A tracked body split since: its pieces too (std sheet metal's Fold tracks its definition sheet through
            // the split that makes the fold's two sides).
            let bodies: Vec<&BodyId> = out.iter().filter_map(|e| if let Ent::Body(b) = e { Some(&q.st.ctx.bodies[*b].id) } else { None }).collect();
            let mut pieces = Vec::new();
            for (op, (all, _)) in &q.st.splits {
                let after = q.st.ops.values().find(|o| &o.id == op).is_none_or(|o| o.seq >= since);
                if after && all.iter().any(|b| bodies.contains(&b)) {
                    pieces.extend(all.iter().filter_map(|b| q.st.ctx.bodies.iter().position(|x| &x.id == b)).map(Ent::Body));
                }
            }
            out.extend(pieces);
            out.retain(|e| match q.owner(e) {
                Some(Ent::Body(b)) => q.st.touched.get(&q.st.ctx.bodies[b].id).is_some_and(|s| *s > since),
                _ => true,
            });
            uniq(out)
        }
        "HISTORICAL" => Vec::new(),
        "ATTRIBUTE_FILTER" => {
            let name = query.get("name").clone();
            let pattern = query.get("attributePattern").clone();
            let exact = query.get("valueToMatchExactly").clone();
            // The entities with a matching attribute, once (thousands of entities and attributes after a pattern).
            let matches = |v: &Value| (name.is_undefined() || *v.get("name") == name) && (exact.is_undefined() || *v.get("attribute") == exact) && attr_matches(v.get("attribute"), &pattern);
            // A value or pattern with a string field (std's sheet metal attributes' ids, a filter each): only the
            // attributes with that field's value are looked at.
            let matching = match [&exact, &pattern].into_iter().find_map(string_field) {
                Some((field, value)) => Arc::new(q.st.attributes_with_field(&field, &value, matches)),
                None => q.st.attributes_matching(format!("{name:?}\u{1}{pattern:?}\u{1}{exact:?}"), matches),
            };
            let attrs = q.st.attributes();
            let having: std::collections::HashSet<&Reference> = matching.iter().map(|&k| &attrs[k].0).collect();
            if having.is_empty() {
                return Ok(Vec::new());
            }
            // No subquery: everything, in all_with's order. Where the matches are all bodies' entities, they alone in
            // that order (bodies, then faces, edges and vertices by body and index): the whole list was built for each
            // filter (c2-sm-louver's Extrude 3 / 4: thousands of entities, 215 filters).
            // (Only where the shortcut below would be taken: the bodies' entities alone outnumber the matches 16 to 1.)
            let body_ents: usize = q.st.ctx.bodies.iter().map(|b| 1 + b.faces.len() + b.edges.len() + b.vertices.len()).sum();
            if query.get("subquery").is_undefined() && having.len() * 16 < body_ents {
                let mut found: Vec<Ent> = Vec::new();
                let resolved = having.iter().all(|r| {
                    let es = q.from_ref(r);
                    let some = !es.is_empty();
                    found.extend(es);
                    some
                });
                let key = |e: &Ent| match *e {
                    Ent::Body(b) => Some((0, b, 0)),
                    Ent::Face(b, f) => Some((1, b, f)),
                    Ent::Edge(b, x) => Some((2, b, x)),
                    Ent::Vertex(b, v) => Some((3, b, v)),
                    _ => None,
                };
                if resolved && found.iter().all(|e| key(e).is_some()) {
                    found.sort_by_key(|e| key(e));
                    found.dedup();
                    return Ok(found.into_iter().filter(|e| having.contains(&q.to_ref(e)) && q.shown(e)).collect());
                }
            }
            let base = if query.get("subquery").is_undefined() { q.all_with(None, true) } else { sub(i, "subquery")? };
            // A few attributed entities among thousands (a sheet metal model after its patterns: c2-sm-louver's 839
            // filters of ~7 among ~9000 took 3.2 s): those found by their references, not every entity's reference
            // built and hashed. (Still each kept one's own reference among them, as below.)
            if having.len() * 16 < base.len() {
                let mut found: std::collections::HashSet<Ent> = std::collections::HashSet::new();
                let resolved = having.iter().all(|r| {
                    let es = q.from_ref(r);
                    let some = !es.is_empty();
                    found.extend(es);
                    some
                });
                if resolved {
                    return Ok(base.into_iter().filter(|e| found.contains(e) && having.contains(&q.to_ref(e)) && q.shown(e)).collect());
                }
            }
            base.into_iter().filter(|e| having.contains(&q.to_ref(e))).collect()
        }
        "EDGE_CONVEXITY_FILTER" => {
            let want = enum_str(query.get("convexityType")).unwrap_or_default();
            sub(i, "subquery")?
                .into_iter()
                .filter(|e| {
                    let Ent::Edge(b, x) = e else { return false };
                    let faces = q.faces_of_edge(*b, *x);
                    if faces.len() != 2 {
                        return false;
                    }
                    let c = if edge_tangent(q, &faces[0], &faces[1], e, 1e-6) { "SMOOTH" } else if edge_convexity(q, &faces[0], &faces[1], e) == Some(true) { "CONVEX" } else { "CONCAVE" };
                    c == &*want
                })
                .collect()
        }
        "AXIS" => {
            let Some((o, d)) = line_of(query.get("axis")) else { return Ok(Vec::new()) };
            sub(i, "subquery")?.into_iter().filter(|e| entity_axis(q, e).is_some_and(|(p, a)| a.dot(d).abs() > 1.0 - 1e-9 && ((p - o) - d * (p - o).dot(d)).length() < 1e-8)).collect()
        }
        "COINCIDENT" => {
            let targets = sub(i, "target")?;
            sub(i, "subquery")?.into_iter().filter(|e| targets.iter().any(|t| coincident(q, e, t))).collect()
        }
        other => return Err(i.error(format!("Query type {other} is not supported"))),
    };
    Ok(r.into_iter().filter(|e| q.shown(e)).collect())
}

/// Historical queries (`makeQuery(operationId, "SKETCH_ENTITY", ...)`).
fn historical(q: &QueryCx, query: &Value) -> Vec<Ent> {
    let kind = query.get("queryType").as_str().unwrap_or_default();
    let Some(op) = id_of(query.get("operationId")) else { return Vec::new() };
    let marker = super::marker_of(&op);
    let t = etype_of(query.get("entityType"));
    if &*kind == "IMPRINT" {
        // `makeQuery(sketchId + "imprint", "IMPRINT", ...)`: the sketch's regions / curves.
        let Some((_, sketch_id)) = op.split_last() else { return Vec::new() };
        let sm = super::marker_of(sketch_id);
        let pieces = q.st.ctx.sketches.iter().enumerate().filter(|(_, x)| x.feature == sm).map(|(s, _)| s);
        return pieces.flat_map(|s| q.sketch_owned(s, match t {
            Some(EType::Edge) => EType::Edge,
            Some(EType::Vertex) => EType::Vertex,
            _ => EType::Face,
        })).collect();
    }
    match &*kind {
        // Default planes and the origin (Show code's packed queries; live
        // dom/fs-packed-query.txt): only unprefixed ids resolve.
        "DUMMY" => match op.iter().map(|c| &**c).collect::<Vec<_>>().as_slice() {
            ["Top.planeOp"] => vec![Ent::Datum(DatumPlane::Top)],
            ["Front.planeOp"] => vec![Ent::Datum(DatumPlane::Front)],
            ["Right.planeOp"] => vec![Ent::Datum(DatumPlane::Right)],
            ["Origin.pointOp"] => vec![Ent::Origin],
            _ => Vec::new(),
        },
        // The side faces an extrude-like operation swept from the profile's
        // edges (std extrude's draft faces), not its caps.
        "SWEPT_FACE" => q
            .all_with(Some(EType::Face), true)
            .into_iter()
            .filter(|e| {
                let Ent::Face(b, f) = e else { return false };
                use crate::reference::{Role, TopoName};
                // Operation names wrap the op's own (`Generated(inner)`).
                let name = match q.st.ctx.bodies[*b].faces[*f].root() {
                    TopoName::Face { role: Role::Generated(inner), .. } => inner.root(),
                    n => n,
                };
                let side = matches!(name, TopoName::Face { role: Role::Side(_), .. });
                side && q.created_by(e).as_ref() == Some(&op)
            })
            .collect(),
        // A part of another Part Studio (PartStudioData.partQuery): the
        // bodies its feature made, told apart by their sketch edges.
        "SWEPT_BODY" => {
            let feature = op.first().map(|o| o.split('.').next().unwrap_or_default().to_string()).unwrap_or_default();
            let wanted: Vec<u32> = query
                .get("disambiguationData")
                .as_array()
                .iter()
                .flat_map(|d| d.iter())
                .flat_map(|d| d.get("originals").as_array().map(|o| o.iter().cloned().collect::<Vec<_>>()).unwrap_or_default())
                .filter_map(|o| o.get("sketchEntityId").as_str().and_then(|s| s.parse().ok()))
                .collect();
            let made: Vec<usize> = q.st.ctx.bodies.iter().enumerate().filter(|(_, b)| b.id.feature.to_string() == feature).map(|(i, _)| i).collect();
            let fits = |i: &usize| {
                q.st.ctx.bodies[*i].faces.iter().any(|f| matches!(f.root(), crate::reference::TopoName::Face { role: crate::reference::Role::Side(crate::reference::Source::Sketch { entity, .. }), .. } if wanted.contains(entity)))
            };
            let picked: Vec<usize> = if made.len() > 1 && !wanted.is_empty() { made.iter().copied().filter(fits).collect() } else { made };
            picked.into_iter().map(Ent::Body).collect()
        }
        // `qSplitBy`: the bodies a split left on one side of its tool plane
        // (the back: against the plane normal), or their faces the split cut.
        "SPLIT" => {
            // opSplitFace: the pieces of the faces it split (either side).
            if let Some(pieces) = q.st.split_face_pieces.get(&op) {
                let piece = |e: &Ent| q.snapshot(e).and_then(|s| s.points.first().copied()).is_some_and(|m| pieces.iter().any(|p| p.distance(m) < 1e-7));
                return q.all_with(Some(EType::Face), true).into_iter().filter(piece).collect();
            }
            if let Some(originals) = q.st.split_edges.get(&op) {
                // Split edges: the pieces lying on the edges that were split.
                let on = |e: &Ent| {
                    let Ent::Edge(b, _) = e else { return false };
                    let body = &q.st.ctx.bodies[*b].id;
                    q.snapshot(e).and_then(|s| s.points.get(1).copied()).is_some_and(|m| originals.iter().any(|(ob, o)| ob == body && q.all_on(o, &[m], 1e-7)))
                };
                return q.all_with(Some(EType::Edge), true).into_iter().filter(on).collect();
            }
            let Some((ids, plane)) = q.st.splits.get(&op) else { return Vec::new() };
            let back = query.get("isFromBackBody").as_bool() == Some(true);
            let bodies: Vec<usize> = ids.iter().filter_map(|id| q.st.ctx.bodies.iter().position(|b| &b.id == id)).collect();
            let Some((origin, normal)) = *plane else { return bodies.into_iter().map(Ent::Body).collect() };
            let side = |e: &Ent| q.point(e).map(|p| (p - origin).dot(normal));
            let mine: Vec<usize> = bodies.into_iter().filter(|b| side(&Ent::Body(*b)).is_some_and(|d| (d < 0.0) == back)).collect();
            match t {
                Some(EType::Face) => {
                    let near = |e: &Ent, sign: f64| q.shape(e).and_then(|s| s.bounding_box(false).ok()).is_some_and(|bb| {
                        let corners = [bb.min, bb.max];
                        corners.iter().any(|c| sign * (*c - origin).dot(normal) < 1e-7)
                    });
                    mine.iter()
                        .flat_map(|b| q.owned(&Ent::Body(*b), Some(EType::Face)))
                        .filter(|f| {
                            // Touches the plane but does not lie in it.
                            let on = near(f, 1.0) && near(f, -1.0);
                            let flat = q.plane(f).is_some_and(|p| p.normal.cross(normal).length() < 1e-9 && (p.origin - origin).dot(normal).abs() < 1e-7);
                            on && !flat
                        })
                        .collect()
                }
                _ => mine.into_iter().map(Ent::Body).collect(),
            }
        }
        "SKETCH_ENTITY" => {
            // (The sketch's piece holding the entity's body.)
            let piece = |e: u32| q.st.ctx.sketches.iter().position(|x| x.feature == marker && x.holds(e));
            let name = query.get("sketchEntityId").as_str().unwrap_or_default();
            // An open curve's `.start` / `.end`: its edge's end vertices (live 2060).
            for (suffix, end) in [(".start", 0u8), (".end", 1u8)] {
                if let Some(base) = name.strip_suffix(suffix)
                    && let Some(c) = super::sketch::sketch_entity(q.st, marker, base)
                    && let Some(s) = piece(c)
                    && open_curve(&q.st.ctx.sketches[s].sketch, c)
                {
                    return if t.is_none_or(|t| t == EType::Vertex) { vec![Ent::SketchCurveEnd(s, c, end)] } else { Vec::new() };
                }
            }
            let Some(e) = super::sketch::sketch_entity(q.st, marker, &name) else { return Vec::new() };
            let Some(s) = piece(e) else { return Vec::new() };
            let sk = &q.st.ctx.sketches[s].sketch;
            let is_point = sk.kind(noshape_sketch::EntityId(e)) == Some(noshape_sketch::EntityKind::Point);
            let me = if is_point { Ent::SketchVertex(s, e) } else { Ent::SketchEdge(s, e) };
            match t {
                None => vec![me],
                Some(EType::Body) => q.owner(&me).into_iter().collect(),
                Some(EType::Edge) if !is_point => vec![me],
                Some(EType::Vertex) if is_point => vec![me],
                // (A fit spline's: nothing, live 2060.)
                Some(EType::Vertex) if open_fit_spline(sk, e) => Vec::new(),
                Some(EType::Vertex) => sk.entity(noshape_sketch::EntityId(e)).map(|x| x.point_refs().iter().map(|p| Ent::SketchVertex(s, p.0)).collect()).unwrap_or_default(),
                Some(EType::Face) => q.st.ctx.sketches[s].regions.iter().filter(|r| r.id.entities().contains(&noshape_sketch::EntityId(e))).map(|r| Ent::SketchRegion(s, encode_region_key(&r.id))).collect(),
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// A map's string field and its value, `attributeId` first: what an attribute matching it must hold too.
fn string_field(v: &Value) -> Option<(Arc<str>, Arc<str>)> {
    let Value::Map(m, _) = v else { return None };
    if let Some(id) = v.get("attributeId").as_str() {
        return Some(("attributeId".into(), id));
    }
    m.iter().find_map(|(k, x)| match (k, x) {
        (Value::Str(k, _), Value::Str(x, _)) => Some((k.clone(), x.clone())),
        _ => None,
    })
}

fn attr_matches(attribute: &Value, pattern: &Value) -> bool {
    if pattern.is_undefined() {
        return true;
    }
    if attribute.tag() != pattern.tag() || attribute.std_type() != pattern.std_type() {
        return false;
    }
    match (attribute, pattern) {
        (Value::Map(a, _), Value::Map(p, _)) => p.iter().all(|(k, v)| a.get(k) == Some(v)),
        _ => true,
    }
}

fn is_sheet_metal(q: &QueryCx, e: &Ent) -> bool {
    match q.owner(e) {
        Some(Ent::Body(b)) => q.st.ctx.sheet_metal.contains_key(&q.st.ctx.bodies[b].id),
        _ => false,
    }
}

fn extremes(v: Vec<(Ent, f64)>, largest: bool) -> Vec<Ent> {
    let best = v.iter().map(|x| x.1).fold(if largest { f64::NEG_INFINITY } else { f64::INFINITY }, |a, b| if largest { a.max(b) } else { a.min(b) });
    let tol = 1e-9 * best.abs().max(1e-9);
    v.into_iter().filter(|x| (x.1 - best).abs() <= tol).map(|x| x.0).collect()
}

fn box_corners(a: DVec3, b: DVec3) -> [DVec3; 8] {
    [
        DVec3::new(a.x, a.y, a.z),
        DVec3::new(b.x, a.y, a.z),
        DVec3::new(a.x, b.y, a.z),
        DVec3::new(b.x, b.y, a.z),
        DVec3::new(a.x, a.y, b.z),
        DVec3::new(b.x, a.y, b.z),
        DVec3::new(a.x, b.y, b.z),
        DVec3::new(b.x, b.y, b.z),
    ]
}

fn farthest_along(q: &QueryCx, e: &Ent, d: DVec3) -> Option<f64> {
    if let Some(s) = q.shape(e) {
        let mut best = f64::NEG_INFINITY;
        for v in s.vertices() {
            if let Ok(p) = v.vertex_point() {
                best = best.max(p.dot(d));
            }
        }
        if s.shape_type() == ShapeType::Vertex {
            best = best.max(s.vertex_point().ok()?.dot(d));
        }
        if best.is_finite() {
            // Curved geometry can reach beyond its vertices.
            if let Ok(b) = s.bounding_box(true) {
                let ext = box_corners(b.min, b.max).iter().map(|c| c.dot(d)).fold(f64::NEG_INFINITY, f64::max);
                if ext > best + 1e-9 && s.vertices().len() < 2 {
                    best = ext;
                }
            }
            return Some(best);
        }
        return s.bounding_box(true).ok().map(|b| box_corners(b.min, b.max).iter().map(|c| c.dot(d)).fold(f64::NEG_INFINITY, f64::max));
    }
    q.point(e).map(|p| p.dot(d))
}

fn line_dir(q: &QueryCx, e: &Ent) -> Option<DVec3> {
    let s = q.shape(e)?;
    match s.edge_info().ok()?.curve {
        k::Curve::Line { direction, .. } => Some(direction.normalize_or_zero()),
        _ => None,
    }
}

/// Axis of an edge (line, circle) or face (cylinder, cone, sphere, torus).
/// A line edge as evLine / evAxis give it: from its middle along its
/// curve's direction, whichever face's use of the edge `s` came from (live
/// oracle P4 / P5: an extruded block's front top edge is (30, 0, 20) mm
/// along +x; behaviours.md "qa5 bracket": its back edge runs −x).
pub fn edge_line(s: &k::Shape) -> Option<(DVec3, DVec3)> {
    let info = s.edge_info().ok()?;
    let k::Curve::Line { direction, .. } = info.curve else { return None };
    let d = direction.normalize_or_zero();
    Some((0.5 * (info.start + info.end), if s.orientation() == k::Orientation::Reversed { -d } else { d }))
}

pub fn entity_axis(q: &QueryCx, e: &Ent) -> Option<(DVec3, DVec3)> {
    let s = q.shape(e)?;
    match entity_type_of(e) {
        EType::Edge => match s.edge_info().ok()?.curve {
            k::Curve::Line { .. } => edge_line(&s),
            k::Curve::Circle { center, axis, .. } | k::Curve::Ellipse { center, axis, .. } => Some((center, axis.normalize_or_zero())),
            _ => None,
        },
        EType::Face => match s.face_surface().ok()? {
            k::Surface::Cylinder { origin, axis, .. } | k::Surface::Cone { origin, axis, .. } => Some((origin, axis.normalize_or_zero())),
            k::Surface::Torus { center, axis, .. } | k::Surface::Sphere { center, axis, .. } => Some((center, axis.normalize_or_zero())),
            k::Surface::Revolution { axis_origin, axis_dir } => Some((axis_origin, axis_dir.normalize_or_zero())),
            _ => None,
        },
        _ => None,
    }
}

fn face_parallel_to(q: &QueryCx, e: &Ent, d: DVec3) -> bool {
    if let Some(p) = q.plane(e) {
        return p.normal.dot(d).abs() < 1e-9;
    }
    match q.shape(e).and_then(|s| s.face_surface().ok()) {
        Some(k::Surface::Cylinder { axis, .. }) => axis.normalize_or_zero().dot(d).abs() > 1.0 - 1e-9,
        Some(k::Surface::Extrusion { direction }) => direction.normalize_or_zero().dot(d).abs() > 1.0 - 1e-9,
        _ => false,
    }
}

fn contains_point(q: &QueryCx, e: &Ent, p: DVec3) -> bool {
    match entity_type_of(e) {
        EType::Body if q.body_type(e) == "SOLID" => q.shape(e).and_then(|s| s.classify_point(p, 1e-7).ok()).is_some_and(|c| !matches!(c, k::PointClass::Outside | k::PointClass::Unknown)),
        _ => {
            if matches!(e, Ent::Datum(_) | Ent::Plane(_) | Ent::PlaneBody(_)) {
                return q.plane(e).is_some_and(|pl| (p - pl.origin).dot(pl.normal).abs() < 1e-8);
            }
            if matches!(e, Ent::Face(..))
                && let Some(f) = q.shape(e)
            {
                if let Some(inside) = planar_face_contains(&f, p) {
                    return inside;
                }
                // Any other face: the point's feet on its surface classified in it (cf-surface-pattern's Surface
                // Pattern 5 asks it of 66 grid points: BRepExtrema's distance took 6.5 ms each on its B-spline face).
                if let Ok(inside) = f.contains_point(p, 1e-7) {
                    return inside;
                }
            }
            // OCCT entities carry ~1e-7 tolerances after booleans and blends.
            q.distance_to_point(e, p).is_some_and(|d| d < 1e-7)
        }
    }
}

/// Whether a planar face bounded by lines and circles holds `p` (within 1e-7 m of it, as the distance test), worked
/// out in its plane: off the plane, near an edge, else by the crossings of a ray (std's Fill pattern asks this of
/// thousands of grid points; BRepExtrema's distance to a face with many holes takes milliseconds each). `None`:
/// another face (the distance decides).
fn planar_face_contains(f: &Shape, p: DVec3) -> Option<bool> {
    const TOL: f64 = 1e-7;
    let pl = f.face_plane().ok()??;
    let n = pl.normal.normalize();
    if (p - pl.origin).dot(n).abs() > TOL {
        return Some(false);
    }
    let x = pl.x_dir.normalize();
    let y = n.cross(x);
    let uv = |w: DVec3| glam::DVec2::new((w - pl.origin).dot(x), (w - pl.origin).dot(y));
    let at = uv(p);
    // An odd direction, so the ray misses the vertices.
    let d = glam::DVec2::new(0.7548776662466927, 0.6558656180971502);
    let mut crossings = 0usize;
    for e in f.edges() {
        let info = e.edge_info().ok()?;
        let (a, b) = (uv(info.start), uv(info.end));
        match info.curve {
            k::Curve::Line { .. } => {
                let ab = b - a;
                let t = ((at - a).dot(ab) / ab.length_squared().max(1e-300)).clamp(0.0, 1.0);
                if (a + ab * t).distance(at) < TOL {
                    return Some(true);
                }
                let den = d.perp_dot(ab);
                if den.abs() > 1e-300 {
                    let (t, s) = ((a - at).perp_dot(ab) / den, (a - at).perp_dot(d) / den);
                    if t > 0.0 && (0.0..1.0).contains(&s) {
                        crossings += 1;
                    }
                }
            }
            k::Curve::Circle { center, axis, radius, .. } if axis.normalize().cross(n).length() < 1e-9 => {
                let c = uv(center);
                let mid = uv(e.point_at((info.first_param + info.last_param) * 0.5).ok()?);
                let full = info.start.distance(info.end) < TOL;
                let angle = |w: glam::DVec2| (w.y - c.y).atan2(w.x - c.x);
                let turn = |from: f64, to: f64| (to - from).rem_euclid(std::f64::consts::TAU);
                let (s0, m0, e0) = (angle(a), angle(mid), angle(b));
                let ccw = turn(s0, m0) < turn(s0, e0);
                let on_arc = |w: glam::DVec2| full || if ccw { turn(s0, angle(w)) <= turn(s0, e0) } else { turn(angle(w), s0) <= turn(e0, s0) };
                if on_arc(at) && (at.distance(c) - radius).abs() < TOL || a.distance(at) < TOL || b.distance(at) < TOL {
                    return Some(true);
                }
                let o = at - c;
                let (bh, cc) = (o.dot(d), o.length_squared() - radius * radius);
                let disc = bh * bh - cc;
                if disc > 0.0 {
                    let r = disc.sqrt();
                    crossings += [-bh - r, -bh + r].iter().filter(|t| **t > 0.0 && on_arc(at + d * **t)).count();
                }
            }
            _ => return None,
        }
    }
    Some(crossings % 2 == 1)
}

fn intersects_plane(q: &QueryCx, e: &Ent, pl: &PlaneFrame) -> bool {
    if let Some(s) = q.shape(e) {
        if s.shape_type() == ShapeType::Vertex {
            return s.vertex_point().is_ok_and(|p| (p - pl.origin).dot(pl.normal).abs() < 1e-8);
        }
        let Ok(face) = plane_face(pl) else { return false };
        return k::distance(&s, &face).is_ok_and(|d| d.distance < 1e-8);
    }
    match q.plane(e) {
        Some(p) => p.normal.cross(pl.normal).length() > 1e-9 || (p.origin - pl.origin).dot(pl.normal).abs() < 1e-8,
        None => false,
    }
}

fn coincides_with_plane(q: &QueryCx, e: &Ent, pl: &PlaneFrame) -> bool {
    let on = |p: DVec3| (p - pl.origin).dot(pl.normal).abs() < 1e-8;
    if let Some(p) = q.plane(e) {
        return p.normal.cross(pl.normal).length() < 1e-9 && on(p.origin);
    }
    match q.shape(e) {
        Some(s) => match s.shape_type() {
            ShapeType::Vertex => s.vertex_point().is_ok_and(on),
            ShapeType::Edge => s.edge_info().is_ok_and(|info| on(info.start) && on(info.end) && s.point_at((info.first_param + info.last_param) * 0.5).is_ok_and(on)),
            _ => false,
        },
        None => q.point(e).is_some_and(on),
    }
}

fn coincident(q: &QueryCx, a: &Ent, b: &Ent) -> bool {
    match (q.shape(a), q.shape(b)) {
        (Some(x), Some(y)) => {
            if x.is_same(&y) {
                return true;
            }
            k::distance(&x, &y).is_ok_and(|d| d.distance < 1e-8) && q.point(a).zip(q.point(b)).is_some_and(|(p, r)| (p - r).length() < 1e-7)
        }
        _ => q.point(a).zip(q.point(b)).is_some_and(|(p, r)| (p - r).length() < 1e-8),
    }
}

fn edge_end_vertex(q: &QueryCx, e: &Ent, at_start: bool) -> Option<Ent> {
    match e {
        Ent::Edge(b, x) => {
            let info = q.shape(e)?.edge_info().ok()?;
            let target = if at_start { info.start } else { info.end };
            q.vertices_of_edge(*b, *x).into_iter().min_by(|v1, v2| {
                let d = |v: &Ent| q.point(v).map(|p| (p - target).length()).unwrap_or(f64::MAX);
                d(v1).total_cmp(&d(v2))
            })
        }
        Ent::SketchEdge(s, id) if open_curve(&q.st.ctx.sketches[*s].sketch, *id) => Some(Ent::SketchCurveEnd(*s, *id, if at_start { 0 } else { 1 })),
        Ent::SketchEdge(s, id) => {
            let sk = &q.st.ctx.sketches[*s].sketch;
            let (a, b) = sk.endpoints(noshape_sketch::EntityId(*id))?;
            Some(Ent::SketchVertex(*s, if at_start { a.0 } else { b.0 }))
        }
        Ent::SketchRegionEdge(s, i) => {
            let pieces = q.region_pieces(*s);
            let (a, b) = pieces.edges.get(*i as usize)?.shape.endpoints().ok()?;
            let at = if at_start { a } else { b };
            pieces.verts.iter().position(|(p, ..)| p.distance(at) < 1e-9).map(|v| Ent::SketchRegionVertex(*s, v as u32))
        }
        _ => None,
    }
}

/// Faces meet tangentially along an edge.
fn edge_tangent(q: &QueryCx, a: &Ent, b: &Ent, e: &Ent, tol: f64) -> bool {
    let (Some(fa), Some(fb), Some(edge)) = (q.shape(a), q.shape(b), q.shape(e)) else { return false };
    let Ok(info) = edge.edge_info() else { return false };
    let ts = [0.25, 0.5, 0.75];
    ts.iter().all(|t| {
        let Ok(p) = edge.point_at(info.first_param + (info.last_param - info.first_param) * t) else { return false };
        match (fa.normal_at_point(p), fb.normal_at_point(p)) {
            (Ok(na), Ok(nb)) => na.normalize_or_zero().dot(nb.normalize_or_zero()) > (tol.cos()).min(1.0 - 1e-9),
            _ => false,
        }
    })
}

/// Convexity of the edge between two faces (`Some(true)` convex).
fn edge_convexity(q: &QueryCx, a: &Ent, b: &Ent, e: &Ent) -> Option<bool> {
    let (fa, fb, edge) = (q.shape(a)?, q.shape(b)?, q.shape(e)?);
    let info = edge.edge_info().ok()?;
    let p = edge.point_at((info.first_param + info.last_param) * 0.5).ok()?;
    let (na, nb) = (fa.normal_at_point(p).ok()?, fb.normal_at_point(p).ok()?);
    // Convex when b's centroid lies behind a's tangent plane.
    let cb = fb.centroid().ok()?;
    let ca = fa.centroid().ok()?;
    let s = (cb - p).dot(na) + (ca - p).dot(nb);
    Some(s < 0.0)
}

fn connected_faces(q: &QueryCx, seeds: Vec<Ent>, ok: impl Fn(&Ent, &Ent, &Ent) -> bool) -> Vec<Ent> {
    let mut out: Vec<Ent> = seeds.iter().filter(|e| matches!(e, Ent::Face(..))).cloned().collect();
    let mut k = 0;
    while k < out.len() {
        let f = out[k].clone();
        k += 1;
        let Ent::Face(b, fi) = f else { continue };
        for e in q.edges_of_face(b, fi) {
            let Ent::Edge(_, ei) = e else { continue };
            for g in q.faces_of_edge(b, ei) {
                if g != f && !out.contains(&g) && ok(&f, &g, &e) {
                    out.push(g);
                }
            }
        }
    }
    out
}

fn tangent_edges(q: &QueryCx, seeds: Vec<Ent>) -> Vec<Ent> {
    let mut out: Vec<Ent> = seeds.iter().filter(|e| matches!(e, Ent::Edge(..))).cloned().collect();
    let mut k = 0;
    while k < out.len() {
        let e = out[k].clone();
        k += 1;
        let Ent::Edge(b, ei) = e else { continue };
        let Some(s) = q.shape(&e) else { continue };
        let Ok(info) = s.edge_info() else { continue };
        let topo = q.topo(b);
        for v in q.vertices_of_edge(b, ei) {
            let Ent::Vertex(_, vi) = v else { continue };
            let Some(p) = q.point(&v) else { continue };
            let t_here = if (p - info.start).length() < (p - info.end).length() { s.tangent_at(info.first_param) } else { s.tangent_at(info.last_param) };
            let Ok(t_here) = t_here else { continue };
            for other in topo.vertex_edges.get(vi).into_iter().flatten() {
                let o = Ent::Edge(b, *other as usize);
                if out.contains(&o) {
                    continue;
                }
                let Some(os) = q.shape(&o) else { continue };
                let Ok(oi) = os.edge_info() else { continue };
                let t_other = if (p - oi.start).length() < (p - oi.end).length() { os.tangent_at(oi.first_param) } else { os.tangent_at(oi.last_param) };
                if let Ok(t2) = t_other
                    && t_here.normalize_or_zero().dot(t2.normalize_or_zero()).abs() > 1.0 - 1e-6
                {
                    out.push(o);
                }
            }
        }
    }
    out
}

/// Faces of the holes a seed face belongs to: connected cylindrical/conical
/// faces sharing its axis, plus planar faces bounded by them (counterbores).
fn hole_faces(q: &QueryCx, seeds: Vec<Ent>) -> Vec<Ent> {
    let mut out = Vec::new();
    for s in seeds {
        let Some((o, d)) = entity_axis(q, &s) else { continue };
        let same_axis = |e: &Ent| entity_axis(q, e).is_some_and(|(p, a)| a.dot(d).abs() > 1.0 - 1e-9 && ((p - o) - d * (p - o).dot(d)).length() < 1e-8);
        out.extend(connected_faces(q, vec![s.clone()], |_, g, _| same_axis(g) || q.plane(g).is_some_and(|pl| pl.normal.dot(d).abs() > 1.0 - 1e-9 && false)));
    }
    uniq(out)
}

fn fillet_faces(q: &QueryCx, seeds: Vec<Ent>, compare: Arc<str>) -> Vec<Ent> {
    let radius = |e: &Ent| match q.shape(e).and_then(|s| s.face_surface().ok()) {
        Some(k::Surface::Cylinder { radius, .. }) => Some(radius),
        Some(k::Surface::Torus { minor_radius, .. }) => Some(minor_radius),
        Some(k::Surface::Sphere { radius, .. }) => Some(radius),
        _ => None,
    };
    let mut out = Vec::new();
    for s in &seeds {
        let Some(r0) = radius(s) else { continue };
        let Ent::Face(b, _) = s else { continue };
        for f in 0..q.st.ctx.bodies[*b].faces.len() {
            let e = Ent::Face(*b, f);
            let Some(r) = radius(&e) else { continue };
            let tol = 1e-9;
            let ok = match &*compare {
                "EQUAL" => (r - r0).abs() < tol,
                "LESS" => r < r0 - tol,
                "LESS_EQUAL" => r <= r0 + tol,
                "GREATER" => r > r0 + tol,
                _ => r >= r0 - tol,
            };
            if ok {
                out.push(e);
            }
        }
    }
    uniq(out)
}

/// Faces reachable from the seed faces without crossing the seed edges.
fn bounded_faces(q: &QueryCx, seeds: Vec<Ent>) -> Vec<Ent> {
    let walls: Vec<Ent> = seeds.iter().filter(|e| !matches!(e, Ent::Face(..))).cloned().collect();
    let faces: Vec<Ent> = seeds.iter().filter(|e| matches!(e, Ent::Face(..))).cloned().collect();
    let first: Vec<Ent> = faces.first().cloned().into_iter().collect();
    let bounding_faces: Vec<Ent> = faces.iter().skip(1).cloned().collect();
    let mut out = connected_faces(q, first, |_, g, e| !walls.contains(e) && !bounding_faces.contains(g));
    out.retain(|f| !bounding_faces.contains(f));
    out
}

// ---------------------------------------------------------------- natives

/// Evaluate a query against a context to persistent references.
pub fn evaluate_refs(i: &mut Interp, cx: &FsContext, query: &Value) -> R<Vec<Reference>> {
    let st = cx.state.lock();
    let q = QueryCx::new(&st);
    let ents = eval(i, &q, query)?;
    Ok(ents.iter().map(|e| q.to_ref(e)).collect())
}

/// Onshape's evaluateQuery order where we know it: ascending transient id, the order entities were made in (live
/// 1995). A body's edges along a sketch's curves were made in that sketch's order: a region's boundary keeps it
/// through the extrude and the extract that make a sheet metal definition (cf-sheet-metal-overcut-dogbone's
/// definition sheet lists Sketch 1's eight lines as drawn; std's dogbone takes the first of a corner's two edges).
/// Those edges of sheets are sorted among themselves, everything else keeps its place, in what a query works out
/// (qEverything, qOwnedByBody, qAdjacent; qIntersection keeps its first list's order): a union keeps its subqueries'
/// (a Flange's picked edges, in pick order: Butt joint - Direction 1 follows the first, live 1599).
pub fn live_order(q: &QueryCx, ents: &mut [Ent]) {
    let keyed: Vec<(usize, (u32, u32))> = ents
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            Ent::Edge(b, ei) => q.sketch_keys(*b).1.get(*ei).copied().flatten().map(|k| (i, k)),
            _ => None,
        })
        .collect();
    if keyed.len() < 2 || keyed.windows(2).all(|w| w[0].1 <= w[1].1) {
        return;
    }
    let mut sorted: Vec<((u32, u32), Ent)> = keyed.iter().map(|(i, k)| (*k, ents[*i].clone())).collect();
    sorted.sort_by_key(|(k, _)| *k);
    for ((at, _), (_, e)) in keyed.iter().zip(sorted) {
        ents[*at] = e;
    }
}

/// Evaluate to entities and run `f` with the query context.
pub fn with_ents<T>(i: &mut Interp, cx: &FsContext, query: &Value, f: impl FnOnce(&mut Interp, &QueryCx, Vec<Ent>) -> R<T>) -> R<T> {
    let st = cx.state.lock();
    let q = QueryCx::new(&st);
    let ents = eval(i, &q, query)?;
    f(i, &q, ents)
}

pub fn transient_query(id: Arc<str>) -> Value {
    let qt = std_enum("query.fs", "QueryType", "TRANSIENT").unwrap_or_else(|| Value::str("TRANSIENT"));
    let mut m = Map::new();
    m.insert(Value::str("queryType"), qt);
    m.insert(Value::str("transientId"), Value::str(id));
    Value::Map(Arc::new(m), std_tag("query.fs", "Query"))
}

/// Transient queries for references (interning their ids).
pub fn transient_queries(cx: &FsContext, refs: &[Reference]) -> Value {
    let mut st = cx.state.lock();
    Value::array(refs.iter().map(|r| transient_query(st.transient(r))).collect())
}

/// A query (union of transient queries) selecting `refs`.
pub fn query_of_refs(cx: &FsContext, refs: &[Reference]) -> Value {
    let qt = std_enum("query.fs", "QueryType", "UNION").unwrap_or_else(|| Value::str("UNION"));
    Value::map_from([("queryType", qt), ("subqueries", transient_queries(cx, refs))]).with_tag(std_tag("query.fs", "Query"))
}

fn query_arg(a: &[Value]) -> Value {
    let d = a.get(1).cloned().unwrap_or_default();
    match d.get("query") {
        Value::Undefined => d,
        q => q.clone(),
    }
}

fn evaluate_query(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let refs = evaluate_refs(i, &cx, &query_arg(&a))?;
    Ok(transient_queries(&cx, &refs))
}

fn evaluate_query_count(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let n = with_ents(i, &cx, &query_arg(&a), |_, _, e| Ok(e.len()))?;
    Ok(Value::num(n as f64))
}

fn is_query_empty(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let n = with_ents(i, &cx, &query_arg(&a), |_, _, e| Ok(e.len()))?;
    Ok(Value::bool(n == 0))
}

fn transient_id_to_string(_: &mut Interp, a: Vec<Value>) -> R<Value> {
    Ok(a.get(1).or(a.first()).cloned().unwrap_or_default())
}

fn get_query_variable(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let name = a.get(1).map(|d| d.get("name").clone()).unwrap_or_default().as_str().unwrap_or_default();
    let refs = cx.state.lock().ctx.query_variables.get(&*name).cloned();
    match refs {
        Some(r) => Ok(query_of_refs(&cx, &r)),
        None => Err(super::raise_error(i, "VARIABLE_CANNOT_RESOLVE", None)),
    }
}

fn set_query_variable(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let name = def.get("name").as_str().unwrap_or_default();
    let refs = evaluate_refs(i, &cx, def.get("value"))?;
    cx.state.lock().ctx.query_variables.insert(name.to_string(), refs);
    Ok(Value::Undefined)
}

/// Body ids of body entities.
pub fn body_ids(q: &QueryCx, ents: &[Ent]) -> Vec<BodyId> {
    ents.iter().filter_map(|e| if let Ent::Body(b) = e { Some(q.st.ctx.bodies[*b].id.clone()) } else { None }).collect()
}

pub fn kernel_error(e: k::KernelError) -> String {
    kerr(e)
}

/// Whether face name `n` is made from `of`: an offset, a region generated from, a copy or a piece of it, at any depth.
fn derives_from(n: &TopoName, of: &TopoName) -> bool {
    if n == of {
        return true;
    }
    match n {
        TopoName::Face { role: Role::Offset(x) | Role::Generated(x) | Role::Copy { of: x, .. }, .. } | TopoName::Split { parent: x, .. } => derives_from(x, of),
        _ => false,
    }
}
