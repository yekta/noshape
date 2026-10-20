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
use glam::DVec3;
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::{Map, Value};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::cell::RefCell;
use std::collections::HashMap;
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
    /// `ctx.planes[i]` (construction planes).
    Plane(usize),
    /// `ctx.connectors[i]` (mate connectors).
    Connector(usize),
    /// `ctx.points[i]`.
    Point(usize),
    /// `ctx.sketches[i]` and its regions, curves and points.
    SketchBody(usize),
    SketchRegion(usize, Vec<u32>),
    SketchEdge(usize, u32),
    SketchVertex(usize, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        Ent::Body(_) | Ent::SketchBody(_) | Ent::Connector(_) => EType::Body,
        Ent::Face(..) | Ent::Datum(_) | Ent::Plane(_) | Ent::SketchRegion(..) => EType::Face,
        Ent::Edge(..) | Ent::SketchEdge(..) => EType::Edge,
        Ent::Vertex(..) | Ent::Origin | Ent::Point(_) | Ent::SketchVertex(..) => EType::Vertex,
    }
}

/// Body/sub-shape topology of one body, computed on demand.
#[derive(Default)]
struct Topo {
    edge_faces: Vec<Vec<u32>>,
    vertex_faces: Vec<Vec<u32>>,
    vertex_edges: Vec<Vec<u32>>,
    edge_vertices: Vec<Vec<u32>>,
}

/// Evaluation state of one query.
/// An entity's type and a few points on it (see [`CtxState::snapshots`]).
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub etype: EType,
    pub points: Vec<DVec3>,
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
        let shape = &self.st.ctx.bodies[b].shape;
        let vertex_edges = shape.vertex_edges().unwrap_or_default();
        let mut edge_vertices = vec![Vec::new(); shape.count(ShapeType::Edge)];
        for (v, es) in vertex_edges.iter().enumerate() {
            for e in es {
                if let Some(ev) = edge_vertices.get_mut(*e as usize) {
                    ev.push(v as u32);
                }
            }
        }
        let t = Arc::new(Topo { edge_faces: shape.edge_faces().unwrap_or_default(), vertex_faces: shape.vertex_faces().unwrap_or_default(), vertex_edges, edge_vertices });
        self.topo.borrow_mut().insert(b, t.clone());
        t
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
            out.extend((0..ctx.sketches.len()).map(Ent::SketchBody));
            out.extend((0..ctx.connectors.len()).map(Ent::Connector));
        }
        if want(EType::Face) {
            out.extend([DatumPlane::Top, DatumPlane::Front, DatumPlane::Right].map(Ent::Datum));
            out.extend((0..ctx.planes.len()).map(Ent::Plane));
            for (b, body) in ctx.bodies.iter().enumerate().filter(|(b, _)| shown(b)) {
                out.extend((0..body.faces.len()).map(|f| Ent::Face(b, f)));
            }
            for (s, sk) in ctx.sketches.iter().enumerate() {
                out.extend(sk.regions.iter().map(|r| Ent::SketchRegion(s, encode_region_key(&r.id))));
            }
        }
        if want(EType::Edge) {
            for (b, body) in ctx.bodies.iter().enumerate().filter(|(b, _)| shown(b)) {
                out.extend((0..body.edges.len()).map(|e| Ent::Edge(b, e)));
            }
            for (s, sk) in ctx.sketches.iter().enumerate() {
                out.extend(sk.sketch.entities().filter(|e| e.kind().is_curve()).map(|e| Ent::SketchEdge(s, e.id.0)));
            }
        }
        if want(EType::Vertex) {
            out.push(Ent::Origin);
            out.extend((0..ctx.points.len()).map(Ent::Point));
            for (b, body) in ctx.bodies.iter().enumerate().filter(|(b, _)| shown(b)) {
                out.extend((0..body.vertices.len()).map(|v| Ent::Vertex(b, v)));
            }
            for (s, sk) in ctx.sketches.iter().enumerate() {
                out.extend(sk.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).map(|e| Ent::SketchVertex(s, e.id.0)));
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
                    out.extend((0..body.faces.len()).map(|f| Ent::Face(*b, f)));
                }
                if want(EType::Edge) {
                    out.extend((0..body.edges.len()).map(|e| Ent::Edge(*b, e)));
                }
                if want(EType::Vertex) {
                    out.extend((0..body.vertices.len()).map(|v| Ent::Vertex(*b, v)));
                }
            }
            Ent::SketchBody(s) => {
                let sk = &self.st.ctx.sketches[*s];
                if want(EType::Body) {
                    out.push(Ent::SketchBody(*s));
                }
                if want(EType::Face) {
                    out.extend(sk.regions.iter().map(|r| Ent::SketchRegion(*s, encode_region_key(&r.id))));
                }
                if want(EType::Edge) {
                    out.extend(sk.sketch.entities().filter(|e| e.kind().is_curve()).map(|e| Ent::SketchEdge(*s, e.id.0)));
                }
                if want(EType::Vertex) {
                    out.extend(sk.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).map(|e| Ent::SketchVertex(*s, e.id.0)));
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
            Ent::Body(_) | Ent::SketchBody(_) | Ent::Connector(_) => Some(e.clone()),
            Ent::Face(b, _) | Ent::Edge(b, _) | Ent::Vertex(b, _) => Some(Ent::Body(*b)),
            Ent::SketchRegion(s, _) | Ent::SketchEdge(s, _) | Ent::SketchVertex(s, _) => Some(Ent::SketchBody(*s)),
            _ => None,
        }
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
            Ent::Plane(i) => ctx.planes[*i].reference.clone(),
            Ent::Connector(i) => ctx.connectors[*i].reference.clone(),
            Ent::Point(i) => ctx.points[*i].reference.clone(),
            Ent::SketchBody(s) => Reference::Sketch(ctx.sketches[*s].feature),
            Ent::SketchRegion(s, key) => Reference::SketchRegion { sketch: ctx.sketches[*s].feature, key: key.clone() },
            Ent::SketchEdge(s, id) | Ent::SketchVertex(s, id) => Reference::SketchEntity { sketch: ctx.sketches[*s].feature, entity: *id },
        }
    }

    /// Entities a persistent reference resolves to now.
    pub fn from_ref(&self, r: &Reference) -> Vec<Ent> {
        let ctx = &self.st.ctx;
        match r {
            Reference::Body(id) => ctx.bodies.iter().position(|b| &b.id == id).map(Ent::Body).into_iter().collect(),
            Reference::Face(n) => ctx.bodies.iter().enumerate().flat_map(|(b, body)| body.find_faces(n).into_iter().map(move |f| Ent::Face(b, f))).collect(),
            Reference::Edge(n) => ctx.bodies.iter().enumerate().flat_map(|(b, body)| body.find_edges(n).into_iter().map(move |f| Ent::Edge(b, f))).collect(),
            Reference::Vertex(n) => ctx.bodies.iter().enumerate().flat_map(|(b, body)| body.find_vertices(n).into_iter().map(move |f| Ent::Vertex(b, f))).collect(),
            Reference::Origin => vec![Ent::Origin],
            Reference::Plane(p) => vec![Ent::Datum(*p)],
            Reference::Sketch(f) => ctx.sketches.iter().position(|s| s.feature == *f).map(Ent::SketchBody).into_iter().collect(),
            Reference::SketchRegion { sketch, key } => {
                let Some(s) = ctx.sketches.iter().position(|s| s.feature == *sketch) else { return Vec::new() };
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
                let Some(s) = ctx.sketches.iter().position(|s| s.feature == *sketch) else { return Vec::new() };
                match ctx.sketches[s].sketch.kind(noshape_sketch::EntityId(*entity)) {
                    Some(noshape_sketch::EntityKind::Point) => vec![Ent::SketchVertex(s, *entity)],
                    Some(_) => vec![Ent::SketchEdge(s, *entity)],
                    None => Vec::new(),
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
            Ent::SketchRegion(s, key) => {
                let sk = &ctx.sketches[*s];
                let region = sk.regions.iter().find(|r| encode_region_key(&r.id) == *key)?;
                crate::regen::eval::region_profile(sk, region, crate::reference::Source::Index(0)).ok().map(|p| p.shape)
            }
            Ent::SketchBody(s) => {
                let sk = &ctx.sketches[*s];
                let edges: Vec<Shape> = sk.sketch.entities().filter(|x| x.kind().is_curve()).filter_map(|x| sketch_edge_shape(sk, x.id.0)).collect();
                Some(Shape::compound(&edges))
            }
            Ent::Datum(_) | Ent::Plane(_) | Ent::Connector(_) => None,
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
            Ent::Plane(i) => Some(ctx.planes[*i].frame),
            Ent::SketchRegion(s, _) | Ent::SketchBody(s) => Some(ctx.sketches[*s].frame),
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
                Some(PlaneFrame { origin, x: pl.unit_x(), y: n.cross(pl.unit_x()), normal: n })
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
            Ent::Edge(..) => {
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
            Some(Ent::SketchBody(_)) => match e {
                Ent::SketchRegion(..) => "SHEET",
                Ent::SketchVertex(..) => "POINT",
                _ => "WIRE",
            },
            _ => match e {
                Ent::Datum(_) | Ent::Plane(_) => "SHEET",
                _ => "POINT",
            },
        }
    }

    pub fn is_construction(&self, e: &Ent) -> bool {
        match e {
            Ent::Datum(_) | Ent::Plane(_) | Ent::Origin | Ent::Point(_) => true,
            Ent::SketchEdge(s, id) | Ent::SketchVertex(s, id) => self.st.ctx.sketches[*s].sketch.entity(noshape_sketch::EntityId(*id)).is_some_and(|x| x.construction),
            _ => false,
        }
    }

    pub fn is_sketch(&self, e: &Ent) -> bool {
        matches!(e, Ent::SketchBody(_) | Ent::SketchRegion(..) | Ent::SketchEdge(..) | Ent::SketchVertex(..))
    }

    pub fn is_modifiable(&self, e: &Ent) -> bool {
        matches!(e, Ent::Body(_) | Ent::Face(..) | Ent::Edge(..) | Ent::Vertex(..))
    }

    /// A representative point (vertex position, curve / face / body centroid).
    pub fn point(&self, e: &Ent) -> Option<DVec3> {
        match e {
            Ent::Origin => Some(DVec3::ZERO),
            Ent::Point(i) => Some(self.st.ctx.points[*i].position),
            Ent::Connector(i) => Some(self.st.ctx.connectors[*i].origin),
            Ent::Datum(_) | Ent::Plane(_) => self.plane(e).map(|p| p.origin),
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
        let shape = match e {
            Ent::Face(..) | Ent::Edge(..) | Ent::Vertex(..) => self.shape(e)?,
            _ => return None,
        };
        let points = match etype {
            EType::Vertex => vec![shape.vertex_point().ok()?],
            EType::Edge => {
                let i = shape.edge_info().ok()?;
                [0.0, 0.5, 1.0].iter().filter_map(|t| shape.point_at(i.first_param + (i.last_param - i.first_param) * t).ok()).collect()
            }
            _ => {
                let [u0, u1, v0, v1] = shape.uv_bounds().ok()?;
                let p = shape.point_at_uv((u0 + u1) * 0.5, (v0 + v1) * 0.5).ok()?;
                vec![shape.project_point(p).ok()?.point]
            }
        };
        Some(Snapshot { etype, points })
    }

    /// Current entities a tracked entity became: those of its type lying on
    /// its old points (renamed or merged), or on the old edge between its
    /// ends (split pieces).
    fn tracked(&self, snap: &Snapshot) -> Vec<Ent> {
        const TOL: f64 = 1e-7;
        let on = |e: &Ent, p: DVec3| self.shape(e).and_then(|s| k::make_vertex(p).ok().and_then(|v| k::distance(&v, &s).ok())).is_some_and(|d| d.distance < TOL);
        self.all(Some(snap.etype))
            .into_iter()
            .filter(|e| {
                if snap.points.iter().all(|p| on(e, *p)) {
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
        if let Some(pl) = (matches!(e, Ent::Datum(_) | Ent::Plane(_))).then(|| self.plane(e)).flatten() {
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
        let face_id = |n: &TopoName| -> Option<(u32, FsId)> {
            if let Some(op) = self.st.op_of_face(n) {
                return Some((op.seq, op.id.clone()));
            }
            n.creator().and_then(|f| feature_id(f).map(|i| (0, i)))
        };
        let newest = |faces: &[Arc<TopoName>]| faces.iter().filter_map(|f| face_id(f)).max_by_key(|x| x.0).map(|x| x.1);
        match e {
            Ent::Datum(DatumPlane::Top) => Some(vec!["Top".into()]),
            Ent::Datum(DatumPlane::Front) => Some(vec!["Front".into()]),
            Ent::Datum(DatumPlane::Right) => Some(vec!["Right".into()]),
            Ent::Origin => Some(vec!["Origin".into()]),
            Ent::Body(b) => self.st.body_creator(&ctx.bodies[*b].id),
            Ent::Face(b, f) => face_id(&ctx.bodies[*b].faces[*f]).map(|x| x.1),
            // Edges and vertices named without faces (wire and point
            // bodies) were created with their body.
            Ent::Edge(b, i) => match &*ctx.bodies[*b].edges[*i] {
                TopoName::Edge { faces, .. } => newest(faces),
                TopoName::Boundary { face, .. } => face_id(face).map(|x| x.1),
                TopoName::Vertex { faces, .. } => newest(faces),
                other => face_id(other).map(|x| x.1),
            }
            .or_else(|| self.created_by(&Ent::Body(*b))),
            Ent::Vertex(b, i) => match &*ctx.bodies[*b].vertices[*i] {
                TopoName::Vertex { faces, .. } => newest(faces),
                other => face_id(other).map(|x| x.1),
            }
            .or_else(|| self.created_by(&Ent::Body(*b))),
            Ent::SketchBody(s) | Ent::SketchRegion(s, _) | Ent::SketchEdge(s, _) | Ent::SketchVertex(s, _) => feature_id(ctx.sketches[*s].feature),
            Ent::Plane(_) | Ent::Connector(_) | Ent::Point(_) => self.to_ref(e).depends_on().and_then(feature_id),
        }
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
    fn adjacent(&self, e: &Ent, t: EType, by_vertex: bool) -> Vec<Ent> {
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

fn same_box(a: &noshape_sketch::BBox, b: &noshape_sketch::BBox) -> bool {
    (a.min - b.min).length() < 1e-9 && (a.max - b.max).length() < 1e-9
}

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

fn uniq(v: Vec<Ent>) -> Vec<Ent> {
    let mut seen = std::collections::HashSet::new();
    v.into_iter().filter(|e| seen.insert(e.clone())).collect()
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
        "EVERYTHING" => q.all(entity_type),
        "TRANSIENT" => {
            let id = query.get("transientId").as_str().unwrap_or_default();
            match q.st.transient_ref(&id) {
                Some(r) => q.from_ref(r),
                None => Vec::new(),
            }
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
            filter(q.all(None).into_iter().filter(|e| q.created_by(e).is_some_and(|c| has_prefix(&c, &id)) || merged(e)).collect())
        }
        "SKETCH_REGION" => {
            let Some(id) = id_of(query.get("featureId")) else { return Ok(Vec::new()) };
            let filter_inner = query.get("filterInnerLoops").as_bool().unwrap_or(false);
            let mut out = Vec::new();
            for (s, sk) in q.st.ctx.sketches.iter().enumerate() {
                if q.created_by(&Ent::SketchBody(s)).is_some_and(|c| has_prefix(&c, &id)) {
                    for r in &sk.regions {
                        // Inner loops: regions inside another region's hole.
                        if filter_inner && sk.regions.iter().any(|o| o.holes.iter().any(|h| same_box(&h.bbox(), &r.outer.bbox()))) {
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
            let cap = query.get("capType").as_str().unwrap_or_else(|| "EITHER".into());
            let faces: Vec<Ent> = q.all(Some(EType::Face)).into_iter().filter(|e| q.cap_of(e).is_some_and(|(c, op)| has_prefix(&op, &id) && (&*cap == "EITHER" || c == &*cap))).collect();
            match entity_type {
                None | Some(EType::Face) => faces,
                Some(t) => {
                    let mut out = Vec::new();
                    for f in &faces {
                        let (edges, verts) = q.boundary(f);
                        out.extend(if t == EType::Edge { edges } else if t == EType::Vertex { verts } else { q.owner(f).into_iter().collect() });
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
            } else {
                let owners: Vec<Ent> = parts.iter().filter_map(|p| q.owner(p)).collect();
                out = sub(i, "subquery")?.into_iter().filter(|e| q.owner(e).is_some_and(|o| owners.contains(&o))).collect();
            }
            uniq(out)
        }
        "OWNER_PART" => uniq(eval(i, q, query.get("query"))?.iter().filter_map(|e| q.owner(e)).collect()),
        "BODY_TYPE" => {
            let types: Vec<Arc<str>> = match query.get("bodyType") {
                Value::Array(a, _) => a.iter().filter_map(enum_str).collect(),
                v => enum_str(v).into_iter().collect(),
            };
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
        "COMPOSITE_PART_TYPE_FITLER" | "CONSUMED" | "TOLERANCE_FILTER" => sub(i, "subquery")?,
        "OP_HOLE_FACE" | "OP_HOLE_PROFILE" => filter(super::hole::op_hole_query(i, q, query, &*kind == "OP_HOLE_FACE")?),
        "CONTAINED_IN_COMPOSITE" | "COMPOSITE_CONTAINING" | "SOURCE_MESH" | "PATTERN_INSTANCES" => Vec::new(),
        "VERTEX_ADJACENT" | "EDGE_ADJACENT" => {
            let t = entity_type.unwrap_or(EType::Face);
            let mut out = Vec::new();
            for e in eval(i, q, query.get("query"))? {
                out.extend(q.adjacent(&e, t, &*kind == "VERTEX_ADJACENT"));
            }
            uniq(out)
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
                    let Ent::Edge(b, x) = e else { return &*t == "WIRE" && matches!(e, Ent::SketchEdge(..)) };
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
            let v: Vec<(Ent, f64)> = sub(i, "subquery")?.into_iter().filter_map(|e| q.measure(&e).map(|m| (e, m))).collect();
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
        "DEPENDENCY" | "LAMINAR_DEPENDENCY" | "PATTERN" => sub(i, "subquery")?,
        "TRACKING" => {
            let mut out = Vec::new();
            for key in ["subquery1", "subquery2"] {
                let items: Vec<Value> = match query.get(key) {
                    Value::Array(a, _) => a.iter().cloned().collect(),
                    Value::Undefined => Vec::new(),
                    v => vec![v.clone()],
                };
                for item in items {
                    let found = eval(i, q, &item)?;
                    // Renamed by an operation since: follow its geometry.
                    if found.is_empty()
                        && let Some(snap) = item.get("transientId").as_str().and_then(|id| q.st.snapshots.get(&*id))
                    {
                        out.extend(q.tracked(snap));
                    }
                    out.extend(found);
                }
            }
            uniq(out)
        }
        "HISTORICAL" => Vec::new(),
        "ATTRIBUTE_FILTER" => {
            let base = if query.get("subquery").is_undefined() { q.all_with(None, true) } else { sub(i, "subquery")? };
            let name = query.get("name").clone();
            let pattern = query.get("attributePattern").clone();
            let exact = query.get("valueToMatchExactly").clone();
            base.into_iter()
                .filter(|e| {
                    let r = q.to_ref(e);
                    q.st.attributes.iter().any(|(x, v)| {
                        *x == r && (name.is_undefined() || *v.get("name") == name) && (exact.is_undefined() || *v.get("attribute") == exact) && attr_matches(v.get("attribute"), &pattern)
                    })
                })
                .collect()
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
    Ok(r)
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
        let Some(s) = q.st.ctx.sketches.iter().position(|x| x.feature == sm) else { return Vec::new() };
        return match t {
            Some(EType::Edge) => q.owned(&Ent::SketchBody(s), Some(EType::Edge)),
            Some(EType::Vertex) => q.owned(&Ent::SketchBody(s), Some(EType::Vertex)),
            _ => q.owned(&Ent::SketchBody(s), Some(EType::Face)),
        };
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
        "SKETCH_ENTITY" => {
            let Some(s) = q.st.ctx.sketches.iter().position(|x| x.feature == marker) else { return Vec::new() };
            let name = query.get("sketchEntityId").as_str().unwrap_or_default();
            let Some(e) = super::sketch::sketch_entity(q.st, marker, &name) else { return Vec::new() };
            let sk = &q.st.ctx.sketches[s].sketch;
            let is_point = sk.kind(noshape_sketch::EntityId(e)) == Some(noshape_sketch::EntityKind::Point);
            let me = if is_point { Ent::SketchVertex(s, e) } else { Ent::SketchEdge(s, e) };
            match t {
                None => vec![me],
                Some(EType::Body) => vec![Ent::SketchBody(s)],
                Some(EType::Edge) if !is_point => vec![me],
                Some(EType::Vertex) if is_point => vec![me],
                Some(EType::Vertex) => sk.entity(noshape_sketch::EntityId(e)).map(|x| x.point_refs().iter().map(|p| Ent::SketchVertex(s, p.0)).collect()).unwrap_or_default(),
                Some(EType::Face) => q.st.ctx.sketches[s].regions.iter().filter(|r| r.id.entities().contains(&noshape_sketch::EntityId(e))).map(|r| Ent::SketchRegion(s, encode_region_key(&r.id))).collect(),
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
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
pub fn entity_axis(q: &QueryCx, e: &Ent) -> Option<(DVec3, DVec3)> {
    let s = q.shape(e)?;
    match entity_type_of(e) {
        EType::Edge => match s.edge_info().ok()?.curve {
            k::Curve::Line { origin, direction } => Some((origin, direction.normalize_or_zero())),
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
            if matches!(e, Ent::Datum(_) | Ent::Plane(_)) {
                return q.plane(e).is_some_and(|pl| (p - pl.origin).dot(pl.normal).abs() < 1e-8);
            }
            // OCCT entities carry ~1e-7 tolerances after booleans and blends.
            q.distance_to_point(e, p).is_some_and(|d| d < 1e-7)
        }
    }
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
        Ent::SketchEdge(s, id) => {
            let sk = &q.st.ctx.sketches[*s].sketch;
            let (a, b) = sk.endpoints(noshape_sketch::EntityId(*id))?;
            Some(Ent::SketchVertex(*s, if at_start { a.0 } else { b.0 }))
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
