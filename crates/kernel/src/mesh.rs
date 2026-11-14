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
    /// Planar faces the kernel's triangulation couldn't take (meshed by
    /// BRepMesh's algorithm instead, much slower on big faces).
    #[serde(default)]
    pub planar_fallbacks: usize,
    /// Faces noshape's own mesher meshed (crates/brep `mesh`), not BRepMesh.
    #[serde(default)]
    pub rust_faces: usize,
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
    /// One vertex per distinct position: `(positions, remap)`, `remap[i]`
    /// being vertex `i`'s index in `positions`. Faces carry their own
    /// vertices; welded, a solid's faces join on their edges' shared points
    /// into the closed mesh 3MF needs (qa EXPORT-MESH-SHARED).
    pub fn welded_positions(&self) -> (Vec<[f32; 3]>, Vec<u32>) {
        let mut at: std::collections::HashMap<[u32; 3], u32> = std::collections::HashMap::new();
        let mut positions = Vec::new();
        let remap = self
            .positions
            .iter()
            .map(|p| {
                // +0.0 makes -0.0 and 0.0 one key.
                let key = p.map(|c| (c + 0.0).to_bits());
                *at.entry(key).or_insert_with(|| {
                    positions.push(*p);
                    positions.len() as u32 - 1
                })
            })
            .collect();
        (positions, remap)
    }

    /// The triangles over [`Self::welded_positions`]'s vertices, those that
    /// welding collapses (two corners at one point) left out.
    pub fn welded_triangles(&self, remap: &[u32]) -> Vec<[u32; 3]> {
        self.indices.as_chunks::<3>().0.iter().map(|t| t.map(|i| remap[i as usize])).filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]).collect()
    }

    /// Polyline of edge `i` (empty for degenerated edges and seams, which
    /// Onshape has not: see [`Shape::shown_topology`]).
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
    /// BRepMesh alone (exports: [`tessellate_export`]); else noshape's display mesher takes the faces it can.
    #[serde(default)]
    pub exact: bool,
    /// The shortest edge segment (model units; 0: none): an STL's Minimum facet width.
    #[serde(default)]
    pub min_size: f64,
}

impl Default for MeshOptions {
    fn default() -> Self {
        MeshOptions { linear_deflection: 0.1, angular_deflection: 0.5, relative: false, parallel: true, exact: false, min_size: 0.0 }
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
        &MeshOptions { linear_deflection, angular_deflection, relative: false, parallel, exact: false, min_size: 0.0 },
    )
}

/// [`tessellate`] with full options.
pub fn tessellate_with(shape: &Shape, o: &MeshOptions) -> Result<Tessellation> {
    let mut t = tessellate_raw(shape, o)?;
    tidy_triangles(&mut t);
    Ok(t)
}

/// An export mesh (STL, OBJ, 3MF…): a closed surface, each edge used once in each direction. Not the display's
/// [`tidy_triangles`] (it turns a sliver by its corners' normals, against its neighbours at a crease, and drops one
/// without area, opening its edges: lead 11-08 P1, c2-hunting-knife, learning-center-michael); the faces a mesher
/// folded re-triangulated (a pole's pinched triangles dropped first), then a triangle wound against its neighbours turned.
pub fn tessellate_closed(shape: &Shape, linear_deflection: f64, angular_deflection: f64) -> Result<Tessellation> {
    let mut t = tessellate_raw(shape, &MeshOptions { linear_deflection, angular_deflection, relative: false, parallel: true, exact: false, min_size: 0.0 })?;
    crate::mesh_repair::drop_pinched_triangles(&mut t);
    crate::mesh_repair::repair_folded_faces(shape, &mut t);
    crate::mesh_repair::orient_by_neighbours(&mut t);
    Ok(t)
}

/// An export mesh at Onshape's facet tolerances (the STL presets, live 2319: deflections under a micrometre in a file
/// in metres): as [`tessellate_closed`], by BRepMesh alone on a copy 1000× up (OCCT's precision and the shape's own
/// tolerances sit at 1e-7 m), then back to size. `min_size`: the shortest edge segment (an STL's Minimum facet width).
pub fn tessellate_export(shape: &Shape, linear_deflection: f64, angular_deflection: f64, min_size: f64) -> Result<Tessellation> {
    const UP: f64 = 1000.0;
    let big = shape.transformed(&crate::Transform::Scale { center: glam::DVec3::ZERO, factor: UP })?;
    let mut t = tessellate_raw(&big, &MeshOptions { linear_deflection: linear_deflection * UP, angular_deflection, relative: false, parallel: true, exact: true, min_size: min_size * UP })?;
    crate::mesh_repair::drop_pinched_triangles(&mut t);
    crate::mesh_repair::repair_folded_faces(&big, &mut t);
    crate::mesh_repair::orient_by_neighbours(&mut t);
    // (Divided in f64: the f32 a mesh at size would have, 20.0 → 0.02 exactly as written.)
    for p in t.positions.iter_mut().chain(t.edge_points.iter_mut()).chain(t.vertices.iter_mut()) {
        for c in p.iter_mut() {
            *c = (*c as f64 / UP) as f32;
        }
    }
    Ok(t)
}

fn tessellate_raw(shape: &Shape, o: &MeshOptions) -> Result<Tessellation> {
    if shape.is_null() {
        return Err(KernelError::invalid("cannot tessellate a null shape"));
    }
    let m = ck(ffi::mesh_prepare(shape.raw(), o.linear_deflection, o.angular_deflection, o.relative, o.parallel, o.exact, o.min_size))?;
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
        planar_fallbacks: c.planar_fallbacks,
        rust_faces: c.rust_faces,
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

/// Each triangle wound counter-clockwise about its corners' normals (the outward side), and none without area. The
/// viewport culls a closed solid's back faces, as live does, so a triangle a mesher wound the other way (BRepMesh's,
/// or a sliver of ours turned over on its grid) left a hole there showing the solid's inside, dark (P0 11-07: dark
/// wedges on a flat face). Returns how many were turned and how many dropped.
pub fn tidy_triangles(t: &mut Tessellation) -> (usize, usize) {
    let v = |a: &[[f32; 3]], i: u32| a.get(i as usize).map_or(glam::DVec3::ZERO, |p| glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
    // The faces' ranges in order, covering the indices: kept triangles packed and the ranges moved with them.
    let mut at = 0;
    let packed = t.faces.iter().all(|f| {
        let ok = f.first_index == at && f.index_count % 3 == 0;
        at += f.index_count;
        ok
    }) && at as usize == t.indices.len();
    let (mut turned, mut dropped) = (0, 0);
    let mut keep = |tri: &mut [u32; 3]| {
        let (a, b, c) = (v(&t.positions, tri[0]), v(&t.positions, tri[1]), v(&t.positions, tri[2]));
        let cross = (b - a).cross(c - a);
        let longest = (b - a).length_squared().max((c - b).length_squared()).max((a - c).length_squared());
        if !(cross.length() > 1e-9 * longest) {
            dropped += 1;
            return false;
        }
        if cross.dot(v(&t.normals, tri[0]) + v(&t.normals, tri[1]) + v(&t.normals, tri[2])) < 0.0 {
            tri.swap(1, 2);
            turned += 1;
        }
        true
    };
    if !packed {
        for tri in t.indices.as_chunks_mut::<3>().0 {
            keep(tri);
        }
        return (turned, 0);
    }
    let mut out: Vec<u32> = Vec::with_capacity(t.indices.len());
    for f in &mut t.faces {
        let start = out.len();
        for tri in t.indices[f.first_index as usize..(f.first_index + f.index_count) as usize].as_chunks::<3>().0 {
            let mut tri = *tri;
            if keep(&mut tri) {
                out.extend(tri);
            }
        }
        (f.first_index, f.index_count) = (start as u32, (out.len() - start) as u32);
    }
    t.indices = out;
    (turned, dropped)
}
