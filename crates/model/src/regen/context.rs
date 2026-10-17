//! Regeneration context: the state of a Part Studio after a feature.

use crate::display::{BodyKind, PlaneFrame};
use crate::naming::{GeoKey, Name, name_edges, name_vertices};
use crate::reference::{BodyId, FeatureId, Reference, TopoName};
use glam::DVec3;
use noshape_kernel::{Shape, ShapeType};
use noshape_units::Quantity;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock};

/// A part, surface or wire body with persistent names for its sub-shapes.
pub struct Body {
    pub id: BodyId,
    pub name: String,
    pub kind: BodyKind,
    pub shape: Shape,
    pub faces: Vec<Name>,
    pub edges: Vec<Name>,
    pub vertices: Vec<Name>,
    /// Index into the part color palette.
    pub color: usize,
    face_lookup: OnceLock<HashMap<Name, usize>>,
    edge_lookup: OnceLock<HashMap<Name, usize>>,
    vertex_lookup: OnceLock<HashMap<Name, usize>>,
}

impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Body").field("id", &self.id).field("name", &self.name).field("faces", &self.faces.len()).finish()
    }
}

/// Geometric ordering keys for faces (centroid + area).
pub fn face_keys(shape: &Shape) -> Vec<GeoKey> {
    shape
        .faces()
        .iter()
        .map(|f| {
            let c = f.centroid().unwrap_or(DVec3::ZERO);
            let a = f.area().unwrap_or(0.0);
            [c.x, c.y, c.z, a]
        })
        .collect()
}

pub fn edge_keys(shape: &Shape) -> Vec<GeoKey> {
    shape
        .edges()
        .iter()
        .map(|e| match e.edge_info() {
            Ok(i) => {
                let m = e.point_at((i.first_param + i.last_param) * 0.5).unwrap_or((i.start + i.end) * 0.5);
                [m.x, m.y, m.z, i.length]
            }
            Err(_) => [0.0; 4],
        })
        .collect()
}

pub fn vertex_keys(shape: &Shape) -> Vec<GeoKey> {
    shape.vertices().iter().map(|v| v.vertex_point().map(|p| [p.x, p.y, p.z, 0.0]).unwrap_or([0.0; 4])).collect()
}

impl Body {
    /// Build a body from its shape and face names; edge and vertex names are
    /// derived from face adjacency.
    pub fn new(id: BodyId, name: String, kind: BodyKind, shape: Shape, faces: Vec<Name>, color: usize, wire_feature: FeatureId) -> Self {
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
            (name_edges(&faces, &ef, &edge_keys(&shape)), name_vertices(&faces, &vf, &vertex_keys(&shape)))
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
            face_lookup: OnceLock::new(),
            edge_lookup: OnceLock::new(),
            vertex_lookup: OnceLock::new(),
        }
    }

    fn lookup<'a>(cell: &'a OnceLock<HashMap<Name, usize>>, names: &[Name]) -> &'a HashMap<Name, usize> {
        cell.get_or_init(|| names.iter().enumerate().map(|(i, n)| (n.clone(), i)).collect())
    }

    /// Indices of faces matching a name: exact, else all split descendants.
    pub fn find_faces(&self, n: &TopoName) -> Vec<usize> {
        let map = Self::lookup(&self.face_lookup, &self.faces);
        if let Some(i) = map.get(n) {
            return vec![*i];
        }
        self.faces.iter().enumerate().filter(|(_, f)| f.descends_from(n)).map(|(i, _)| i).collect()
    }

    pub fn find_edges(&self, n: &TopoName) -> Vec<usize> {
        let map = Self::lookup(&self.edge_lookup, &self.edges);
        if let Some(i) = map.get(n) {
            return vec![*i];
        }
        // Fuzzy: an edge whose adjacent faces descend from the referenced faces.
        match n {
            TopoName::Edge { faces: [a, b], .. } => self
                .edges
                .iter()
                .enumerate()
                .filter(|(_, e)| match &***e {
                    TopoName::Edge { faces: [c, d], .. } => (c.descends_from(a) && d.descends_from(b)) || (c.descends_from(b) && d.descends_from(a)),
                    _ => false,
                })
                .map(|(i, _)| i)
                .collect(),
            TopoName::Boundary { face, .. } => self
                .edges
                .iter()
                .enumerate()
                .filter(|(_, e)| matches!(&***e, TopoName::Boundary { face: f, .. } if f.descends_from(face)))
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
        match n {
            TopoName::Vertex { faces, .. } => self
                .vertices
                .iter()
                .enumerate()
                .filter(|(_, v)| match &***v {
                    TopoName::Vertex { faces: fs, .. } => fs.len() == faces.len() && faces.iter().all(|a| fs.iter().any(|b| b.descends_from(a))),
                    _ => false,
                })
                .map(|(i, _)| i)
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn shape_names(&self) -> crate::naming::ShapeNames {
        crate::naming::ShapeNames { faces: self.faces.clone(), edges: self.edges.clone(), vertices: self.vertices.clone() }
    }
}

/// A solved sketch.
pub struct SketchResult {
    pub feature: FeatureId,
    pub name: String,
    pub frame: PlaneFrame,
    pub sketch: noshape_sketch::Sketch,
    pub report: noshape_sketch::SolveReport,
    pub regions: Vec<noshape_sketch::regions::Region>,
    pub wires: Vec<noshape_sketch::regions::Wire>,
    /// Dimension label texts by constraint id.
    pub dimension_text: BTreeMap<u32, String>,
}

#[derive(Clone, Debug)]
pub struct PlaneEntity {
    pub reference: Reference,
    pub name: String,
    pub frame: PlaneFrame,
    pub half_width: f64,
    pub half_height: f64,
}

#[derive(Clone, Debug)]
pub struct Connector {
    pub reference: Reference,
    pub name: String,
    pub origin: DVec3,
    pub x: DVec3,
    pub z: DVec3,
}

/// Point bodies and other datum points.
#[derive(Clone, Debug)]
pub struct PointEntity {
    pub reference: Reference,
    pub position: DVec3,
}

#[derive(Clone, Default)]
pub struct Context {
    pub bodies: Vec<Arc<Body>>,
    pub sketches: Vec<Arc<SketchResult>>,
    pub planes: Vec<PlaneEntity>,
    pub connectors: Vec<Connector>,
    pub points: Vec<PointEntity>,
    pub variables: BTreeMap<String, Quantity>,
    pub next_part: usize,
    pub next_surface: usize,
    pub next_curve: usize,
    pub next_color: usize,
}

impl Context {
    pub fn body(&self, id: &BodyId) -> Option<&Arc<Body>> {
        self.bodies.iter().find(|b| &b.id == id)
    }

    pub fn sketch(&self, f: FeatureId) -> Option<&Arc<SketchResult>> {
        self.sketches.iter().find(|s| s.feature == f)
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
        let mut acc: Option<(DVec3, DVec3)> = None;
        for b in &self.bodies {
            if let Ok(bb) = b.shape.bounding_box(false) {
                acc = Some(match acc {
                    None => (bb.min, bb.max),
                    Some((a, c)) => (a.min(bb.min), c.max(bb.max)),
                });
            }
        }
        acc
    }
}
