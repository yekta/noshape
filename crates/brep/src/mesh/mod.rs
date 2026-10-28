//! Display meshes of analytic faces (lead 10-28: our own mesher, BRepMesh only for the faces it declines).
//!
//! A body is given as its edges (each sampled once here, so the two faces on an edge share its nodes and the
//! mesh stays watertight) and faces bounded by loops of those edges. Each face is meshed on its own and either
//! gets a triangulation or is declined (`None`): the kernel meshes the declined ones with BRepMesh. Meshes are in
//! the surface's own orientation (its normal; the kernel turns them for reversed faces), every boundary node a
//! mesh node.
//!
//! Faces taken so far: planes bounded by straight edges (constrained Delaunay of their boundary nodes, as the
//! kernel's planar triangulation).

pub mod cdt;

use crate::{Curve, Use};
use glam::{DVec2, DVec3};

/// A face's surface (the kernel reads it from OCCT's geometry).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshSurface {
    /// The plane through `origin` with unit `normal` (the surface's own, `xdir` × `ydir`) and unit `xdir` in it.
    Plane { origin: DVec3, normal: DVec3, xdir: DVec3 },
}

/// An edge: its curve (`None`: one not meshed here) between vertices `v[0]` (at parameter `t0`) and `v[1]`
/// (at `t1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshEdge {
    pub curve: Option<Curve>,
    pub v: [u32; 2],
    pub t0: f64,
    pub t1: f64,
}

/// A face: its surface and boundary loops, each a cycle of edge uses (an outer loop first or not: the
/// triangulation tells them apart).
#[derive(Clone, Debug, PartialEq)]
pub struct MeshFace {
    pub surface: MeshSurface,
    pub loops: Vec<Vec<Use>>,
}

/// The display's deflections: `linear` the largest chord to surface distance, `angular` the largest turn
/// between a segment and the tangent (radians), as BRepMesh takes them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Deflection {
    pub linear: f64,
    pub angular: f64,
}

/// An edge's nodes: points and their curve parameters, from `v[0]` to `v[1]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdgeNodes {
    pub points: Vec<DVec3>,
    pub params: Vec<f64>,
}

/// A face's triangulation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FaceMesh {
    pub points: Vec<DVec3>,
    pub normals: Vec<DVec3>,
    /// Counter-clockwise about the surface normal.
    pub triangles: Vec<[u32; 3]>,
    /// Per loop, per use: the mesh nodes of the edge's nodes, from `v[0]` to `v[1]` (whatever the use's
    /// direction).
    pub edge_nodes: Vec<Vec<Vec<u32>>>,
}

/// The meshed body: nodes of the edges the meshed faces use, a mesh or `None` (declined) per face.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BodyMesh {
    pub edges: Vec<Option<EdgeNodes>>,
    pub faces: Vec<Option<FaceMesh>>,
}

/// The nodes of `edge` under `defl`, if it is a curve meshed here.
pub fn edge_nodes(vertices: &[DVec3], edge: &MeshEdge, _defl: Deflection) -> Option<EdgeNodes> {
    let (a, b) = (*vertices.get(edge.v[0] as usize)?, *vertices.get(edge.v[1] as usize)?);
    match edge.curve {
        Some(Curve::Line) => Some(EdgeNodes { points: vec![a, b], params: vec![edge.t0, edge.t1] }),
        _ => None,
    }
}

/// Meshes the faces of a body (`vertices`, `edges`, `faces`; `None`: a face not meshed here) that are meshed
/// here.
pub fn mesh_body(vertices: &[DVec3], edges: &[MeshEdge], faces: &[Option<MeshFace>], defl: Deflection) -> BodyMesh {
    let mut out = BodyMesh { edges: vec![None; edges.len()], faces: Vec::with_capacity(faces.len()) };
    let mut done = vec![false; edges.len()];
    for f in faces {
        let mut nodes = |e: u32| -> Option<EdgeNodes> {
            let e = e as usize;
            if !done[e] {
                done[e] = true;
                out.edges[e] = edge_nodes(vertices, edges.get(e)?, defl);
            }
            out.edges[e].clone()
        };
        out.faces.push(f.as_ref().and_then(|f| mesh_face(vertices, edges, f, &mut nodes)));
    }
    out
}

/// One face, its edges' nodes from `nodes`.
fn mesh_face(vertices: &[DVec3], edges: &[MeshEdge], face: &MeshFace, nodes: &mut dyn FnMut(u32) -> Option<EdgeNodes>) -> Option<FaceMesh> {
    match face.surface {
        MeshSurface::Plane { origin, normal, xdir } => mesh_plane(vertices, edges, face, origin, normal, xdir, nodes),
    }
}

/// A loop's nodes in order round it (each shared vertex once), and per use the indices of its edge's nodes
/// (from `v[0]` to `v[1]`) into that list. `None` when the uses don't join end to start.
fn loop_nodes(edges: &[MeshEdge], uses: &[Use], nodes: &mut dyn FnMut(u32) -> Option<EdgeNodes>) -> Option<(Vec<DVec3>, Vec<Vec<usize>>)> {
    let ends = |u: &Use| -> Option<(u32, u32)> {
        let e = edges.get(u.edge as usize)?;
        Some(if u.reversed { (e.v[1], e.v[0]) } else { (e.v[0], e.v[1]) })
    };
    // In order round the loop (wires needn't list their edges so): each use's end the next one's start; a
    // vertex two uses start from (a loop touching itself) is ambiguous.
    let mut order = Vec::with_capacity(uses.len());
    let mut left: Vec<usize> = (0..uses.len()).collect();
    let mut at = 0;
    while !left.is_empty() {
        let i = left.remove(at);
        order.push(i);
        let end = ends(&uses[i])?.1;
        let next: Vec<usize> = (0..left.len()).filter(|&k| ends(&uses[left[k]]).is_some_and(|e| e.0 == end)).collect();
        match (next.as_slice(), left.is_empty()) {
            ([k], _) => at = *k,
            ([], true) => {}
            _ => return None,
        }
    }
    if ends(&uses[*order.last()?])?.1 != ends(&uses[order[0]])?.0 {
        return None;
    }
    let mut pts: Vec<DVec3> = Vec::new();
    // Per use (in `uses`' order): its first node's place round the loop, its node count, direction.
    let mut spans: Vec<(usize, usize, bool)> = vec![(0, 0, false); uses.len()];
    for &k in &order {
        let u = &uses[k];
        let n = nodes(u.edge)?;
        let count = n.points.len();
        if count < 2 {
            return None;
        }
        spans[k] = (pts.len(), count, u.reversed);
        // Along the use; its last node is the next use's first.
        let along: Box<dyn Iterator<Item = &DVec3>> = if u.reversed { Box::new(n.points.iter().rev()) } else { Box::new(n.points.iter()) };
        pts.extend(along.take(count - 1));
    }
    let total = pts.len();
    let per_use = spans
        .into_iter()
        .map(|(first, count, reversed)| {
            // Node j along the use sits at first + j round the loop; listed from v[0] to v[1].
            let along: Vec<usize> = (0..count).map(|j| (first + j) % total).collect();
            if reversed { along.into_iter().rev().collect() } else { along }
        })
        .collect();
    Some((pts, per_use))
}

#[allow(clippy::too_many_arguments)]
fn mesh_plane(vertices: &[DVec3], edges: &[MeshEdge], face: &MeshFace, origin: DVec3, normal: DVec3, xdir: DVec3, nodes: &mut dyn FnMut(u32) -> Option<EdgeNodes>) -> Option<FaceMesh> {
    let _ = vertices;
    let ydir = normal.cross(xdir);
    let mut points: Vec<DVec3> = Vec::new();
    let mut rings: Vec<Vec<usize>> = Vec::with_capacity(face.loops.len());
    let mut edge_nodes = Vec::with_capacity(face.loops.len());
    for l in &face.loops {
        let (pts, per_use) = loop_nodes(edges, l, nodes)?;
        let base = points.len();
        rings.push((base..base + pts.len()).collect());
        edge_nodes.push(per_use.into_iter().map(|idx| idx.into_iter().map(|i| (base + i) as u32).collect()).collect());
        points.extend(pts);
    }
    let uv: Vec<cdt::Pt> = points
        .iter()
        .map(|p| {
            let d = *p - origin;
            let q = DVec2::new(d.dot(xdir), d.dot(ydir));
            cdt::Pt { x: q.x, y: q.y }
        })
        .collect();
    let triangles = cdt::triangulate(&uv, &rings)?;
    Some(FaceMesh { normals: vec![normal; points.len()], points, triangles, edge_nodes })
}

#[cfg(test)]
mod tests;
