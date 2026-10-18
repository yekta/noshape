//! Persistent references ("queries") from features to geometry.
//!
//! Features never store sub-shape indices. They store [`Reference`]s built
//! from persistent [`TopoName`]s that are derived only from feature ids,
//! sketch entity ids and creation roles, so they survive regeneration and
//! upstream edits. See `docs/ARCHITECTURE.md` (Persistent topological naming).
//!
//! Naming scheme:
//! * Faces are named from history: the feature that created them plus a
//!   [`Role`] (side of which profile edge, start/end cap, blend of which edge…).
//!   When a later operation splits a face, the pieces become
//!   `Split { parent, ordinal }` with ordinals ordered by a deterministic
//!   geometric key; a reference to the parent resolves to all pieces.
//! * Edges are named by their (sorted) pair of adjacent face names plus an
//!   ordinal for multiple edges between the same faces.
//! * Vertices are named by their sorted set of adjacent face names.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// Stable feature identifier, unique within a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FeatureId(pub u64);

impl FeatureId {
    pub fn new_random() -> Self {
        // 48 random bits are plenty within one document and keep names short.
        FeatureId(rand_u64() & 0xFFFF_FFFF_FFFF)
    }
}

impl fmt::Display for FeatureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "F{:x}", self.0)
    }
}

pub(crate) fn rand_u64() -> u64 {
    let u = uuid::Uuid::new_v4();
    let b = u.as_bytes();
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

/// Where a generated face/edge came from in a sketch or wire profile.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Source {
    /// Segment of a sketch entity. `segment` orders pieces of an entity split
    /// by intersections along its parameter.
    Sketch { sketch: FeatureId, entity: u32, segment: u16 },
    /// An existing named face/edge/vertex used as profile (e.g. extruding a face).
    Topo(Arc<TopoName>),
    /// A sketch region used as a profile (encoded region key, see
    /// [`Reference::SketchRegion`]).
    Region { sketch: FeatureId, key: Vec<u32> },
    /// Fallback when nothing better is known; ordered geometrically.
    Index(u32),
}

/// Why a face exists.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Role {
    /// Face swept from a profile edge (extrude/revolve/sweep/loft sides).
    Side(Source),
    /// Start cap (the profile itself) and end cap, keyed by the profile.
    CapStart(Option<Source>),
    CapEnd(Option<Source>),
    /// Face of the profile region itself (sketch region / planar face profile).
    Profile(Source),
    /// Fillet/chamfer face created from an edge or vertex.
    Blend(Arc<TopoName>),
    /// Generic "generated from" relation (e.g. a vertex sweeping into a face).
    Generated(Arc<TopoName>),
    /// Primitive / imported / otherwise new faces, ordered geometrically.
    New(u32),
    /// Copy of a face made by a pattern/mirror/transform instance.
    Copy { instance: u32, of: Arc<TopoName> },
    /// Offset/thicken/shell counterpart of a face.
    Offset(Arc<TopoName>),
    /// Named datum or sketch-produced face.
    Datum(u32),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TopoName {
    Face { feature: FeatureId, role: Role },
    /// One piece of a face that a later feature split.
    Split { parent: Arc<TopoName>, by: FeatureId, ordinal: u16 },
    Edge { faces: [Arc<TopoName>; 2], ordinal: u16 },
    Vertex { faces: Arc<[Arc<TopoName>]>, ordinal: u16 },
    /// Edges/vertices of wire bodies and sketches.
    Wire { feature: FeatureId, source: Source },
    /// Lamina edges (open sheet boundaries) named by their single face.
    Boundary { face: Arc<TopoName>, ordinal: u16 },
}

impl TopoName {
    pub fn face(feature: FeatureId, role: Role) -> Self {
        TopoName::Face { feature, role }
    }

    /// Edge name from two adjacent face names (order independent).
    pub fn edge(a: Arc<TopoName>, b: Arc<TopoName>, ordinal: u16) -> Self {
        let faces = if a <= b { [a, b] } else { [b, a] };
        TopoName::Edge { faces, ordinal }
    }

    pub fn vertex(mut faces: Vec<Arc<TopoName>>, ordinal: u16) -> Self {
        faces.sort();
        faces.dedup();
        TopoName::Vertex { faces: faces.into(), ordinal }
    }

    /// Whether `self` is `ancestor` or descends from it through splits.
    pub fn descends_from(&self, ancestor: &TopoName) -> bool {
        let mut cur = self;
        loop {
            if cur == ancestor {
                return true;
            }
            match cur {
                TopoName::Split { parent, .. } => cur = parent,
                _ => return false,
            }
        }
    }

    /// Strip split wrappers: the original face this one came from.
    pub fn root(&self) -> &TopoName {
        let mut cur = self;
        while let TopoName::Split { parent, .. } = cur {
            cur = parent;
        }
        cur
    }

    /// The feature that originally created this entity (for faces).
    pub fn creator(&self) -> Option<FeatureId> {
        match self.root() {
            TopoName::Face { feature, .. } | TopoName::Wire { feature, .. } => Some(*feature),
            _ => None,
        }
    }
}

/// Stable identifier of a body (part, surface, wire body) in a Part Studio.
/// Assigned by the feature that created the body; kept by boolean targets.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BodyId {
    pub feature: FeatureId,
    pub index: u32,
}

impl fmt::Display for BodyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.feature, self.index)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DatumPlane {
    Top,
    Front,
    Right,
}

/// Implicit mate connector inference (Onshape's hover points).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Inference {
    Centroid,
    Center,
    Midpoint,
    Vertex,
    TopAxis,
    MidAxis,
    BottomAxis,
    Origin,
}

/// A persistent selection of one entity.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Reference {
    Origin,
    Plane(DatumPlane),
    Face(TopoName),
    Edge(TopoName),
    Vertex(TopoName),
    Body(BodyId),
    /// A sketch entity (curve or point) of a sketch feature.
    SketchEntity { sketch: FeatureId, entity: u32 },
    /// A closed region of a sketch. `key` encodes the sketch solver's
    /// region id: `[ordinal, (entity << 1) | forward, ...]`.
    SketchRegion { sketch: FeatureId, key: Vec<u32> },
    /// A whole sketch.
    Sketch(FeatureId),
    /// Plane created by a Plane feature.
    FeaturePlane(FeatureId),
    /// Mate connector created by a feature (index for features creating several).
    MateConnector { feature: FeatureId, index: u32 },
    /// A feature (for feature patterns and "Up to" by feature).
    Feature(FeatureId),
    /// Implicit mate connector inferred on an entity.
    Implicit { on: Box<Reference>, inference: Inference },
    /// A query variable (`#name`, defined by a Query variable feature
    /// upstream). Stands for the variable's references; the regenerator
    /// expands it before a feature is evaluated and the `EvalCtx::resolve_*`
    /// helpers accept it too. Serialized as `{"QueryVariable":"name"}`;
    /// documents without it deserialize unchanged.
    QueryVariable(String),
}

impl Reference {
    /// Feature this reference depends on, if any (for dependency tracking).
    pub fn depends_on(&self) -> Option<FeatureId> {
        match self {
            Reference::Origin | Reference::Plane(_) => None,
            Reference::Face(n) | Reference::Edge(n) | Reference::Vertex(n) => n.creator(),
            Reference::Body(b) => Some(b.feature),
            Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } | Reference::Sketch(sketch) => Some(*sketch),
            Reference::FeaturePlane(f) | Reference::Feature(f) => Some(*f),
            Reference::MateConnector { feature, .. } => Some(*feature),
            Reference::Implicit { on, .. } => on.depends_on(),
            // Depends on the defining Query variable feature, which is only
            // known by name here.
            Reference::QueryVariable(_) => None,
        }
    }

    /// Short user-facing label ("Face of Extrude 1", "Top plane").
    pub fn kind_label(&self) -> &'static str {
        match self {
            Reference::Origin => "Origin",
            Reference::Plane(DatumPlane::Top) => "Top plane",
            Reference::Plane(DatumPlane::Front) => "Front plane",
            Reference::Plane(DatumPlane::Right) => "Right plane",
            Reference::Face(_) => "Face",
            Reference::Edge(_) => "Edge",
            Reference::Vertex(_) => "Vertex",
            Reference::Body(_) => "Part",
            Reference::SketchEntity { .. } => "Sketch entity",
            Reference::SketchRegion { .. } => "Sketch region",
            Reference::Sketch(_) => "Sketch",
            Reference::FeaturePlane(_) => "Plane",
            Reference::MateConnector { .. } => "Mate connector",
            Reference::Feature(_) => "Feature",
            Reference::Implicit { .. } => "Mate connector",
            Reference::QueryVariable(_) => "Query variable",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_names_are_order_independent() {
        let f = FeatureId(1);
        let a = Arc::new(TopoName::face(f, Role::CapStart(None)));
        let b = Arc::new(TopoName::face(f, Role::CapEnd(None)));
        assert_eq!(TopoName::edge(a.clone(), b.clone(), 0), TopoName::edge(b, a, 0));
    }

    #[test]
    fn split_descends_from_parent() {
        let f = FeatureId(1);
        let a = Arc::new(TopoName::face(f, Role::CapStart(None)));
        let s = TopoName::Split { parent: a.clone(), by: FeatureId(2), ordinal: 1 };
        assert!(s.descends_from(&a));
        assert_eq!(s.root(), &*a);
        assert_eq!(s.creator(), Some(f));
    }

    #[test]
    fn references_round_trip_through_json() {
        let f = FeatureId(7);
        let r = Reference::Edge(TopoName::edge(
            Arc::new(TopoName::face(f, Role::Side(Source::Sketch { sketch: FeatureId(3), entity: 4, segment: 0 }))),
            Arc::new(TopoName::face(f, Role::CapEnd(None))),
            0,
        ));
        let s = serde_json::to_string(&r).unwrap();
        let back: Reference = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn query_variable_reference_serde() {
        let r = Reference::QueryVariable("holes".into());
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"QueryVariable":"holes"}"#);
        assert_eq!(serde_json::from_str::<Reference>(&s).unwrap(), r);
        // Older variants still parse.
        let old: Reference = serde_json::from_str(r#"{"Plane":"Top"}"#).unwrap();
        assert_eq!(old, Reference::Plane(DatumPlane::Top));
    }
}
