//! [`Shape`]: the B-rep shape handle, and topology exploration.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use cxx::UniquePtr;
use serde::{Deserialize, Serialize};

use crate::error::{KernelError, Result};
use crate::ffi::ffi;

/// Topological shape type (values match OCCT `TopAbs_ShapeEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum ShapeType {
    /// A group of arbitrary shapes.
    Compound = 0,
    /// A set of solids connected by their faces.
    CompSolid = 1,
    /// A closed volume bounded by shells.
    Solid = 2,
    /// A set of faces connected by their edges.
    Shell = 3,
    /// A bounded piece of a surface.
    Face = 4,
    /// A sequence of connected edges.
    Wire = 5,
    /// A bounded piece of a curve.
    Edge = 6,
    /// A point.
    Vertex = 7,
    /// Null shape / unknown (used for "no output" in history records).
    Unknown = 8,
}

impl ShapeType {
    pub(crate) fn from_u8(v: u8) -> ShapeType {
        match v {
            0 => ShapeType::Compound,
            1 => ShapeType::CompSolid,
            2 => ShapeType::Solid,
            3 => ShapeType::Shell,
            4 => ShapeType::Face,
            5 => ShapeType::Wire,
            6 => ShapeType::Edge,
            7 => ShapeType::Vertex,
            _ => ShapeType::Unknown,
        }
    }
}

/// Orientation of a shape relative to its underlying geometry / parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Orientation {
    Forward = 0,
    Reversed = 1,
    Internal = 2,
    External = 3,
}

pub(crate) struct ShapeInner {
    pub(crate) ptr: UniquePtr<ffi::OcShape>,
}

/// A B-rep shape (wrapper around OCCT `TopoDS_Shape`).
///
/// `Shape` is immutable and cheap to clone (an `Arc` bump). It is `Send` and
/// `Sync`: OCCT shape handles use atomic reference counts and the kernel never
/// mutates a `TopoDS_Shape` after construction. The one place where OCCT
/// writes into shared topology — attaching triangulations during meshing — is
/// serialized by a global lock inside the C++ bridge.
///
/// Equality and hashing use OCCT `IsSame` semantics (same underlying TShape
/// and location, orientation ignored), so a `Shape` can be used directly as a
/// `HashMap` key for sub-shapes.
#[derive(Clone)]
pub struct Shape {
    pub(crate) inner: Arc<ShapeInner>,
}

pub(crate) fn ck<T>(r: std::result::Result<T, cxx::Exception>) -> Result<T> {
    r.map_err(KernelError::from)
}

impl Shape {
    pub(crate) fn from_ptr(ptr: UniquePtr<ffi::OcShape>) -> Shape {
        Shape {
            inner: Arc::new(ShapeInner { ptr }),
        }
    }

    pub(crate) fn from_res(r: std::result::Result<UniquePtr<ffi::OcShape>, cxx::Exception>) -> Result<Shape> {
        Ok(Shape::from_ptr(ck(r)?))
    }

    pub(crate) fn raw(&self) -> &ffi::OcShape {
        // The pointer is never null: every constructor wraps a valid OcShape.
        self.inner.ptr.as_ref().expect("OcShape pointer is never null")
    }

    /// The null shape.
    pub fn null() -> Shape {
        Shape::from_ptr(ffi::shape_null().expect("shape_null cannot fail"))
    }

    /// True if this is the null shape.
    pub fn is_null(&self) -> bool {
        ffi::shape_is_null(self.raw()).unwrap_or(true)
    }

    /// Topological type; [`ShapeType::Unknown`] for the null shape.
    pub fn shape_type(&self) -> ShapeType {
        ShapeType::from_u8(ffi::shape_type(self.raw()).unwrap_or(8))
    }

    /// Orientation of the shape.
    pub fn orientation(&self) -> Orientation {
        match ffi::shape_orientation(self.raw()).unwrap_or(0) {
            1 => Orientation::Reversed,
            2 => Orientation::Internal,
            3 => Orientation::External,
            _ => Orientation::Forward,
        }
    }

    /// The same shape with the given orientation.
    pub fn oriented(&self, o: Orientation) -> Result<Shape> {
        Shape::from_res(ffi::shape_oriented(self.raw(), o as u8))
    }

    /// The same shape with reversed orientation.
    pub fn reversed(&self) -> Result<Shape> {
        let o = match self.orientation() {
            Orientation::Forward => Orientation::Reversed,
            Orientation::Reversed => Orientation::Forward,
            other => other,
        };
        self.oriented(o)
    }

    /// Same TShape and same location (orientation ignored). OCCT `IsSame`.
    pub fn is_same(&self, other: &Shape) -> bool {
        ffi::shape_is_same(self.raw(), other.raw()).unwrap_or(false)
    }

    /// Same TShape, location and orientation. OCCT `IsEqual`.
    pub fn is_equal(&self, other: &Shape) -> bool {
        ffi::shape_is_equal(self.raw(), other.raw()).unwrap_or(false)
    }

    /// Same TShape (location and orientation ignored). OCCT `IsPartner`.
    pub fn is_partner(&self, other: &Shape) -> bool {
        ffi::shape_is_partner(self.raw(), other.raw()).unwrap_or(false)
    }

    /// Hash consistent with [`Shape::is_partner`]: the TShape's identity.
    /// Rigidly moved pattern instances share it with their seed, so one
    /// mesh of [`Shape::unlocated`] serves them all, placed by
    /// [`Shape::location`].
    pub fn partner_hash(&self) -> u64 {
        ffi::shape_partner_hash(self.raw()).unwrap_or(0)
    }

    /// The shape's own location: a rigid 3x4 row-major matrix (identity for
    /// most shapes).
    pub fn location(&self) -> [f64; 12] {
        let m = ffi::shape_location(self.raw()).unwrap_or_default();
        std::array::from_fn(|i| m.get(i).copied().unwrap_or(if i % 5 == 0 { 1.0 } else { 0.0 }))
    }

    /// The same TShape at the identity location.
    pub fn unlocated(&self) -> Result<Shape> {
        Shape::from_res(ffi::shape_unlocated(self.raw()))
    }

    /// Hash consistent with [`Shape::is_same`] (`std::hash<TopoDS_Shape>`).
    pub fn hash_code(&self) -> u64 {
        ffi::shape_hash(self.raw()).unwrap_or(0)
    }

    /// Number of distinct sub-shapes of `kind` (TopExp::MapShapes).
    pub fn count(&self, kind: ShapeType) -> usize {
        ffi::sub_count(self.raw(), kind as u8).unwrap_or(0) as usize
    }

    /// All distinct sub-shapes of `kind`, in the deterministic order of
    /// `TopExp::MapShapes` (an indexed map). These indices are the ones used
    /// by [`crate::History`], tessellation face ranges and everything else
    /// that refers to sub-shapes by index.
    pub fn sub_shapes(&self, kind: ShapeType) -> Vec<Shape> {
        match ffi::sub_shapes(self.raw(), kind as u8) {
            Ok(l) => list_to_vec(&l),
            Err(_) => Vec::new(),
        }
    }

    /// The `index`-th sub-shape of `kind` (MapShapes order).
    pub fn sub_shape(&self, kind: ShapeType, index: usize) -> Result<Shape> {
        let index = u32::try_from(index).map_err(|_| KernelError::invalid("index out of range"))?;
        Shape::from_res(ffi::sub_shape(self.raw(), kind as u8, index))
    }

    /// Faces in MapShapes order.
    pub fn faces(&self) -> Vec<Shape> {
        self.sub_shapes(ShapeType::Face)
    }
    /// Edges in MapShapes order.
    pub fn edges(&self) -> Vec<Shape> {
        self.sub_shapes(ShapeType::Edge)
    }
    /// Vertices in MapShapes order.
    pub fn vertices(&self) -> Vec<Shape> {
        self.sub_shapes(ShapeType::Vertex)
    }
    /// Solids in MapShapes order.
    pub fn solids(&self) -> Vec<Shape> {
        self.sub_shapes(ShapeType::Solid)
    }
    /// Wires in MapShapes order.
    pub fn wires(&self) -> Vec<Shape> {
        self.sub_shapes(ShapeType::Wire)
    }
    /// Shells in MapShapes order.
    pub fn shells(&self) -> Vec<Shape> {
        self.sub_shapes(ShapeType::Shell)
    }

    /// Index of `sub` among this shape's sub-shapes of `kind` (O(1) after the
    /// first query of that kind: the indexed maps are cached per shape).
    pub fn index_of(&self, sub: &Shape, kind: ShapeType) -> Option<usize> {
        match ffi::index_of(self.raw(), sub.raw(), kind as u8) {
            Ok(i) if i >= 0 => Some(i as usize),
            _ => None,
        }
    }

    /// Full ancestor relation: for every sub-shape of `kind` (by index), the
    /// indices of the sub-shapes of `ancestor_kind` containing it
    /// (e.g. edge → faces, vertex → edges). Uses TopExp::MapShapesAndAncestors.
    pub fn ancestor_map(&self, kind: ShapeType, ancestor_kind: ShapeType) -> Result<AncestorMap> {
        let csr = ck(ffi::ancestor_map(self.raw(), kind as u8, ancestor_kind as u8))?;
        Ok(AncestorMap {
            offsets: csr.offsets,
            values: csr.values,
        })
    }

    /// Indices of the `ancestor_kind` sub-shapes containing sub-shape
    /// `index` of `kind`. Builds the whole map; prefer [`Shape::ancestor_map`]
    /// for many queries.
    pub fn ancestors(&self, kind: ShapeType, index: usize, ancestor_kind: ShapeType) -> Result<Vec<usize>> {
        let m = self.ancestor_map(kind, ancestor_kind)?;
        if index >= m.len() {
            return Err(KernelError::invalid("sub-shape index out of range"));
        }
        Ok(m.get(index).iter().map(|&v| v as usize).collect())
    }

    /// Edge → faces adjacency: `result[e]` = indices of the faces containing
    /// edge `e` (MapShapes order on both sides).
    pub fn edge_faces(&self) -> Result<Vec<Vec<u32>>> {
        Ok(self.ancestor_map(ShapeType::Edge, ShapeType::Face)?.to_vecs())
    }

    /// Vertex → faces adjacency (MapShapes order).
    pub fn vertex_faces(&self) -> Result<Vec<Vec<u32>>> {
        Ok(self.ancestor_map(ShapeType::Vertex, ShapeType::Face)?.to_vecs())
    }

    /// Vertex → edges adjacency (MapShapes order).
    pub fn vertex_edges(&self) -> Result<Vec<Vec<u32>>> {
        Ok(self.ancestor_map(ShapeType::Vertex, ShapeType::Edge)?.to_vecs())
    }

    /// Which edges and vertices Onshape's (Parasolid's) topology has, in
    /// MapShapes order: OCCT's seams and degenerated edges aren't edges there,
    /// and a vertex is one only if it ends a shown open edge, has no edge, or
    /// is a cone's apex (a cylinder has 3 faces, 2 edges and no vertex).
    pub fn shown_topology(&self) -> Result<ShownTopology> {
        let flags = ck(ffi::shown_topology(self.raw()))?;
        let ne = self.count(ShapeType::Edge);
        Ok(ShownTopology { edges: flags[..ne].iter().map(|&f| f != 0).collect(), vertices: flags[ne..].iter().map(|&f| f != 0).collect() })
    }

    /// Faces sharing at least one edge with face `face_index`.
    pub fn adjacent_faces(&self, face_index: usize) -> Result<Vec<usize>> {
        let i = u32::try_from(face_index).map_err(|_| KernelError::invalid("index out of range"))?;
        Ok(ck(ffi::adjacent_faces(self.raw(), i))?
            .into_iter()
            .map(|v| v as usize)
            .collect())
    }

    /// Direct children (TopoDS_Iterator), e.g. the members of a compound.
    pub fn children(&self) -> Vec<Shape> {
        match ffi::children(self.raw()) {
            Ok(l) => list_to_vec(&l),
            Err(_) => Vec::new(),
        }
    }

    /// Build a compound containing `shapes`.
    pub fn compound(shapes: &[Shape]) -> Shape {
        let l = vec_to_list(shapes);
        Shape::from_res(ffi::make_compound(&l)).unwrap_or_else(|_| Shape::null())
    }

    /// A mesh body (or holds one): faces with a triangulation and no surface,
    /// edges or vertices, as Onshape's mesh parts (imported STL, OBJ, PVZ, …).
    pub fn is_mesh(&self) -> bool {
        ffi::is_mesh(self.raw())
    }

    /// Whether a B-rep face carries a stored triangulation (display meshing
    /// leaves none behind; mesh bodies' faces don't count).
    pub fn has_triangulation(&self) -> bool {
        ffi::has_triangulation(self.raw())
    }

    /// BRepCheck_Analyzer validity (mesh faces: a triangulation with triangles).
    pub fn is_valid(&self) -> bool {
        ffi::is_valid(self.raw()).unwrap_or(false)
    }

    /// Whether faces of the shape intersect each other (BOPAlgo_ArgumentAnalyzer
    /// self-interference check; BRepCheck doesn't see this, e.g. a sweep whose
    /// bend is tighter than its profile). ~10 ms on small parts.
    pub fn self_intersects(&self) -> bool {
        ffi::self_intersects(self.raw()).unwrap_or(false)
    }

    /// Heal the shape with ShapeFix_Shape (`tolerance <= 0`: default precision).
    pub fn fixed(&self, tolerance: f64) -> Result<Shape> {
        Shape::from_res(ffi::fix_shape(self.raw(), tolerance))
    }

    /// Human-readable dump of the topology (BRepTools::Dump), for debugging.
    pub fn dump(&self) -> String {
        ffi::dump_shape(self.raw()).unwrap_or_default()
    }
}

/// [`Shape::shown_topology`]: per edge and per vertex (MapShapes order),
/// whether Onshape's topology has it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShownTopology {
    pub edges: Vec<bool>,
    pub vertices: Vec<bool>,
}

impl Default for Shape {
    fn default() -> Self {
        Shape::null()
    }
}

impl PartialEq for Shape {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner) || self.is_same(other)
    }
}
impl Eq for Shape {}

impl Hash for Shape {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash_code().hash(state)
    }
}

impl fmt::Debug for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_null() {
            return write!(f, "Shape(null)");
        }
        write!(
            f,
            "Shape({:?}, faces={}, edges={}, vertices={})",
            self.shape_type(),
            self.count(ShapeType::Face),
            self.count(ShapeType::Edge),
            self.count(ShapeType::Vertex)
        )
    }
}

/// Compressed ancestor relation returned by [`Shape::ancestor_map`].
#[derive(Debug, Clone, Default)]
pub struct AncestorMap {
    offsets: Vec<u32>,
    values: Vec<u32>,
}

impl AncestorMap {
    /// Number of rows (sub-shapes of the queried kind).
    pub fn len(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }
    /// True if there are no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Expand into one `Vec` per row.
    pub fn to_vecs(&self) -> Vec<Vec<u32>> {
        (0..self.len()).map(|i| self.get(i).to_vec()).collect()
    }
    /// Ancestor indices of row `i` (sorted, unique). Empty if out of range.
    pub fn get(&self, i: usize) -> &[u32] {
        if i + 1 >= self.offsets.len() {
            return &[];
        }
        &self.values[self.offsets[i] as usize..self.offsets[i + 1] as usize]
    }
}

pub(crate) fn list_to_vec(l: &ffi::OcShapeList) -> Vec<Shape> {
    let n = ffi::list_len(l).unwrap_or(0);
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        if let Ok(p) = ffi::list_get(l, i) {
            v.push(Shape::from_ptr(p));
        }
    }
    v
}

pub(crate) fn vec_to_list(shapes: &[Shape]) -> UniquePtr<ffi::OcShapeList> {
    let mut l = ffi::list_new().expect("list_new cannot fail");
    for s in shapes {
        ffi::list_push(l.pin_mut(), s.raw()).expect("list_push cannot fail");
    }
    l
}
