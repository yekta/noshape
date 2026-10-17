//! Tessellation into flat GPU-friendly buffers.

use serde::{Deserialize, Serialize};

use crate::error::{KernelError, Result};
use crate::ffi::ffi;
use crate::shape::{Shape, ck};

/// Index range of one face in a [`Tessellation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FaceRange {
    /// First index into [`Tessellation::indices`].
    pub first_index: u32,
    /// Number of indices (3 per triangle).
    pub index_count: u32,
    /// First vertex in `positions` / `normals`.
    pub first_vertex: u32,
    /// Number of vertices of this face.
    pub vertex_count: u32,
}

impl FaceRange {
    /// Number of triangles of the face.
    pub fn triangle_count(&self) -> u32 {
        self.index_count / 3
    }
}

/// Range of one edge polyline in [`Tessellation::edge_points`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EdgeRange {
    pub first_point: u32,
    pub point_count: u32,
}

/// A triangulated shape as flat buffers.
///
/// Faces, edges and vertices are indexed in `TopExp::MapShapes` order of the
/// tessellated shape (same as [`Shape::faces`] etc.), so `faces[i]` is face
/// `i` and `edges[j]` is edge `j`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tessellation {
    pub positions: Vec<[f32; 3]>,
    /// Unit normals per vertex, pointing out of the material (face orientation
    /// applied).
    pub normals: Vec<[f32; 3]>,
    /// Triangle indices (counter-clockwise seen from outside).
    pub indices: Vec<u32>,
    pub faces: Vec<FaceRange>,
    /// Polyline points of all edges; edge polylines coincide with the mesh
    /// (Poly_PolygonOnTriangulation) when the edge borders a meshed face.
    pub edge_points: Vec<[f32; 3]>,
    pub edges: Vec<EdgeRange>,
    /// Vertex positions (one per topological vertex).
    pub vertices: Vec<[f32; 3]>,
}

impl Tessellation {
    /// Total number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
    /// Indices of face `i`'s triangles.
    pub fn face_indices(&self, i: usize) -> &[u32] {
        let r = self.faces[i];
        &self.indices[r.first_index as usize..(r.first_index + r.index_count) as usize]
    }
    /// Polyline of edge `i` (empty for degenerated edges).
    pub fn edge_polyline(&self, i: usize) -> &[[f32; 3]] {
        let r = self.edges[i];
        &self.edge_points[r.first_point as usize..(r.first_point + r.point_count) as usize]
    }
}

/// Meshing parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MeshOptions {
    /// Maximum chordal deviation (model units, or relative to edge size if
    /// `relative`).
    pub linear_deflection: f64,
    /// Maximum angle between adjacent segments/normals (radians).
    pub angular_deflection: f64,
    pub relative: bool,
    /// Mesh faces in parallel inside OCCT.
    pub parallel: bool,
}

impl Default for MeshOptions {
    fn default() -> Self {
        MeshOptions { linear_deflection: 0.1, angular_deflection: 0.5, relative: false, parallel: true }
    }
}

fn as_flat_mut(v: &mut [[f32; 3]]) -> &mut [f32] {
    // SAFETY: [f32; 3] has the layout of three consecutive f32 values.
    unsafe { std::slice::from_raw_parts_mut(v.as_mut_ptr() as *mut f32, v.len() * 3) }
}


/// Tessellate a shape with BRepMesh_IncrementalMesh (existing triangulations
/// that are already fine enough are reused) and copy everything into flat
/// buffers in a single bridge call.
pub fn tessellate(shape: &Shape, linear_deflection: f64, angular_deflection: f64, parallel: bool) -> Result<Tessellation> {
    tessellate_with(
        shape,
        &MeshOptions { linear_deflection, angular_deflection, relative: false, parallel },
    )
}

/// [`tessellate`] with full options.
pub fn tessellate_with(shape: &Shape, o: &MeshOptions) -> Result<Tessellation> {
    if shape.is_null() {
        return Err(KernelError::invalid("cannot tessellate a null shape"));
    }
    let m = ck(ffi::mesh_prepare(shape.raw(), o.linear_deflection, o.angular_deflection, o.relative, o.parallel))?;
    let mr = m.as_ref().ok_or_else(|| KernelError::Occt("mesh_prepare returned null".into()))?;
    let c = ck(ffi::mesh_counts(mr))?;
    let mut t = Tessellation {
        positions: vec![[0.0; 3]; c.nodes],
        normals: vec![[0.0; 3]; c.nodes],
        indices: vec![0; c.indices],
        faces: Vec::new(),
        edge_points: vec![[0.0; 3]; c.edge_points],
        edges: Vec::new(),
        vertices: vec![[0.0; 3]; c.vertices],
    };
    let mut face_ranges = vec![0u32; c.faces * 4];
    let mut edge_ranges = vec![0u32; c.edges * 2];
    ck(ffi::mesh_fill(
        mr,
        as_flat_mut(&mut t.positions),
        as_flat_mut(&mut t.normals),
        &mut t.indices,
        &mut face_ranges,
        as_flat_mut(&mut t.edge_points),
        &mut edge_ranges,
        as_flat_mut(&mut t.vertices),
    ))?;
    t.faces = face_ranges
        .as_chunks::<4>()
        .0
        .iter()
        .map(|r| FaceRange { first_index: r[0], index_count: r[1], first_vertex: r[2], vertex_count: r[3] })
        .collect();
    t.edges = edge_ranges
        .as_chunks::<2>()
        .0
        .iter()
        .map(|r| EdgeRange { first_point: r[0], point_count: r[1] })
        .collect();
    Ok(t)
}
