//! noshape's own boundary representation (docs/BOOLEAN-SPEEDUP.md phase 3): topology in index-based arenas, and a
//! Boolean (fuse, cut, common) for solids bounded by planes. The kernel converts OCCT shapes to [`Body`] and back
//! and runs this Boolean where it accepts the inputs; anything it doesn't handle is a [`Decline`], and OCCT runs.
//!
//! The result is what OCCT's Boolean (without unify) makes of the same inputs: every input face split by the
//! other solids' faces and by their edges lying on it, the pieces kept by where they lie, input edges split at
//! the vertices on them, coincident faces (same domain) kept once. The history says which input face, edge,
//! vertex and solid became which output one, in OCCT's terms (Modified / Generated / Deleted; whatever is
//! reused unchanged is Kept).
//!
//! Tolerances are explicit: two points closer than the call's `tol` are one, a point closer than `tol` to a
//! plane lies on it. A distance that is neither clearly within nor clearly beyond `tol` (up to [`GREY`] times
//! it) is ambiguous, and the Boolean declines rather than guess.

mod arrange;
mod boolean;

pub use boolean::{boolean, Op};
use glam::DVec3;

/// Distances in (tol, GREY × tol] are ambiguous: the Boolean declines.
pub const GREY: f64 = 50.0;

/// An edge's curve. Straight edges run between their vertices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Line,
}

/// A face's surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Surface {
    /// The points `x` with `normal · x == normal · origin` (`normal` unit).
    Plane { origin: DVec3, normal: DVec3 },
}

/// An edge between two vertices (`v[0]` its start, `v[1]` its end: one forward direction).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub v: [u32; 2],
    pub curve: Curve,
}

/// An edge as a face's boundary uses it: `reversed` runs it from `v[1]` to `v[0]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Use {
    pub edge: u32,
    pub reversed: bool,
}

/// A face: its surface and boundary loops. `reversed`: the material is on the surface normal's side (the
/// outward normal is minus the surface's). Looking against the outward normal, outer loops run
/// counter-clockwise and holes clockwise; an input face's loops may list their edges in any order.
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    pub surface: Surface,
    pub reversed: bool,
    pub loops: Vec<Vec<Use>>,
}

impl Face {
    /// The outward normal.
    pub fn normal(&self) -> DVec3 {
        let Surface::Plane { normal, .. } = self.surface;
        if self.reversed { -normal } else { normal }
    }
}

/// A solid: closed shells of faces, the first the outer one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Solid {
    pub shells: Vec<Vec<u32>>,
}

/// Vertices, edges, faces and solids, each referring to the ones below by index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Body {
    pub vertices: Vec<DVec3>,
    pub edges: Vec<Edge>,
    pub faces: Vec<Face>,
    pub solids: Vec<Solid>,
}

/// Why a Boolean declined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decline(pub &'static str);

/// Where an output entity comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The input entity (`arg`, `index`) itself, unchanged.
    Same { arg: u32, index: u32 },
    /// A new entity on the input one's curve or surface (a piece of it).
    Piece { arg: u32, index: u32 },
    /// A new entity (a vertex, an edge where two faces meet, a solid).
    New,
}

/// Kinds of entity, as the history names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Solid,
    Face,
    Edge,
    Vertex,
}

/// History relations (OCCT's BRepTools_History). An input entity with no record and an output that is
/// [`Origin::Same`] is Kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Relation {
    Modified,
    Generated,
    Deleted,
}

/// One history record: input entity (`kind`, `arg`, `index`) → output entity `out` of the same kind
/// (Modified), or of `out_kind` (Generated); Deleted has no output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Record {
    pub kind: Kind,
    pub arg: u32,
    pub index: u32,
    pub relation: Relation,
    pub out_kind: Kind,
    pub out: u32,
}

/// A Boolean's result: the body, where each of its entities comes from, and the history.
#[derive(Clone, Debug, Default)]
pub struct Output {
    pub body: Body,
    pub vertex_origin: Vec<Origin>,
    pub edge_origin: Vec<Origin>,
    pub face_origin: Vec<Origin>,
    pub solid_origin: Vec<Origin>,
    pub history: Vec<Record>,
}

#[cfg(test)]
mod tests;
