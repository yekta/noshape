//! The kernel side of noshape's own display mesher (crates/brep `mesh`): [`brep_mesh`], which cpp/mesh_rust.cpp
//! calls with a body's fresh faces in flat arrays (ffi `MeshIn`) and turns the result (`MeshOut`) into
//! triangulations and edge polygons.

use crate::ffi::ffi::MeshOut;
use glam::DVec3;
use std::mem::MaybeUninit;
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

static OFF_EVERYWHERE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// [`set_rust_mesh`] for every thread (a regeneration meshes its bodies on a pool): tests comparing whole
/// documents meshed both ways, in a test binary whose other tests don't count the faces it takes.
pub fn set_rust_mesh_everywhere(on: bool) {
    OFF_EVERYWHERE.store(!on, std::sync::atomic::Ordering::Relaxed);
}

/// Whether this thread meshes with it.
pub(crate) fn brep_mesh_on() -> bool {
    !OFF.with(|o| o.get()) && !OFF_EVERYWHERE.load(std::sync::atomic::Ordering::Relaxed)
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
    // A panic here would abort the process (it can't cross the bridge): declined instead, BRepMesh meshes every face
    // (MESH-EMPTY-LOOP-PANIC hung the app on open).
    let mut made = MeshOut::default();
    let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mesh_into(vertices, edges, curves, surfaces, face_loops, loop_uses, uses, linear, angular, &mut made))).unwrap_or_else(|_| {
        log::error!("noshape's mesher panicked: the body goes to BRepMesh");
        false
    });
    if ok {
        *out = made;
    }
    ok
}

#[allow(clippy::too_many_arguments)]
fn mesh_into(vertices: &[f64], edges: &[u32], curves: &[f64], surfaces: &[f64], face_loops: &[u32], loop_uses: &[u32], uses: &[u32], linear: f64, angular: f64, out: &mut MeshOut) -> bool {
    let t = std::time::Instant::now();
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
                2 => Some(Curve::Degenerate),
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
            4 => MeshSurface::Sphere { origin: v3(s, 1), axis: v3(s, 4), xdir: v3(s, 7), radius: s[10], direct: s[12] > 0.5 },
            3 => MeshSurface::Torus { origin: v3(s, 1), axis: v3(s, 4), xdir: v3(s, 7), radius: s[10], minor: s[11], direct: s[12] > 0.5 },
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
    // Its steps in the kernel's profile (NOSHAPE_KERNEL_PROFILE).
    let note = |name: &str, d: std::time::Duration| crate::ffi::ffi::kernel_profile_note(name, d.as_secs_f64() * 1e3);
    note("mesh.rust.mesh.in", t.elapsed());
    let m = mesh_body(&vertices, &edges, &faces, Deflection { linear: input.linear, angular: input.angular });
    note("mesh.rust.mesh.edges", m.took[0]);
    note("mesh.rust.mesh.faces", m.took[1]);
    let t = std::time::Instant::now();
    // Per face its first node, triangle and use entry; then the faces copied in groups of about as many nodes, on
    // scoped threads for a big body, each into its own part of the arrays (serially it was as long as meshing the
    // faces on live-30's ~60k nodes, writing fresh pages one thread at a time).
    let nf = m.faces.len();
    let (mut n_at, mut t_at, mut u_at) = (Vec::with_capacity(nf + 1), Vec::with_capacity(nf + 1), Vec::with_capacity(nf + 1));
    let (mut nn, mut nt, mut nu) = (0usize, 0usize, 0usize);
    out.face_ok.reserve(nf);
    for f in &m.faces {
        out.face_ok.push(f.is_some() as u8);
        n_at.push(nn);
        t_at.push(nt);
        u_at.push(nu);
        if let Some(f) = f {
            nn += f.points.len();
            nt += f.triangles.len();
            nu += f.edge_nodes.iter().flatten().map(|e| 1 + e.len()).sum::<usize>();
        }
    }
    n_at.push(nn);
    t_at.push(nt);
    u_at.push(nu);
    out.face_nodes.extend(n_at.iter().map(|&n| n as u32));
    out.face_tris.extend(t_at.iter().map(|&t| t as u32));
    out.nodes.reserve(3 * nn);
    out.normals.reserve(3 * nn);
    out.tris.reserve(3 * nt);
    out.use_nodes.reserve(nu);
    let threads = if nn < 20_000 { 1 } else { std::thread::available_parallelism().map_or(1, |n| n.get()).min(8) };
    let mut bounds: Vec<usize> = (0..threads).map(|g| n_at.partition_point(|&n| n < nn * g / threads).min(nf)).collect();
    bounds.push(nf);
    bounds.dedup();
    type Part<'a> = (std::ops::Range<usize>, &'a mut [MaybeUninit<f64>], &'a mut [MaybeUninit<f64>], &'a mut [MaybeUninit<u32>], &'a mut [MaybeUninit<u32>]);
    let (mut nodes, mut normals, mut tris, mut uses) = (&mut out.nodes.spare_capacity_mut()[..3 * nn], &mut out.normals.spare_capacity_mut()[..3 * nn], &mut out.tris.spare_capacity_mut()[..3 * nt], &mut out.use_nodes.spare_capacity_mut()[..nu]);
    let mut parts: Vec<Part> = Vec::new();
    for w in bounds.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (n, rest) = std::mem::take(&mut nodes).split_at_mut(3 * (n_at[b] - n_at[a]));
        nodes = rest;
        let (nm, rest) = std::mem::take(&mut normals).split_at_mut(3 * (n_at[b] - n_at[a]));
        normals = rest;
        let (tr, rest) = std::mem::take(&mut tris).split_at_mut(3 * (t_at[b] - t_at[a]));
        tris = rest;
        let (us, rest) = std::mem::take(&mut uses).split_at_mut(u_at[b] - u_at[a]);
        uses = rest;
        parts.push((a..b, n, nm, tr, us));
    }
    let fill = |(range, nodes, normals, tris, uses): Part| {
        let (mut i, mut k, mut u) = (0, 0, 0);
        for f in m.faces[range].iter().flatten() {
            for (p, n) in f.points.iter().zip(&f.normals) {
                for (j, (a, b)) in [(p.x, n.x), (p.y, n.y), (p.z, n.z)].into_iter().enumerate() {
                    nodes[i + j].write(a);
                    normals[i + j].write(b);
                }
                i += 3;
            }
            for &x in f.triangles.iter().flatten() {
                tris[k].write(x);
                k += 1;
            }
            // Per loop, per use: its node count, then its nodes (face-local).
            for un in f.edge_nodes.iter().flatten() {
                uses[u].write(un.len() as u32);
                u += 1;
                for &x in un {
                    uses[u].write(x);
                    u += 1;
                }
            }
        }
    };
    let mut parts = parts.into_iter();
    let here = parts.next();
    std::thread::scope(|sc| {
        for p in parts {
            sc.spawn(move || fill(p));
        }
        if let Some(p) = here {
            fill(p);
        }
    });
    // SAFETY: every slot up to these lengths was written above (the parts cover the reserved ranges exactly: the
    // sizes they were cut to are the faces' own counts).
    unsafe {
        out.nodes.set_len(out.nodes.len() + 3 * nn);
        out.normals.set_len(out.normals.len() + 3 * nn);
        out.tris.set_len(out.tris.len() + 3 * nt);
        out.use_nodes.set_len(out.use_nodes.len() + nu);
    }
    // Per edge, its nodes' parameters (from its start).
    out.edge_params.push(0);
    for e in &m.edges {
        if let Some(e) = e {
            out.params.extend(e.params.iter().copied());
        }
        out.edge_params.push(out.params.len() as u32);
    }
    note("mesh.rust.mesh.out", t.elapsed());
    true
}
