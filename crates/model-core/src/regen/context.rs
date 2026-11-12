//! Regeneration context: the state of a Part Studio after a feature.

use serde::{Deserialize, Serialize};
use crate::display::{BodyKind, PlaneFrame};
use crate::naming::{GeoKey, Name, name_edges, name_vertices};
use crate::reference::{BodyId, FeatureId, Reference, TopoName};
use glam::DVec3;
use noshape_kernel::{Shape, ShapeType};
use noshape_units::Quantity;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::{Arc, OnceLock};

/// A part, surface or wire body with persistent names for its sub-shapes.
#[derive(Serialize, Deserialize)]
pub struct Body {
    pub id: BodyId,
    pub name: String,
    pub kind: BodyKind,
    pub shape: Shape,
    #[serde(with = "shared_names")]
    pub faces: Vec<Name>,
    #[serde(with = "shared_names")]
    pub edges: Vec<Name>,
    #[serde(with = "shared_names")]
    pub vertices: Vec<Name>,
    /// Index into the part color palette.
    pub color: usize,
    /// Set on Composite part bodies.
    pub composite: Option<crate::display::CompositeInfo>,
    #[serde(skip)]
    face_lookup: OnceLock<HashMap<Name, usize>>,
    #[serde(skip)]
    edge_lookup: OnceLock<HashMap<Name, usize>>,
    #[serde(skip)]
    vertex_lookup: OnceLock<HashMap<Name, usize>>,
}

impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Body").field("id", &self.id).field("name", &self.name).field("faces", &self.faces.len()).finish()
    }
}

/// Geometric ordering key of one face: bounding-box center and diagonal.
/// Cheap (no surface integration) and deterministic.
pub fn face_key(shape: &Shape, i: usize) -> GeoKey {
    shape.sub_shape(ShapeType::Face, i).ok().and_then(|f| f.bounding_box(false).ok()).map(|b| {
        let c = b.center();
        [c.x, c.y, c.z, b.diagonal()]
    }).unwrap_or([0.0; 4])
}

pub fn edge_key(shape: &Shape, i: usize) -> GeoKey {
    match shape.sub_shape(ShapeType::Edge, i).and_then(|e| e.edge_info().map(|info| (e, info))) {
        Ok((e, i)) => {
            let m = e.point_at((i.first_param + i.last_param) * 0.5).unwrap_or((i.start + i.end) * 0.5);
            [m.x, m.y, m.z, i.length]
        }
        Err(_) => [0.0; 4],
    }
}

pub fn vertex_key(shape: &Shape, i: usize) -> GeoKey {
    shape.sub_shape(ShapeType::Vertex, i).ok().and_then(|v| v.vertex_point().ok()).map(|p| [p.x, p.y, p.z, 0.0]).unwrap_or([0.0; 4])
}

impl Body {
    /// Build a body from its shape and face names; edge and vertex names are
    /// derived from face adjacency.
    pub fn new(id: BodyId, name: String, kind: BodyKind, shape: Shape, mut faces: Vec<Name>, color: usize, wire_feature: FeatureId) -> Self {
        join_face_names(&shape, &mut faces);
        let (edges, vertices) = if faces.is_empty() {
            // Wire / point bodies: name edges by index from the creating feature.
            let ne = shape.count(ShapeType::Edge);
            let nv = shape.count(ShapeType::Vertex);
            let e = (0..ne).map(|i| Arc::new(TopoName::Wire { feature: wire_feature, source: crate::reference::Source::Index(i as u32) })).collect();
            let v = (0..nv).map(|i| Arc::new(TopoName::Wire { feature: wire_feature, source: crate::reference::Source::Index(1_000_000 + i as u32) })).collect();
            (e, v)
        } else {
            let ef = shape.edge_faces().unwrap_or_default();
            let vf = shape.vertex_faces().unwrap_or_default();
            (name_edges(&faces, &ef, &|i| edge_key(&shape, i), &joined_pieces(&shape)), name_vertices(&faces, &vf, &|i| vertex_key(&shape, i)))
        };
        Self {
            id,
            name,
            kind,
            shape,
            faces,
            edges,
            vertices,
            color,
            composite: None,
            face_lookup: OnceLock::new(),
            edge_lookup: OnceLock::new(),
            vertex_lookup: OnceLock::new(),
        }
    }

    /// [`Body::new`] with the edge and vertex names already derived from the
    /// faces' ([`super::eval::names_for`], as `new` derives them): a part's
    /// 5000 edges and 3300 vertices named once, not twice.
    pub fn named(id: BodyId, name: String, kind: BodyKind, shape: Shape, names: crate::naming::ShapeNames, color: usize, wire_feature: FeatureId) -> Self {
        if names.faces.is_empty() || names.edges.len() != shape.count(ShapeType::Edge) || names.vertices.len() != shape.count(ShapeType::Vertex) {
            return Body::new(id, name, kind, shape, names.faces, color, wire_feature);
        }
        // (Faces shown as one: one name, and the edges and vertices named again from it.)
        let mut joined = names.faces.clone();
        if join_face_names(&shape, &mut joined) {
            return Body::new(id, name, kind, shape, joined, color, wire_feature);
        }
        Self {
            id,
            name,
            kind,
            shape,
            faces: names.faces,
            edges: names.edges,
            vertices: names.vertices,
            color,
            composite: None,
            face_lookup: OnceLock::new(),
            edge_lookup: OnceLock::new(),
            vertex_lookup: OnceLock::new(),
        }
    }

    pub fn renamed(&self, name: String) -> Body {
        Body { name, ..self.clone_shallow() }
    }

    fn clone_shallow(&self) -> Body {
        Body {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind,
            shape: self.shape.clone(),
            faces: self.faces.clone(),
            edges: self.edges.clone(),
            vertices: self.vertices.clone(),
            color: self.color,
            composite: self.composite.clone(),
            face_lookup: OnceLock::new(),
            edge_lookup: OnceLock::new(),
            vertex_lookup: OnceLock::new(),
        }
    }

    fn lookup<'a>(cell: &'a OnceLock<HashMap<Name, usize>>, names: &[Name]) -> &'a HashMap<Name, usize> {
        cell.get_or_init(|| names.iter().enumerate().map(|(i, n)| (n.clone(), i)).collect())
    }

    /// Whether a face, edge or vertex of this body has exactly the name `n`
    /// (the first, cheap step of `find_faces` / `find_edges` / `find_vertices`).
    pub fn has_exact(&self, n: &TopoName) -> bool {
        Self::lookup(&self.face_lookup, &self.faces).contains_key(n)
            || Self::lookup(&self.edge_lookup, &self.edges).contains_key(n)
            || Self::lookup(&self.vertex_lookup, &self.vertices).contains_key(n)
    }

    /// Indices of faces matching a name: exact, else all split descendants.
    pub fn find_faces(&self, n: &TopoName) -> Vec<usize> {
        let map = Self::lookup(&self.face_lookup, &self.faces);
        if let Some(&i) = map.get(n) {
            // (Pieces shown as one face share its name, one Arc: all of them. Only when some name repeats.)
            if map.len() == self.faces.len() {
                return vec![i];
            }
            return (0..self.faces.len()).filter(|&j| j == i || Arc::ptr_eq(&self.faces[j], &self.faces[i])).collect();
        }
        let pieces: Vec<usize> = self.faces.iter().enumerate().filter(|(_, f)| f.descends_from(n)).map(|(i, _)| i).collect();
        if !pieces.is_empty() {
            return pieces;
        }
        // A piece of a face that is whole again (the feature that split it
        // was suppressed or changed): the whole face, like Onshape.
        self.faces.iter().enumerate().filter(|(_, f)| n.descends_from(f)).map(|(i, _)| i).collect()
    }

    pub fn find_edges(&self, n: &TopoName) -> Vec<usize> {
        let map = Self::lookup(&self.edge_lookup, &self.edges);
        if let Some(&i) = map.get(n) {
            // (Pieces shown as one edge share its name, one Arc: all of them. Only when some name repeats.)
            if map.len() == self.edges.len() {
                return vec![i];
            }
            return (0..self.edges.len()).filter(|&j| j == i || Arc::ptr_eq(&self.edges[j], &self.edges[i])).collect();
        }
        // Fuzzy: an edge whose adjacent faces descend from the referenced
        // faces; else (faces split when the reference was made are whole
        // again) one whose faces are their ancestors.
        let pieces = self.edges_related(n, |cand, refd| cand.descends_from(refd));
        if !pieces.is_empty() {
            return pieces;
        }
        self.edges_related(n, |cand, refd| refd.descends_from(cand))
    }

    /// Edges whose adjacent faces relate (`rel(candidate face, referenced face)`)
    /// to those of the edge name `n`.
    fn edges_related(&self, n: &TopoName, rel: impl Fn(&TopoName, &TopoName) -> bool) -> Vec<usize> {
        match n {
            TopoName::Edge { faces: [a, b], .. } => self
                .edges
                .iter()
                .enumerate()
                .filter(|(_, e)| match &***e {
                    TopoName::Edge { faces: [c, d], .. } => (rel(c, a) && rel(d, b)) || (rel(c, b) && rel(d, a)),
                    _ => false,
                })
                .map(|(i, _)| i)
                .collect(),
            TopoName::Boundary { face, .. } => self
                .edges
                .iter()
                .enumerate()
                .filter(|(_, e)| matches!(&***e, TopoName::Boundary { face: f, .. } if rel(f, face)))
                .map(|(i, _)| i)
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn find_vertices(&self, n: &TopoName) -> Vec<usize> {
        let map = Self::lookup(&self.vertex_lookup, &self.vertices);
        if let Some(i) = map.get(n) {
            return vec![*i];
        }
        let TopoName::Vertex { faces, .. } = n else { return Vec::new() };
        let related = |rel: &dyn Fn(&TopoName, &TopoName) -> bool| -> Vec<usize> {
            self.vertices
                .iter()
                .enumerate()
                .filter(|(_, v)| match &***v {
                    TopoName::Vertex { faces: fs, .. } => fs.len() == faces.len() && faces.iter().all(|a| fs.iter().any(|b| rel(b, a))),
                    _ => false,
                })
                .map(|(i, _)| i)
                .collect()
        };
        // As for edges: split pieces first, else the faces they were split from.
        let pieces = related(&|cand, refd| cand.descends_from(refd));
        if !pieces.is_empty() {
            return pieces;
        }
        related(&|cand, refd| refd.descends_from(cand))
    }

    pub fn shape_names(&self) -> crate::naming::ShapeNames {
        crate::naming::ShapeNames { faces: self.faces.clone(), edges: self.edges.clone(), vertices: self.vertices.clone() }
    }
}

/// A solved sketch.
#[derive(Clone, Serialize, Deserialize)]
pub struct SketchResult {
    pub feature: FeatureId,
    pub name: String,
    pub frame: PlaneFrame,
    pub sketch: noshape_sketch::Sketch,
    pub report: noshape_sketch::SolveReport,
    pub regions: Vec<noshape_sketch::regions::Region>,
    /// Which `regions` are pieces of the sketch plane's face outside the sketch's own loops (imprinted: sketch faces
    /// that qSketchRegion leaves out, as live); empty when nothing is imprinted.
    pub imprinted: Vec<bool>,
    /// The sketch's own regions the imprinted face edges split: references made before imprinting still resolve.
    pub split: Vec<noshape_sketch::regions::Region>,
    pub wires: Vec<noshape_sketch::regions::Wire>,
    /// Dimension label texts by constraint id.
    pub dimension_text: BTreeMap<u32, String>,
    /// Inserted images (Insert image): drawn in their solved boxes.
    pub images: Vec<SketchResultImage>,
    /// Its regions' edges run their loops' way, not their curves' ([`Context::legacy_edge_directions`]).
    pub legacy_edge_directions: bool,
    /// The sketch's bodies this result holds (`None`: all). A FeatureScript operation on some of them (delete,
    /// transform, pattern) leaves pieces of one sketch, each holding its own.
    #[serde(default)]
    pub bodies: Option<SketchBodies>,
}

/// Bodies of a sketch (live: a wire body per curve, a point body per point that is no curve's end, one sheet body
/// for the regions): curves and points by entity id, and the sheet.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SketchBodies {
    pub entities: BTreeSet<u32>,
    pub sheet: bool,
}

impl SketchResult {
    /// The points that are only the ends of lines, arcs, elliptical arcs or conics: those curves' own end vertices
    /// stand for them, so they have no body of their own (live: a rectangle 4 wires, 8 vertices).
    pub fn curve_end_points(&self) -> HashSet<u32> {
        use noshape_sketch::Geometry as G;
        let (mut ends, mut kept) = (HashSet::new(), HashSet::new());
        for e in self.sketch.entities() {
            match &e.geometry {
                G::Line { start, end } | G::Arc { start, end, .. } | G::EllipticalArc { start, end, .. } | G::Conic { start, end, .. } => {
                    ends.extend([start.0, end.0]);
                }
                G::Point { .. } => {}
                _ => kept.extend(e.point_refs().iter().map(|p| p.0)),
            }
            kept.extend(e.center().map(|c| c.0));
            if let G::Conic { control, .. } = &e.geometry {
                kept.insert(control.0);
            }
        }
        ends.retain(|p| !kept.contains(p));
        ends
    }

    /// Every body of the sketch: its curves, its points with a body of their own, its sheet if it has regions.
    pub fn all_bodies(&self) -> SketchBodies {
        let ends = self.curve_end_points();
        let entities = self.sketch.entities().filter(|e| e.kind().is_curve() || (e.kind() == noshape_sketch::EntityKind::Point && !ends.contains(&e.id.0))).map(|e| e.id.0).collect();
        SketchBodies { entities, sheet: !self.regions.is_empty() }
    }

    /// The bodies this result holds.
    pub fn held(&self) -> SketchBodies {
        self.bodies.clone().unwrap_or_else(|| self.all_bodies())
    }

    /// Whether it holds an entity's body (a curve's end point goes with its curve).
    pub fn holds(&self, entity: u32) -> bool {
        let Some(b) = &self.bodies else { return self.sketch.entity(noshape_sketch::EntityId(entity)).is_some() };
        b.entities.contains(&entity) || self.sketch.entities().any(|e| b.entities.contains(&e.id.0) && e.endpoints().is_some_and(|(a, z)| a.0 == entity || z.0 == entity))
    }

    /// Whether it holds the regions' sheet.
    pub fn holds_sheet(&self) -> bool {
        self.bodies.as_ref().is_none_or(|b| b.sheet)
    }
}

/// An inserted sketch image: its box corner points and encoded file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SketchResultImage {
    pub id: u32,
    /// Box corner point entities: lower-left, lower-right, upper-right, upper-left.
    pub box_points: Vec<u32>,
    /// PNG / JPEG / GIF / BMP bytes, base64 encoded.
    pub data_base64: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaneEntity {
    pub reference: Reference,
    pub name: String,
    pub frame: PlaneFrame,
    pub half_width: f64,
    pub half_height: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connector {
    pub reference: Reference,
    pub name: String,
    pub origin: DVec3,
    pub x: DVec3,
    pub z: DVec3,
}

/// Point bodies and other datum points.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointEntity {
    pub reference: Reference,
    pub position: DVec3,
}

/// A merge scope an evaluation found: its tools, the bodies they touch, and whether exactly
/// ([`crate::regen::eval::touching`]).
pub type ScopeNote = (Vec<Shape>, Vec<BodyId>, bool);

/// What a feature's evaluation read of its input's bodies (incremental regeneration: a cached evaluation is reused
/// when the bodies it read are the very same and every name lookup comes out the same). Bodies by address; any
/// access to the whole list (iteration, a slice) reads them all.
#[derive(Default)]
pub struct BodyReads {
    pub all: std::sync::atomic::AtomicBool,
    pub bodies: parking_lot::Mutex<Vec<usize>>,
    /// Name lookups over all bodies (a reference resolved): replayed on the old and the new input.
    pub lookups: parking_lot::Mutex<Vec<(LookupKind, TopoName)>>,
    /// NOSHAPE_REUSE_TRACE: where the evaluation first read all the bodies (why it can't be reused).
    pub first_all: parking_lot::Mutex<Option<String>>,
    /// The bounds of all the bodies, when the evaluation asked for them (a datum plane's tool face sized to the
    /// model): compared with the new input's.
    pub bounds: parking_lot::Mutex<Option<Option<(DVec3, DVec3)>>>,
    /// The solids found touching tools (`eval::touching`): the tools and the ids found, found again on a new input.
    pub scopes: parking_lot::Mutex<Vec<ScopeNote>>,
}

/// NOSHAPE_REUSE_TRACE: note where evaluations read all the bodies.
pub fn trace_reads() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NOSHAPE_REUSE_TRACE").is_some())
}

/// What a name lookup looked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupKind {
    Face,
    Edge,
    Vertex,
}

impl BodyReads {
    fn read(&self, b: &Arc<Body>) {
        if self.all.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let p = Arc::as_ptr(b) as usize;
        let mut v = self.bodies.lock();
        if v.last() != Some(&p) {
            v.push(p);
        }
    }
    pub fn read_all(&self) {
        if !self.all.swap(true, std::sync::atomic::Ordering::Relaxed) && trace_reads() {
            *self.first_all.lock() = Some(std::backtrace::Backtrace::force_capture().to_string());
        }
    }
}

/// A context's bodies: a list, which can note what a feature reads of it ([`BodyReads`]). Single bodies by index
/// (`bodies[i]`, `get`) are noted one by one; anything else (`iter`, slices, `retain`) as reading them all.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Bodies {
    v: Vec<Arc<Body>>,
    #[serde(skip)]
    reads: Option<Arc<BodyReads>>,
    /// Other names faces go by (a merged face's other parents': [`Bodies::add_aliases`]), looked at only when a name
    /// finds nothing.
    #[serde(default)]
    aliases: Arc<Vec<(Name, Name)>>,
}

impl Bodies {
    pub fn new() -> Self {
        Self::default()
    }
    /// Start noting reads into `reads` (`None`: stop).
    pub fn track(&mut self, reads: Option<Arc<BodyReads>>) {
        self.reads = reads;
    }
    fn all(&self) {
        if let Some(r) = &self.reads {
            r.read_all();
        }
    }
    pub fn len(&self) -> usize {
        self.v.len()
    }
    pub fn is_empty(&self) -> bool {
        self.v.is_empty()
    }
    pub fn get(&self, i: usize) -> Option<&Arc<Body>> {
        let b = self.v.get(i)?;
        if let Some(r) = &self.reads {
            r.read(b);
        }
        Some(b)
    }
    /// Every body's sub-shapes named `n` (body index, sub-shape index): a name lookup, not a read of the bodies'
    /// geometry (noted as a lookup, replayed when an evaluation is reused).
    pub fn find(&self, kind: LookupKind, n: &TopoName) -> Vec<(usize, usize)> {
        if let Some(r) = &self.reads {
            r.lookups.lock().push((kind, n.clone()));
        }
        self.find_untracked(kind, n)
    }
    /// Notes a name lookup made some other way (replayed with [`Bodies::find_untracked`] when the evaluation is reused).
    pub fn note_lookup(&self, kind: LookupKind, n: &TopoName) {
        if let Some(r) = &self.reads {
            r.lookups.lock().push((kind, n.clone()));
        }
    }
    pub fn find_untracked(&self, kind: LookupKind, n: &TopoName) -> Vec<(usize, usize)> {
        if self.aliases.is_empty() {
            return self.find_named(kind, n);
        }
        // Not a name any face goes by now: under another name a merged face goes by (a reference made when an older
        // version named it by another of its parents: hat15's SSD pad sketches, made in v0.4.43), followed a few
        // merges on; that name exactly before the near matches of this one (a piece of another face of that parent
        // made whole again).
        let exact = |m: &TopoName| self.v.iter().any(|b| b.has_exact(m));
        let mut chain: Vec<TopoName> = Vec::new();
        if !exact(n) {
            let mut cur = n.clone();
            for _ in 0..8 {
                let Some(next) = cur.renamed(&|x| self.aliases.iter().find(|(a, _)| **a == *x).map(|(_, to)| to.clone())) else { break };
                if exact(&next) {
                    return self.find_named(kind, &next);
                }
                chain.push(next.clone());
                cur = next;
            }
        }
        let found = self.find_named(kind, n);
        if !found.is_empty() {
            return found;
        }
        chain.iter().map(|m| self.find_named(kind, m)).find(|f| !f.is_empty()).unwrap_or_default()
    }
    /// Records `pairs` (another name, the face's name) for [`Bodies::find_untracked`].
    pub fn add_aliases(&mut self, pairs: impl IntoIterator<Item = (Name, Name)>) {
        // (The first wins at lookup: no check for repeats, which would scan the list each time.)
        Arc::make_mut(&mut self.aliases).extend(pairs.into_iter().filter(|(from, to)| from != to));
    }
    /// Puts `v` in place of the bodies, as a new list would (no reads noted from here on), keeping the other names
    /// faces go by.
    pub fn set_all(&mut self, v: Vec<Arc<Body>>) {
        self.v = v;
        self.reads = None;
    }
    /// Takes the bodies out, keeping the other names faces go by.
    pub fn take_all(&mut self) -> Vec<Arc<Body>> {
        self.reads = None;
        std::mem::take(&mut self.v)
    }
    fn find_named(&self, kind: LookupKind, n: &TopoName) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        // (An open composite's entities are its members': features change the members, never the composite.)
        for (bi, b) in self.v.iter().enumerate().filter(|(_, b)| b.composite.as_ref().is_none_or(|c| c.closed)) {
            let found = match kind {
                LookupKind::Face => b.find_faces(n),
                LookupKind::Edge => b.find_edges(n),
                LookupKind::Vertex => b.find_vertices(n),
            };
            out.extend(found.into_iter().map(|i| (bi, i)));
        }
        out
    }
    /// Puts `b` in place of body `i` (the regenerator's own bookkeeping, not features).
    pub fn replace_untracked(&mut self, i: usize, b: Arc<Body>) {
        self.v[i] = b;
    }
    /// Index of the body with id `id` (ids are no read: an input whose body ids differ is never reused).
    pub fn position_id(&self, id: &BodyId) -> Option<usize> {
        self.v.iter().position(|b| &b.id == id)
    }
    /// The bodies without noting a read (the regenerator's own bookkeeping, not features).
    pub fn untracked(&self) -> &[Arc<Body>] {
        &self.v
    }
    pub fn push(&mut self, b: Arc<Body>) {
        self.v.push(b);
    }
    /// (Positions are no read: an input whose body ids differ is never reused.)
    pub fn insert(&mut self, i: usize, b: Arc<Body>) {
        self.v.insert(i, b);
    }
    /// Removes body `i`, a read of it.
    pub fn remove(&mut self, i: usize) -> Arc<Body> {
        if let Some(r) = &self.reads {
            r.read(&self.v[i]);
        }
        self.v.remove(i)
    }
    /// Removes the bodies with these ids, a read of each.
    pub fn remove_ids(&mut self, ids: &[BodyId]) {
        while let Some(i) = self.v.iter().position(|b| ids.contains(&b.id)) {
            self.remove(i);
        }
    }
    /// Notes the solids found touching `tools` (found again when the evaluation is reused).
    pub fn note_scope(&self, tools: Vec<Shape>, ids: Vec<BodyId>, exact: bool) {
        if let Some(r) = &self.reads {
            r.scopes.lock().push((tools, ids, exact));
        }
    }
    /// The bounds of all the bodies (noted, compared with a new input's when the evaluation is reused).
    pub fn bounds(&self) -> Option<(DVec3, DVec3)> {
        let b = self.bounds_untracked();
        if let Some(r) = &self.reads {
            *r.bounds.lock() = Some(b);
        }
        b
    }
    pub fn bounds_untracked(&self) -> Option<(DVec3, DVec3)> {
        let mut acc: Option<(DVec3, DVec3)> = None;
        for b in &self.v {
            if let Ok(bb) = b.shape.bounding_box(false) {
                acc = Some(match acc {
                    None => (bb.min, bb.max),
                    Some((a, c)) => (a.min(bb.min), c.max(bb.max)),
                });
            }
        }
        acc
    }
    pub fn retain(&mut self, f: impl FnMut(&Arc<Body>) -> bool) {
        self.all();
        self.v.retain(f);
    }
    pub fn extend(&mut self, it: impl IntoIterator<Item = Arc<Body>>) {
        self.v.extend(it);
    }
    pub fn append(&mut self, other: &mut Vec<Arc<Body>>) {
        self.v.append(other);
    }
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Arc<Body>> {
        self.all();
        self.v.iter_mut()
    }
    pub fn to_vec(&self) -> Vec<Arc<Body>> {
        self.all();
        self.v.clone()
    }
}

impl std::ops::Deref for Bodies {
    type Target = [Arc<Body>];
    fn deref(&self) -> &[Arc<Body>] {
        self.all();
        &self.v
    }
}

impl std::ops::DerefMut for Bodies {
    fn deref_mut(&mut self) -> &mut [Arc<Body>] {
        self.all();
        &mut self.v
    }
}

impl std::ops::Index<usize> for Bodies {
    type Output = Arc<Body>;
    fn index(&self, i: usize) -> &Arc<Body> {
        let b = &self.v[i];
        if let Some(r) = &self.reads {
            r.read(b);
        }
        b
    }
}

impl std::ops::IndexMut<usize> for Bodies {
    fn index_mut(&mut self, i: usize) -> &mut Arc<Body> {
        if let Some(r) = &self.reads {
            r.read(&self.v[i]);
        }
        &mut self.v[i]
    }
}

impl<'a> IntoIterator for &'a Bodies {
    type Item = &'a Arc<Body>;
    type IntoIter = std::slice::Iter<'a, Arc<Body>>;
    fn into_iter(self) -> Self::IntoIter {
        self.all();
        self.v.iter()
    }
}

impl IntoIterator for Bodies {
    type Item = Arc<Body>;
    type IntoIter = std::vec::IntoIter<Arc<Body>>;
    fn into_iter(self) -> Self::IntoIter {
        self.all();
        self.v.into_iter()
    }
}

impl FromIterator<Arc<Body>> for Bodies {
    fn from_iter<I: IntoIterator<Item = Arc<Body>>>(it: I) -> Self {
        Bodies { v: it.into_iter().collect(), reads: None, aliases: Arc::default() }
    }
}

impl From<Vec<Arc<Body>>> for Bodies {
    fn from(v: Vec<Arc<Body>>) -> Self {
        Bodies { v, reads: None, aliases: Arc::default() }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Context {
    /// The studio's planar faces keep OCCT's plane x ([`crate::partstudio::PartStudio::legacy_face_frames`]).
    pub legacy_face_frames: bool,
    /// The studio's tilted planar faces keep x = Z × n ([`crate::partstudio::PartStudio::legacy_tilted_face_frames`]).
    pub legacy_tilted_face_frames: bool,
    /// The studio's sketches on construction planes keep the planes' own frames
    /// ([`crate::partstudio::PartStudio::legacy_plane_sketch_frames`]).
    pub legacy_plane_sketch_frames: bool,
    /// The studio's sketch regions' edges run their loops' way ([`crate::partstudio::PartStudio::legacy_edge_directions`]).
    pub legacy_edge_directions: bool,
    /// The studio's part patterns' Add with a merge scope leaves the seeds apart
    /// ([`crate::partstudio::PartStudio::legacy_pattern_seeds`]).
    pub legacy_pattern_seeds: bool,
    pub bodies: Bodies,
    pub sketches: Vec<Arc<SketchResult>>,
    /// Sketches a Derived feature brought in from its source: (the Derived
    /// feature, the source sketch's feature id, which the copy keeps).
    pub derived_sketches: Vec<(FeatureId, FeatureId)>,
    /// Sketch copies a Feature pattern made (live L41): (the pattern, the
    /// copy's own id). The copies follow the pattern's visibility.
    pub sketch_copies: Vec<(FeatureId, FeatureId)>,
    /// Sketches FeatureScript brought in or copied for its own use, never
    /// shown (std Form's flat sketches: a form's "Form profile").
    pub hidden_sketches: Vec<FeatureId>,
    pub planes: Vec<PlaneEntity>,
    pub connectors: Vec<Connector>,
    /// Mate connectors' owner parts: a connector moves with its owner
    /// (mateConnector.fs "Owner entity"; a Transform of the part).
    pub connector_owners: Vec<(Reference, BodyId)>,
    pub points: Vec<PointEntity>,
    pub variables: BTreeMap<String, Quantity>,
    /// Query variables by name (Query variable feature), as persistent references.
    pub query_variables: BTreeMap<String, Vec<Reference>>,
    pub next_part: usize,
    pub next_surface: usize,
    pub next_curve: usize,
    pub next_color: usize,
    // --- batch SM: sheet metal models and frame members ---
    /// Sheet metal models by body (see `crate::sheetmetal`).
    pub sheet_metal: BTreeMap<BodyId, Arc<crate::sheetmetal::SheetMetalData>>,
    /// Flat patterns of sheet metal parts built by the standard library
    /// (custom/host/sheetmetal.rs), in live's placement.
    pub sheet_metal_flats: BTreeMap<BodyId, Arc<crate::sheetmetal::FlatPattern>>,
    /// Frame members (frame segments, gussets, end caps) by body, for cut lists.
    pub frame_members: BTreeMap<BodyId, Arc<super::features::frames::FrameMember>>,
    /// Form tools tagged by Tag features (purpose Form), for sheet metal Form.
    pub form_tools: Vec<crate::sheetmetal::FormTool>,
    // --- end batch SM ---
    /// Holes made by Hole features (drawings' hole / thread callouts).
    pub holes: Vec<HoleRecord>,
    /// Images Decal features put on faces (std decalUtils.fs DecalData).
    pub decals: Vec<DecalRecord>,
    /// Appearances features gave their parts (Import: the file's colours),
    /// linear RGBA; a part's own appearance (part properties) wins.
    pub part_appearance: BTreeMap<BodyId, [f32; 4]>,
    /// Appearances features gave single faces (Import), by body and face
    /// name: the body's faces descending from it keep it. Linear RGBA.
    pub face_appearance: Vec<(BodyId, crate::naming::Name, [f32; 4])>,
    /// FeatureScript attributes (`setAttribute`), kept from feature to
    /// feature (custom/host loads and saves them around each custom feature).
    #[serde(skip)]
    pub fs_attributes: Vec<(crate::reference::Reference, noshape_featurescript::Value)>,
    /// FeatureScript variables that aren't quantities (`setVariable(context, "t", <function>)`: cf-parametric-curve's
    /// Define Parameter), kept from feature to feature like the attributes.
    #[serde(skip)]
    pub fs_variables: BTreeMap<String, noshape_featurescript::Value>,
    /// Bodies FeatureScript keeps out of the Part Studio (std sheet metal's
    /// definition sheets): no part, no display; custom features see them
    /// through attribute queries only.
    pub fs_hidden_bodies: Vec<Arc<Body>>,
    /// Transient ids FeatureScript stored in attributes that must outlive the
    /// feature (std Form's tool bodies, kept in the walls' attributes).
    pub fs_transients: Vec<(Arc<str>, crate::reference::Reference)>,
    /// How many transient ids FeatureScript has handed out so far in this regeneration: the next feature's go on
    /// from there, so ids (and std's attribute ids made from them) never repeat across features, as Onshape's.
    pub fs_transient_next: usize,
    /// Sheet metal definition joints in the order they appeared (the std
    /// rebuild's: an edge by its end points and its faces' boxes, a rolled
    /// face by its box, in 0.1 µm): an entity a later feature changes, or an
    /// edge beside a face it cuts down, is new, as Onshape gives it a new
    /// transient id. The table letters follow it (live request 1604).
    pub sm_births: Vec<Vec<i64>>,
    /// Parts named from outside the studio (an imported file's names, a
    /// Derived source's): `renumber_default_names` leaves them as they are
    /// (live, qa5 NAME3).
    pub own_names: std::collections::BTreeSet<BodyId>,
}

/// A Decal's image on a face (std decalUtils.fs `DecalData`, the "decals"
/// attribute): a face point maps to image coordinates by `mapping` (meters
/// in the plane, or arc length and height on the cylinder), then by the UV
/// transform; the image covers UV [0, 1]².
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecalRecord {
    pub feature: FeatureId,
    pub body: BodyId,
    pub face: crate::naming::Name,
    /// The image's file tab.
    pub image: String,
    /// Its encoded bytes (PNG / JPEG …), for display.
    pub image_bytes: Option<std::sync::Arc<Vec<u8>>>,
    pub mapping: DecalMapping,
    /// TransformUV: uv = translation + linear · local.
    pub uv_linear: [[f64; 2]; 2],
    pub uv_translation: [f64; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DecalMapping {
    /// Local coordinates along `x` and `y` from `origin`.
    Planar { origin: DVec3, x: DVec3, y: DVec3, z: DVec3 },
    /// Arc length round `z` from `x` (radius · angle) and height along `z`.
    Cylindrical { origin: DVec3, x: DVec3, y: DVec3, z: DVec3, radius: f64 },
}

impl DecalRecord {
    /// Image coordinates of a point on the face (std getDecalUvSpacePosition).
    pub fn uv_at(&self, p: DVec3) -> [f64; 2] {
        let local = match &self.mapping {
            DecalMapping::Planar { origin, x, y, .. } => [(p - *origin).dot(*x), (p - *origin).dot(*y)],
            DecalMapping::Cylindrical { origin, x, y, z, radius } => {
                let d = p - *origin;
                let r = d.normalize_or_zero();
                let mut theta = r.dot(*y).atan2(r.dot(*x));
                if self.uv_translation[0] <= 0.0 && theta < 0.0 {
                    theta += std::f64::consts::TAU;
                } else if self.uv_translation[0] >= 1.0 && theta > 0.0 {
                    theta -= std::f64::consts::TAU;
                }
                [theta * radius, d.dot(*z)]
            }
        };
        let l = &self.uv_linear;
        [self.uv_translation[0] + l[0][0] * local[0] + l[0][1] * local[1], self.uv_translation[1] + l[1][0] * local[0] + l[1][1] * local[1]]
    }
}

/// One hole of a Hole feature: where it starts, its direction into the
/// material and its sizes (meters, radians).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HoleRecord {
    pub feature: FeatureId,
    pub start: DVec3,
    pub dir: DVec3,
    pub diameter: f64,
    /// Blind depth (None: through all / up to).
    pub depth: Option<f64>,
    pub through: bool,
    pub tap_drill: Option<f64>,
    pub tapped_depth: Option<f64>,
    /// Thread designation ("M6x1.0", "#10-24") and thread class ("6H").
    pub thread: Option<String>,
    pub thread_class: Option<String>,
    /// Counterbore (diameter, depth), countersink (diameter, angle).
    pub cbore: Option<(f64, f64)>,
    pub csink: Option<(f64, f64)>,
    /// Inch hole standard (callout values in inches).
    pub inch: bool,
    /// Fit tolerance of the diameter (callout "⌀6.350 H7 +0.015/−0.000").
    pub diameter_fit: Option<crate::fits::HoleFit>,
    /// The hole runs out of the last part it cuts (the Part Studio callout
    /// then reads THRU, live dialogs/hole-30).
    pub exits: bool,
    /// The parts it cuts (the Hole table lists the hole under each).
    pub parts: Vec<BodyId>,
    /// Drill tip angle (None: flat).
    pub tip_angle: Option<f64>,
}

impl HoleRecord {
    /// Is a circle of `radius` centered at `center` part of this hole
    /// (on its axis, with one of its diameters)?
    pub fn matches_circle(&self, center: DVec3, radius: f64, tol: f64) -> bool {
        let v = center - self.start;
        let off = (v - self.dir * v.dot(self.dir)).length();
        let d = radius * 2.0;
        let sizes = [Some(self.diameter), self.tap_drill, self.cbore.map(|c| c.0), self.csink.map(|c| c.0)];
        off < tol && sizes.iter().flatten().any(|s| (s - d).abs() < tol.max(1e-7))
    }
}

impl Context {
    /// The x of a sketch or evPlane on a planar face of plane `pl`, by the rules the studio was saved under.
    pub fn planar_face_x(&self, pl: &noshape_kernel::Plane) -> DVec3 {
        let n = pl.unit_normal();
        if self.legacy_face_frames {
            pl.unit_x()
        } else if self.legacy_tilted_face_frames {
            PlaneFrame::face_x_1897(n)
        } else {
            PlaneFrame::face_x(n)
        }
    }

    /// Everything but the bodies the same as `o` (an evaluation's other inputs, for reusing it): shared parts by
    /// identity, values by equality; FeatureScript attributes only when both have none. Every field is named here, so
    /// a new one has to say how it compares.
    pub fn same_but_bodies(&self, o: &Context) -> bool {
        let Context {
            legacy_face_frames,
            legacy_tilted_face_frames,
            legacy_plane_sketch_frames,
            legacy_edge_directions,
            legacy_pattern_seeds,
            bodies: _,
            sketches,
            derived_sketches,
            sketch_copies,
            hidden_sketches,
            planes,
            connectors,
            connector_owners,
            points,
            variables,
            query_variables,
            next_part,
            next_surface,
            next_curve,
            next_color,
            sheet_metal,
            sheet_metal_flats,
            frame_members,
            form_tools,
            holes,
            decals,
            part_appearance,
            face_appearance,
            fs_attributes,
            fs_variables,
            fs_hidden_bodies,
            fs_transients,
            fs_transient_next,
            sm_births,
            own_names,
        } = self;
        fn same_arcs<T>(a: &[Arc<T>], b: &[Arc<T>]) -> bool {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| Arc::ptr_eq(x, y))
        }
        fn same_map<K: PartialEq, V>(a: &BTreeMap<K, Arc<V>>, b: &BTreeMap<K, Arc<V>>) -> bool {
            a.len() == b.len() && a.iter().zip(b).all(|((k, v), (l, w))| k == l && Arc::ptr_eq(v, w))
        }
        *legacy_face_frames == o.legacy_face_frames
            && *legacy_tilted_face_frames == o.legacy_tilted_face_frames
            && *legacy_plane_sketch_frames == o.legacy_plane_sketch_frames
            && *legacy_edge_directions == o.legacy_edge_directions
            && *legacy_pattern_seeds == o.legacy_pattern_seeds
            && same_arcs(sketches, &o.sketches)
            && *derived_sketches == o.derived_sketches
            && *sketch_copies == o.sketch_copies
            && *hidden_sketches == o.hidden_sketches
            && *planes == o.planes
            && *connectors == o.connectors
            && *connector_owners == o.connector_owners
            && *points == o.points
            && *variables == o.variables
            && *query_variables == o.query_variables
            && (*next_part, *next_surface, *next_curve, *next_color) == (o.next_part, o.next_surface, o.next_curve, o.next_color)
            && same_map(sheet_metal, &o.sheet_metal)
            && same_map(sheet_metal_flats, &o.sheet_metal_flats)
            && same_map(frame_members, &o.frame_members)
            && form_tools.is_empty()
            && o.form_tools.is_empty()
            && *holes == o.holes
            && *decals == o.decals
            && *part_appearance == o.part_appearance
            && *face_appearance == o.face_appearance
            && fs_attributes.is_empty()
            && o.fs_attributes.is_empty()
            && fs_variables.is_empty()
            && o.fs_variables.is_empty()
            && same_arcs(fs_hidden_bodies, &o.fs_hidden_bodies)
            && *fs_transients == o.fs_transients
            && *fs_transient_next == o.fs_transient_next
            && *sm_births == o.sm_births
            && *own_names == o.own_names
    }

    pub fn body(&self, id: &BodyId) -> Option<&Arc<Body>> {
        self.bodies.iter().find(|b| &b.id == id)
    }

    /// A sketch's result: of its pieces, the one holding its regions first.
    pub fn sketch(&self, f: FeatureId) -> Option<&Arc<SketchResult>> {
        self.sketches.iter().find(|s| s.feature == f && s.holds_sheet()).or_else(|| self.sketches.iter().find(|s| s.feature == f))
    }

    /// The piece of a sketch holding an entity's body.
    pub fn sketch_holding(&self, f: FeatureId, entity: u32) -> Option<&Arc<SketchResult>> {
        self.sketches.iter().find(|s| s.feature == f && s.holds(entity))
    }

    /// The sketch bodies references stand for, by piece (`sketches` index): a whole sketch all its pieces' bodies, a
    /// sketch body itself, a curve or its end its wire, a point its point body (a curve's end its curve's wire), a
    /// region or its edges the sheet.
    pub fn sketch_bodies_of(&self, refs: &[Reference]) -> Vec<(usize, SketchBodies)> {
        let mut out: Vec<(usize, SketchBodies)> = Vec::new();
        let mut add = |i: usize, entity: Option<u32>| {
            let at = match out.iter().position(|(j, _)| *j == i) {
                Some(at) => at,
                None => {
                    out.push((i, SketchBodies::default()));
                    out.len() - 1
                }
            };
            match entity {
                Some(e) => {
                    out[at].1.entities.insert(e);
                }
                None => out[at].1.sheet = true,
            }
        };
        let piece = |f: FeatureId, e: u32| self.sketches.iter().position(|s| s.feature == f && s.holds(e));
        let sheet = |f: FeatureId| self.sketches.iter().position(|s| s.feature == f && s.holds_sheet() && !s.regions.is_empty());
        for r in refs {
            match r {
                Reference::Sketch(f) => {
                    for (i, s) in self.sketches.iter().enumerate().filter(|(_, s)| s.feature == *f) {
                        let held = s.held();
                        for e in held.entities {
                            add(i, Some(e));
                        }
                        if held.sheet {
                            add(i, None);
                        }
                    }
                }
                Reference::SketchBody { sketch, entity: None } | Reference::SketchRegion { sketch, .. } | Reference::SketchRegionEdge { sketch, .. } => {
                    if let Some(i) = sheet(*sketch) {
                        add(i, None);
                    }
                }
                Reference::SketchBody { sketch, entity: Some(e) } | Reference::SketchEntity { sketch, entity: e } | Reference::SketchCurveEnd { sketch, entity: e, .. } => {
                    let Some(i) = piece(*sketch, *e) else { continue };
                    let s = &self.sketches[i];
                    // A point that is only curves' ends: the first of those curves' wire.
                    let body = match s.curve_end_points().contains(e) {
                        true => s.sketch.entities().find(|c| s.holds(c.id.0) && c.endpoints().is_some_and(|(a, b)| a.0 == *e || b.0 == *e)).map(|c| c.id.0),
                        false => Some(*e),
                    };
                    if let Some(b) = body {
                        add(i, Some(b));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Splits `take` of piece `i`'s bodies off into a piece of their own (the same sketch, at the end of `sketches`);
    /// `i` itself when it holds no others. The piece holding them.
    pub fn split_sketch(&mut self, i: usize, take: &SketchBodies) -> usize {
        let held = self.sketches[i].held();
        let taken = SketchBodies { entities: held.entities.intersection(&take.entities).copied().collect(), sheet: held.sheet && take.sheet };
        let rest = SketchBodies { entities: held.entities.difference(&take.entities).copied().collect(), sheet: held.sheet && !take.sheet };
        if rest.entities.is_empty() && !rest.sheet {
            return i;
        }
        let mut kept = (*self.sketches[i]).clone();
        kept.bodies = Some(rest);
        let mut piece = (*self.sketches[i]).clone();
        piece.bodies = Some(taken);
        self.sketches[i] = Arc::new(kept);
        self.sketches.push(Arc::new(piece));
        self.sketches.len() - 1
    }

    /// Deletes sketch bodies (live opDeleteBodies on some of a sketch's: those only); a piece left with none goes.
    pub fn delete_sketch_bodies(&mut self, bodies: &[(usize, SketchBodies)]) {
        let mut gone = Vec::new();
        for (i, take) in bodies {
            let held = self.sketches[*i].held();
            let rest = SketchBodies { entities: held.entities.difference(&take.entities).copied().collect(), sheet: held.sheet && !take.sheet };
            if rest.entities.is_empty() && !rest.sheet {
                gone.push(*i);
                continue;
            }
            let mut kept = (*self.sketches[*i]).clone();
            kept.bodies = Some(rest);
            self.sketches[*i] = Arc::new(kept);
        }
        let mut i = 0;
        self.sketches.retain(|_| {
            i += 1;
            !gone.contains(&(i - 1))
        });
    }

    /// Onshape numbers default names by list order ("Part 1", "Part 2", …):
    /// when a part goes, the ones after it move up (live "qa5 booleans": the
    /// old Part 3 becomes "Part 2"). Other names (renamed, derived, imported)
    /// are left alone; new parts continue the numbering.
    pub fn renumber_default_names(&mut self) {
        let mut n = [0usize; 3];
        for b in self.bodies.iter_mut() {
            let (prefix, k) = match b.kind {
                BodyKind::Solid => ("Part ", 0),
                BodyKind::Surface => ("Surface ", 1),
                BodyKind::Wire | BodyKind::Point => ("Curve ", 2),
            };
            // Any kind's default name: a part turned into a sheet (Delete
            // face › Leave open) becomes "Surface n" (live request 1547, qa
            // BODY-NAME-BY-TYPE).
            let numbered = |p: &str| b.name.strip_prefix(p).is_some_and(|r| !r.is_empty() && r.bytes().all(|c| c.is_ascii_digit()));
            let default = b.composite.is_none() && !self.own_names.contains(&b.id) && ["Part ", "Surface ", "Curve "].into_iter().any(numbered);
            if !default {
                continue;
            }
            n[k] += 1;
            let want = format!("{prefix}{}", n[k]);
            if b.name != want {
                *b = Arc::new(b.renamed(want));
            }
        }
        (self.next_part, self.next_surface, self.next_curve) = (n[0], n[1], n[2]);
    }

    pub fn alloc_part_name(&mut self, kind: BodyKind) -> String {
        match kind {
            BodyKind::Solid => {
                self.next_part += 1;
                format!("Part {}", self.next_part)
            }
            BodyKind::Surface => {
                self.next_surface += 1;
                format!("Surface {}", self.next_surface)
            }
            BodyKind::Wire | BodyKind::Point => {
                self.next_curve += 1;
                format!("Curve {}", self.next_curve)
            }
        }
    }

    pub fn alloc_color(&mut self) -> usize {
        let c = self.next_color;
        self.next_color += 1;
        c
    }

    /// World bounds of all bodies.
    pub fn bounds(&self) -> Option<(DVec3, DVec3)> {
        self.bodies.bounds()
    }
}

/// A body's names as stored: the pieces shown as one face share one Arc ([`join_face_names`], found by `Arc::ptr_eq`), so
/// each name is written once and a piece sharing an earlier one's as its index.
mod shared_names {
    use super::{Name, TopoName};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::sync::Arc;

    #[derive(Serialize, Deserialize)]
    enum Entry<T> {
        Name(T),
        Same(u32),
    }

    pub fn serialize<S: Serializer>(names: &[Name], s: S) -> Result<S::Ok, S::Error> {
        let mut first = std::collections::HashMap::with_capacity(names.len());
        s.collect_seq(names.iter().enumerate().map(|(i, n)| match *first.entry(Arc::as_ptr(n)).or_insert(i as u32) {
            j if j as usize == i => Entry::Name(&**n),
            j => Entry::Same(j),
        }))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Name>, D::Error> {
        let entries = Vec::<Entry<TopoName>>::deserialize(d)?;
        let mut out: Vec<Name> = Vec::with_capacity(entries.len());
        for e in entries {
            let n = match e {
                Entry::Name(n) => Arc::new(n),
                Entry::Same(j) => out.get(j as usize).cloned().ok_or_else(|| serde::de::Error::custom("a shared name before its first"))?,
            };
            out.push(n);
        }
        Ok(out)
    }
}

/// Faces shown as one face ([`noshape_kernel::ShownTopology::faces_joined`]: pieces of one periodic surface OCCT split
/// only at its seam, a cut's helical strip round a cylinder) take one name, one Arc: the least of theirs, so it doesn't
/// depend on the pieces' order. Not in a studio saved before (its references name the pieces). True when any changed.
pub fn join_face_names(shape: &Shape, faces: &mut [Name]) -> bool {
    if noshape_kernel::legacy_topology() || faces.len() < 2 {
        return false;
    }
    let Ok(t) = shape.shown_topology() else { return false };
    if t.faces_joined.len() != faces.len() || t.faces_joined.iter().enumerate().all(|(i, &r)| r == i) {
        return false;
    }
    let mut changed = false;
    for root in 0..faces.len() {
        let group: Vec<usize> = (0..faces.len()).filter(|&i| t.faces_joined[i] == root).collect();
        if group.len() < 2 {
            continue;
        }
        let Some(least) = group.iter().map(|&i| faces[i].clone()).min() else { continue };
        for &i in &group {
            if !Arc::ptr_eq(&faces[i], &least) {
                faces[i] = least.clone();
                changed = true;
            }
        }
    }
    changed
}

/// Per edge, the first of the pieces shown as one edge with it ([`noshape_kernel::ShownTopology::joined`]): they share
/// its name. Not in a studio saved before (its references name the pieces).
pub fn joined_pieces(shape: &Shape) -> Vec<usize> {
    if noshape_kernel::legacy_topology() {
        return Vec::new();
    }
    shape.shown_topology().map(|t| t.joined).unwrap_or_default()
}
