//! Kernel-free display model of a regenerated Part Studio: everything the UI
//! needs to draw, pick and select, with persistent [`Reference`]s attached to
//! every pickable element. Produced by regeneration; consumed by the app.

use crate::reference::{BodyId, FeatureId, Reference};
use glam::DVec3;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    Solid,
    Surface,
    Wire,
    Point,
}

/// Triangle mesh of a body; faces occupy contiguous index ranges.
#[derive(Clone, Debug, Default)]
pub struct MeshBuffers {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// Per face: (first index, index count) into `indices`.
    pub face_ranges: Vec<(u32, u32)>,
    /// Per edge: polyline.
    pub edges: Vec<Vec<[f32; 3]>>,
    pub vertices: Vec<[f32; 3]>,
    /// Mesh faces' triangle sides as segments (pairs of points): Onshape
    /// draws every triangle edge of a mesh part as a thin dark-grey line, at
    /// every zoom (live G35).
    pub mesh_lines: Vec<[f32; 3]>,
}

impl MeshBuffers {
    /// Fill [`Self::mesh_lines`] from the mesh faces of `shape` (the shape
    /// this mesh tessellates; its faces in `face_ranges` order).
    pub fn with_mesh_lines(mut self, shape: &noshape_kernel::Shape) -> Self {
        use noshape_kernel::ShapeType;
        self.mesh_lines.clear();
        if !shape.is_mesh() {
            return self;
        }
        let q = |p: [f32; 3]| p.map(|v| (v as f64 * 1e7).round() as i64);
        let mut seen = std::collections::HashSet::new();
        for (fi, &(first, count)) in self.face_ranges.iter().enumerate() {
            if !shape.sub_shape(ShapeType::Face, fi).is_ok_and(|f| f.is_mesh()) {
                continue;
            }
            let Some(tris) = self.indices.get(first as usize..(first + count) as usize) else { continue };
            for t in tris.as_chunks::<3>().0 {
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    let (Some(&pa), Some(&pb)) = (self.positions.get(a as usize), self.positions.get(b as usize)) else { continue };
                    let (ka, kb) = (q(pa), q(pb));
                    if seen.insert(if ka <= kb { (ka, kb) } else { (kb, ka) }) {
                        self.mesh_lines.push(pa);
                        self.mesh_lines.push(pb);
                    }
                }
            }
        }
        self
    }

    /// Per edge: whether its two faces meet tangentially (Onshape's
    /// "tangent edges", e.g. a fillet's boundary). Found from the mesh: a
    /// triangle side lying on the edge in two faces whose normals agree.
    pub fn smooth_edges(&self) -> Vec<bool> {
        use std::collections::HashMap;
        let q = |p: [f32; 3]| [(p[0] as f64 * 1e6).round() as i64, (p[1] as f64 * 1e6).round() as i64, (p[2] as f64 * 1e6).round() as i64];
        let key = |a: [i64; 3], b: [i64; 3]| if a <= b { (a, b) } else { (b, a) };
        // Triangle sides: (face, normal at the side's midpoint).
        type Side = ([i64; 3], [i64; 3]);
        let mut sides: HashMap<Side, Vec<(usize, glam::Vec3)>> = HashMap::new();
        for (f, &(first, count)) in self.face_ranges.iter().enumerate() {
            let tris = self.indices.get(first as usize..(first + count) as usize).unwrap_or(&[]);
            for t in tris.as_chunks::<3>().0 {
                for (i, j) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    let (Some(pa), Some(pb), Some(na), Some(nb)) = (self.positions.get(i as usize), self.positions.get(j as usize), self.normals.get(i as usize), self.normals.get(j as usize)) else { continue };
                    let n = (glam::Vec3::from(*na) + glam::Vec3::from(*nb)).normalize_or_zero();
                    let e = sides.entry(key(q(*pa), q(*pb))).or_default();
                    if !e.iter().any(|(g, _)| *g == f) {
                        e.push((f, n));
                    }
                }
            }
        }
        self.edges
            .iter()
            .map(|pl| {
                let mut seen = false;
                for w in pl.windows(2) {
                    if let Some(fs) = sides.get(&key(q(w[0]), q(w[1])))
                        && fs.len() == 2
                    {
                        seen = true;
                        if fs[0].1.dot(fs[1].1) < 0.999 {
                            return false;
                        }
                    }
                }
                seen
            })
            .collect()
    }

    /// Edges bounding face `face` (a triangle side of the face lies on them).
    pub fn face_edges(&self, face: usize) -> Vec<usize> {
        use std::collections::HashSet;
        let q = |p: [f32; 3]| [(p[0] as f64 * 1e6).round() as i64, (p[1] as f64 * 1e6).round() as i64, (p[2] as f64 * 1e6).round() as i64];
        let key = |a: [i64; 3], b: [i64; 3]| if a <= b { (a, b) } else { (b, a) };
        let Some(&(first, count)) = self.face_ranges.get(face) else { return Vec::new() };
        let tris = self.indices.get(first as usize..(first + count) as usize).unwrap_or(&[]);
        let mut sides = HashSet::new();
        for t in tris.as_chunks::<3>().0 {
            for (i, j) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                if let (Some(a), Some(b)) = (self.positions.get(i as usize), self.positions.get(j as usize)) {
                    sides.insert(key(q(*a), q(*b)));
                }
            }
        }
        self.edges.iter().enumerate().filter(|(_, pl)| pl.windows(2).any(|w| sides.contains(&key(q(w[0]), q(w[1]))))).map(|(i, _)| i).collect()
    }
}

#[derive(Clone, Debug)]
pub struct BodyDisplay {
    pub id: BodyId,
    pub name: String,
    pub kind: BodyKind,
    /// Linear RGBA.
    pub color: [f32; 4],
    pub hidden: bool,
    pub mesh: Arc<MeshBuffers>,
    pub face_refs: Vec<Reference>,
    pub edge_refs: Vec<Reference>,
    pub vertex_refs: Vec<Reference>,
    pub bounds: Option<(DVec3, DVec3)>,
    /// A Composite part: its members and whether it is closed.
    pub composite: Option<CompositeInfo>,
    /// A mesh part (imported STL, OBJ, PVZ, …): parts-list mesh icon, faceted.
    pub is_mesh: bool,
    /// Colours features gave single faces (an import's), by face index,
    /// linear RGBA; empty when none. Face appearances set in the Part
    /// Studio draw over them.
    pub face_colors: Vec<Option<[f32; 4]>>,
}

/// A Composite part's members (Help PartStudio__composite_part). An open
/// composite keeps them as parts of their own (its body is not drawn: the
/// members are); a closed one consumed them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositeInfo {
    pub members: Vec<BodyId>,
    pub closed: bool,
}

/// Constraint state of a sketch entity (drives color).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SketchEntityState {
    Under,
    Full,
    Over,
    /// Inactive sketch (not being edited).
    Inactive,
}

#[derive(Clone, Debug)]
pub struct SketchCurveDisplay {
    pub entity: u32,
    pub points: Vec<[f32; 3]>,
    pub construction: bool,
    pub external: bool,
    pub state: SketchEntityState,
}

#[derive(Clone, Debug)]
pub struct SketchPointDisplay {
    pub entity: u32,
    pub position: [f32; 3],
    pub state: SketchEntityState,
    /// Endpoints of curves vs. standalone sketch points.
    pub standalone: bool,
    /// Interior control point of a spline (drawn smaller).
    pub handle: bool,
}

#[derive(Clone, Debug)]
pub struct SketchRegionDisplay {
    pub key: Vec<u32>,
    /// Triangulated region in world space.
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub area: f64,
}

/// Coordinate frame of a sketch plane (world space).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneFrame {
    pub origin: DVec3,
    pub x: DVec3,
    pub y: DVec3,
    pub normal: DVec3,
}

impl PlaneFrame {
    pub fn to_world(&self, p: glam::DVec2) -> DVec3 {
        self.origin + self.x * p.x + self.y * p.y
    }
    pub fn to_local(&self, p: DVec3) -> glam::DVec2 {
        let d = p - self.origin;
        glam::DVec2::new(d.dot(self.x), d.dot(self.y))
    }
    /// Intersect a ray with the plane.
    pub fn intersect_ray(&self, origin: DVec3, dir: DVec3) -> Option<DVec3> {
        let denom = dir.dot(self.normal);
        if denom.abs() < 1e-12 {
            return None;
        }
        let t = (self.origin - origin).dot(self.normal) / denom;
        Some(origin + dir * t)
    }
    pub const TOP: PlaneFrame = PlaneFrame { origin: DVec3::ZERO, x: DVec3::X, y: DVec3::Y, normal: DVec3::Z };
    pub const FRONT: PlaneFrame = PlaneFrame { origin: DVec3::ZERO, x: DVec3::X, y: DVec3::Z, normal: DVec3::NEG_Y };
    pub const RIGHT: PlaneFrame = PlaneFrame { origin: DVec3::ZERO, x: DVec3::Y, y: DVec3::Z, normal: DVec3::X };
}

#[derive(Clone, Debug)]
pub struct DimensionDisplay {
    pub constraint: u32,
    /// Anchor position for the label (world).
    pub label_pos: [f32; 3],
    pub text: String,
    /// Leader/extension line segments (world).
    pub lines: Vec<[[f32; 3]; 2]>,
    pub driven: bool,
    pub state: SketchEntityState,
}

#[derive(Clone, Debug)]
pub struct ConstraintGlyph {
    pub constraint: u32,
    pub kind: &'static str,
    pub position: [f32; 3],
    pub conflicting: bool,
}

#[derive(Clone, Debug)]
pub struct SketchDisplay {
    pub feature: FeatureId,
    pub name: String,
    pub frame: PlaneFrame,
    pub curves: Vec<SketchCurveDisplay>,
    pub points: Vec<SketchPointDisplay>,
    pub regions: Vec<SketchRegionDisplay>,
    pub dimensions: Vec<DimensionDisplay>,
    pub glyphs: Vec<ConstraintGlyph>,
    pub hidden: bool,
    /// Remaining degrees of freedom (0 = fully defined).
    pub dof: usize,
    /// Custom sketch appearance (linear RGBA) when not being edited.
    pub color: Option<[f32; 4]>,
    /// Inserted images in their boxes (world corners: lower-left,
    /// lower-right, upper-right, upper-left).
    pub images: Vec<SketchImageDisplay>,
}

#[derive(Clone, Debug)]
pub struct SketchImageDisplay {
    pub id: u32,
    pub corners: [[f32; 3]; 4],
    /// PNG / JPEG / GIF / BMP bytes, base64 encoded.
    pub data_base64: std::sync::Arc<str>,
}

#[derive(Clone, Debug)]
pub struct PlaneDisplay {
    pub reference: Reference,
    pub name: String,
    pub frame: PlaneFrame,
    pub half_width: f64,
    pub half_height: f64,
    pub hidden: bool,
}

#[derive(Clone, Debug)]
pub struct ConnectorDisplay {
    pub reference: Reference,
    pub name: String,
    pub origin: DVec3,
    pub x: DVec3,
    pub z: DVec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureStatus {
    Ok,
    Info,
    Warning,
    Error,
    Suppressed,
    /// Below the rollback bar.
    RolledBack,
}

#[derive(Clone, Debug)]
pub struct FeatureState {
    pub id: FeatureId,
    pub status: FeatureStatus,
    pub message: Option<String>,
    /// Sketch under-defined (blue in the list).
    pub underdefined: bool,
    /// Milliseconds spent regenerating this feature.
    pub time_ms: f32,
    /// Parameters at fault when it failed.
    pub faulty: Vec<String>,
    /// Values the feature computed for its read-only parameters (Onshape's
    /// `setFeatureComputedParameter`, e.g. a Hole's Up to next depth).
    pub computed: Vec<(String, ComputedValue)>,
}

/// A computed parameter value: a length (meters) or text such as "Multiple".
#[derive(Clone, Debug, PartialEq)]
pub enum ComputedValue {
    Length(f64),
    Text(String),
}

impl FeatureState {
    pub fn computed(&self, id: &str) -> Option<&ComputedValue> {
        self.computed.iter().find(|(k, _)| k == id).map(|(_, v)| v)
    }
}

#[derive(Clone, Debug, Default)]
pub struct StudioDisplay {
    pub bodies: Vec<BodyDisplay>,
    pub sketches: Vec<SketchDisplay>,
    pub planes: Vec<PlaneDisplay>,
    pub connectors: Vec<ConnectorDisplay>,
    pub features: Vec<FeatureState>,
    /// Origin visible.
    pub show_origin: bool,
    /// Total regeneration time.
    pub regen_ms: f32,
    /// Variables defined by the evaluated features (for expression entry).
    pub variables: std::collections::BTreeMap<String, noshape_units::Quantity>,
    /// The shown Assembly context's bodies (translucent, pickable for
    /// references; not parts of this Part Studio, see `crate::in_context`).
    pub context_bodies: Vec<BodyDisplay>,
}

impl StudioDisplay {
    /// Display of an empty Part Studio: origin and the three default planes.
    pub fn defaults() -> Self {
        let half = 0.05;
        let mk = |p: crate::reference::DatumPlane, name: &str, frame| PlaneDisplay {
            reference: Reference::Plane(p),
            name: name.into(),
            frame,
            half_width: half,
            half_height: half,
            hidden: false,
        };
        StudioDisplay {
            planes: vec![
                mk(crate::reference::DatumPlane::Top, "Top", PlaneFrame::TOP),
                mk(crate::reference::DatumPlane::Front, "Front", PlaneFrame::FRONT),
                mk(crate::reference::DatumPlane::Right, "Right", PlaneFrame::RIGHT),
            ],
            show_origin: true,
            ..Default::default()
        }
    }

    pub fn feature_state(&self, id: FeatureId) -> Option<&FeatureState> {
        self.features.iter().find(|f| f.id == id)
    }

    pub fn body(&self, id: &BodyId) -> Option<&BodyDisplay> {
        self.bodies.iter().find(|b| &b.id == id)
    }

    pub fn bounds(&self) -> Option<(DVec3, DVec3)> {
        let mut acc: Option<(DVec3, DVec3)> = None;
        for b in self.bodies.iter().filter(|b| !b.hidden) {
            if let Some((lo, hi)) = b.bounds {
                acc = Some(match acc {
                    None => (lo, hi),
                    Some((a, c)) => (a.min(lo), c.max(hi)),
                });
            }
        }
        acc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two quads sharing the edge x = 0: flat (tangent) or folded (sharp).
    fn two_faces(fold: bool) -> MeshBuffers {
        let n2: [f32; 3] = if fold { [1.0, 0.0, 0.0] } else { [0.0, 0.0, 1.0] };
        let p2 = |y: f32| if fold { [0.0, y, -1.0] } else { [1.0, y, 0.0] };
        MeshBuffers {
            positions: vec![[-1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [0.0, 0.0, 0.0], p2(0.0), p2(1.0), [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], n2, n2, n2, n2],
            indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
            face_ranges: vec![(0, 6), (6, 6)],
            edges: vec![vec![[0.0, 0.0, 0.0], [0.0, 1.0, 0.0]], vec![[-1.0, 0.0, 0.0], [-1.0, 1.0, 0.0]]],
            vertices: vec![],
            mesh_lines: vec![],
        }
    }

    #[test]
    fn tangent_edges_from_the_mesh() {
        assert_eq!(two_faces(false).smooth_edges(), vec![true, false]);
        assert_eq!(two_faces(true).smooth_edges(), vec![false, false]);
    }

    #[test]
    fn edges_of_a_face() {
        let m = two_faces(false);
        assert_eq!(m.face_edges(0), vec![0, 1]);
        assert_eq!(m.face_edges(1), vec![0]);
        assert!(m.face_edges(5).is_empty());
    }
}
