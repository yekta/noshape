//! The kernel side of noshape's own display mesher (crates/brep `mesh`): [`brep_mesh`], which cpp/mesh_rust.cpp
//! calls with a body's fresh faces in flat arrays (ffi `MeshIn`) and turns the result (`MeshOut`) into
//! triangulations and edge polygons.

use crate::ffi::ffi::MeshOut;
use glam::DVec3;
use noshape_brep::mesh::{mesh_body, Deflection, MeshEdge, MeshFace, MeshSurface};
use noshape_brep::{Curve, Use};

fn v3(s: &[f64], i: usize) -> DVec3 {
    DVec3::new(s[i], s[i + 1], s[i + 2])
}

thread_local! {
    static OFF: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether this thread's display meshes use noshape's own mesher (on by default; NOSHAPE_MESH_RUST=0 turns it
/// off everywhere): tests compare it with BRepMesh on one thread while others run.
pub fn set_rust_mesh(on: bool) {
    OFF.with(|o| o.set(!on));
}

/// Whether this thread meshes with it.
pub(crate) fn brep_mesh_on() -> bool {
    !OFF.with(|o| o.get())
}

/// The body's faces (ffi `brep_mesh`'s flat arrays).
struct MeshIn<'a> {
    vertices: &'a [f64],
    edges: &'a [u32],
    curves: &'a [f64],
    surfaces: &'a [f64],
    face_loops: &'a [u32],
    loop_uses: &'a [u32],
    uses: &'a [u32],
    linear: f64,
    angular: f64,
}

/// Meshes the faces noshape's mesher takes into `out`; false when the input is malformed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn brep_mesh(vertices: &[f64], edges: &[u32], curves: &[f64], surfaces: &[f64], face_loops: &[u32], loop_uses: &[u32], uses: &[u32], linear: f64, angular: f64, out: &mut MeshOut) -> bool {
    let input = &MeshIn { vertices, edges, curves, surfaces, face_loops, loop_uses, uses, linear, angular };
    let vertices: Vec<DVec3> = input.vertices.as_chunks::<3>().0.iter().map(|c| DVec3::new(c[0], c[1], c[2])).collect();
    let ne = input.edges.len() / 2;
    let nf = input.face_loops.len().saturating_sub(1);
    if input.curves.len() != 13 * ne || input.surfaces.len() != 13 * nf || input.edges.iter().any(|&v| v as usize >= vertices.len()) {
        return false;
    }
    let edges: Vec<MeshEdge> = (0..ne)
        .map(|i| {
            let c = &input.curves[13 * i..13 * i + 13];
            let curve = match c[0] as u8 {
                0 => Some(Curve::Line),
                1 => Some(Curve::Circle { center: v3(c, 1), normal: v3(c, 4), xdir: v3(c, 7), radius: c[10] }),
                _ => None,
            };
            MeshEdge { curve, v: [input.edges[2 * i], input.edges[2 * i + 1]], t0: c[11], t1: c[12] }
        })
        .collect();
    let mut faces: Vec<MeshFace> = Vec::with_capacity(nf);
    for f in 0..nf {
        let s = &input.surfaces[13 * f..13 * f + 13];
        let surface = match s[0] as u8 {
            0 => MeshSurface::Plane { origin: v3(s, 1), normal: v3(s, 4), xdir: v3(s, 7) },
            1 => MeshSurface::Cylinder { origin: v3(s, 1), axis: v3(s, 4), xdir: v3(s, 7), radius: s[10], direct: s[12] > 0.5 },
            2 => MeshSurface::Cone { origin: v3(s, 1), axis: v3(s, 4), xdir: v3(s, 7), radius: s[10], angle: s[11], direct: s[12] > 0.5 },
            _ => MeshSurface::Other,
        };
        let (l0, l1) = (input.face_loops[f] as usize, input.face_loops[f + 1] as usize);
        if l1 < l0 || l1 + 1 > input.loop_uses.len() {
            return false;
        }
        let mut loops = Vec::with_capacity(l1 - l0);
        for l in l0..l1 {
            let (u0, u1) = (input.loop_uses[l] as usize, input.loop_uses[l + 1] as usize);
            if u1 < u0 || u1 > input.uses.len() {
                return false;
            }
            let uses: Vec<Use> = input.uses[u0..u1].iter().map(|&u| Use { edge: u >> 1, reversed: u & 1 == 1 }).collect();
            if uses.iter().any(|u| u.edge as usize >= ne) {
                return false;
            }
            loops.push(uses);
        }
        faces.push(MeshFace { surface, loops });
    }
    let m = mesh_body(&vertices, &edges, &faces, Deflection { linear: input.linear, angular: input.angular });
    out.face_nodes.push(0);
    out.face_tris.push(0);
    for f in &m.faces {
        out.face_ok.push(f.is_some() as u8);
        if let Some(f) = f {
            for (p, n) in f.points.iter().zip(&f.normals) {
                out.nodes.extend([p.x, p.y, p.z]);
                out.normals.extend([n.x, n.y, n.z]);
            }
            out.tris.extend(f.triangles.iter().flatten().copied());
            // Per loop, per use: its node count, then its nodes (face-local).
            for l in &f.edge_nodes {
                for nodes in l {
                    out.use_nodes.push(nodes.len() as u32);
                    out.use_nodes.extend(nodes.iter().copied());
                }
            }
        }
        out.face_nodes.push((out.nodes.len() / 3) as u32);
        out.face_tris.push((out.tris.len() / 3) as u32);
    }
    // Per edge, its nodes' parameters (from its start).
    out.edge_params.push(0);
    for e in &m.edges {
        if let Some(e) = e {
            out.params.extend(e.params.iter().copied());
        }
        out.edge_params.push(out.params.len() as u32);
    }
    true
}
