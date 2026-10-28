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
    /// Set on Composite part bodies.
    pub composite: Option<crate::display::CompositeInfo>,
    face_lookup: OnceLock<HashMap<Name, usize>>,
    edge_lookup: OnceLock<HashMap<Name, usize>>,
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
            (name_edges(&faces, &ef, &|i| edge_key(&shape, i)), name_vertices(&faces, &vf, &|i| vertex_key(&shape, i)))
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
        if let Some(i) = map.get(n) {
            return vec![*i];
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
        if let Some(i) = map.get(n) {
            return vec![*i];
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
#[derive(Clone)]
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
    /// Inserted images (Insert image): drawn in their solved boxes.
    pub images: Vec<SketchResultImage>,
}

/// An inserted sketch image: its box corner points and encoded file.
#[derive(Clone, Debug)]
pub struct SketchResultImage {
    pub id: u32,
    /// Box corner point entities: lower-left, lower-right, upper-right, upper-left.
    pub box_points: Vec<u32>,
    /// PNG / JPEG / GIF / BMP bytes, base64 encoded.
    pub data_base64: Arc<str>,
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
    /// Sketches a Derived feature brought in from its source: (the Derived
    /// feature, the source sketch's feature id, which the copy keeps).
    pub derived_sketches: Vec<(FeatureId, FeatureId)>,
    /// Sketch copies a Feature pattern made (live L41): (the pattern, the
    /// copy's own id). The copies follow the pattern's visibility.
    pub sketch_copies: Vec<(FeatureId, FeatureId)>,
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
    pub fs_attributes: Vec<(crate::reference::Reference, noshape_featurescript::Value)>,
    /// Bodies FeatureScript keeps out of the Part Studio (std sheet metal's
    /// definition sheets): no part, no display; custom features see them
    /// through attribute queries only.
    pub fs_hidden_bodies: Vec<Arc<Body>>,
    /// Transient ids FeatureScript stored in attributes that must outlive the
    /// feature (std Form's tool bodies, kept in the walls' attributes).
    pub fs_transients: Vec<(Arc<str>, crate::reference::Reference)>,
    /// Parts named from outside the studio (an imported file's names, a
    /// Derived source's): `renumber_default_names` leaves them as they are
    /// (live, qa5 NAME3).
    pub own_names: std::collections::BTreeSet<BodyId>,
}

/// A Decal's image on a face (std decalUtils.fs `DecalData`, the "decals"
/// attribute): a face point maps to image coordinates by `mapping` (meters
/// in the plane, or arc length and height on the cylinder), then by the UV
/// transform; the image covers UV [0, 1]².
#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
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
    pub fn body(&self, id: &BodyId) -> Option<&Arc<Body>> {
        self.bodies.iter().find(|b| &b.id == id)
    }

    pub fn sketch(&self, f: FeatureId) -> Option<&Arc<SketchResult>> {
        self.sketches.iter().find(|s| s.feature == f)
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
            let default = b.composite.is_none() && !self.own_names.contains(&b.id) && b.name.strip_prefix(prefix).is_some_and(|r| !r.is_empty() && r.bytes().all(|c| c.is_ascii_digit()));
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
