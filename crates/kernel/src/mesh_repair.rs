//! Faces whose triangles fold over one another, re-triangulated. BRepMesh folds some high-degree B-spline strips
//! (lead 11-08 P1, dx's exchange suite: cf-sheet-metal-model-plus's flange edge, an edge used twice the same way):
//! an exported STL with such a fold isn't a closed, consistently oriented surface. The face's own nodes are
//! triangulated again in its parameter plane (scaled by the surface's first derivatives, near isotropic) with its
//! boundary as constraints, so it stays welded to the faces around it.

use crate::mesh::Tessellation;
use crate::shape::{Shape, ShapeType};
use glam::{DVec2, DVec3};
use noshape_brep::mesh::cdt::{self, Pt};
use std::collections::HashMap;

/// Folded faces of `t` (the tessellation of `shape`, its faces in order) re-triangulated; returns how many.
pub fn repair_folded_faces(shape: &Shape, t: &mut Tessellation) -> usize {
    let mut id = HashMap::new();
    let weld: Vec<u32> = t
        .positions
        .iter()
        .map(|p| {
            let n = id.len() as u32;
            *id.entry(p.map(f32::to_bits)).or_insert(n)
        })
        .collect();
    // Each directed welded edge: the faces using it.
    let mut faces_of: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (fi, r) in t.faces.iter().enumerate() {
        for tri in t.indices[r.first_index as usize..(r.first_index + r.index_count) as usize].as_chunks::<3>().0 {
            let w = tri.map(|i| weld[i as usize]);
            for k in 0..3 {
                let f = faces_of.entry((w[k], w[(k + 1) % 3])).or_default();
                if !f.contains(&fi) {
                    f.push(fi);
                }
            }
        }
    }
    let mut fixed: Vec<(usize, Vec<u32>)> = Vec::new();
    for (fi, r) in t.faces.iter().enumerate() {
        let tris = &t.indices[r.first_index as usize..(r.first_index + r.index_count) as usize];
        let mut used: HashMap<(u32, u32), u32> = HashMap::new();
        for tri in tris.as_chunks::<3>().0 {
            let w = tri.map(|i| weld[i as usize]);
            for k in 0..3 {
                *used.entry((w[k], w[(k + 1) % 3])).or_default() += 1;
            }
        }
        if used.values().all(|&n| n == 1) {
            continue;
        }
        let Ok(face) = shape.sub_shape(ShapeType::Face, fi) else { continue };
        let others = |a: u32, b: u32| faces_of.get(&(a, b)).is_some_and(|f| f.iter().any(|&g| g != fi));
        if let Some(new) = retriangulate(&face, t, tris, &weld, &others) {
            fixed.push((fi, new));
        }
    }
    if fixed.is_empty() {
        return 0;
    }
    // The indices again face by face, the fixed faces' new triangles in place of their old ones.
    let mut indices = Vec::with_capacity(t.indices.len());
    for (fi, r) in t.faces.iter_mut().enumerate() {
        let first = indices.len() as u32;
        match fixed.iter().find(|(f, _)| *f == fi) {
            Some((_, new)) => indices.extend_from_slice(new),
            None => indices.extend_from_slice(&t.indices[r.first_index as usize..(r.first_index + r.index_count) as usize]),
        }
        r.first_index = first;
        r.index_count = indices.len() as u32 - first;
    }
    t.indices = indices;
    fixed.len()
}

/// A face's triangles from its own nodes: its boundary (the triangle sides another face shares) chained into rings,
/// the rest of its nodes inside; `None` when the boundary doesn't chain or the triangulation fails (the face is left).
/// `others(a, b)`: whether another face uses the welded edge a → b.
fn retriangulate(face: &Shape, t: &Tessellation, tris: &[u32], weld: &[u32], others: &dyn Fn(u32, u32) -> bool) -> Option<Vec<u32>> {
    // Boundary sides, by the face's own vertex indices, each once and against the face beyond it (a fold may have
    // turned one round).
    let mut next: HashMap<u32, u32> = HashMap::new();
    for tri in tris.as_chunks::<3>().0 {
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            let (wa, wb) = (weld[a as usize], weld[b as usize]);
            let side = match (others(wb, wa), others(wa, wb)) {
                (true, false) => (a, b),
                (false, true) => (b, a),
                (false, false) => continue,
                (true, true) => return None,
            };
            if next.insert(side.0, side.1).is_some_and(|old| old != side.1) {
                return None;
            }
        }
    }
    let mut rings: Vec<Vec<u32>> = Vec::new();
    let mut left = next.clone();
    while let Some((&start, _)) = left.iter().next() {
        let mut ring = vec![start];
        let mut at = start;
        loop {
            let to = left.remove(&at)?;
            if to == start {
                break;
            }
            ring.push(to);
            at = to;
            if ring.len() > next.len() {
                return None;
            }
        }
        rings.push(ring);
    }
    let mut verts: Vec<u32> = tris.to_vec();
    verts.sort_unstable();
    verts.dedup();
    let local: HashMap<u32, usize> = verts.iter().enumerate().map(|(k, &v)| (v, k)).collect();
    let pos = |v: u32| DVec3::from(t.positions[v as usize].map(f64::from));
    let uv: Vec<DVec2> = verts.iter().map(|&v| face.project_point(pos(v)).map(|p| DVec2::new(p.u, p.v))).collect::<Result<_, _>>().ok()?;
    // Near isotropic: the parameters scaled by the surface's first derivatives at their middle.
    let mid = uv.iter().fold(DVec2::ZERO, |s, p| s + *p) / uv.len().max(1) as f64;
    let d = face.d2_at_uvs(&[mid]).ok()?.into_iter().next()?;
    let (su, sv) = (d[0].length().max(1e-12), d[1].length().max(1e-12));
    let pts: Vec<Pt> = uv.iter().map(|p| Pt { x: p.x * su, y: p.y * sv }).collect();
    let mut on_ring = vec![false; verts.len()];
    let rings: Vec<Vec<usize>> = rings.iter().map(|r| r.iter().map(|v| local[v]).inspect(|&k| on_ring[k] = true).collect()).collect();
    // The outer ring first (the largest), as the triangulation takes them.
    let mut rings = rings;
    let area = |r: &Vec<usize>| r.iter().zip(r.iter().cycle().skip(1)).map(|(&a, &b)| pts[a].x * pts[b].y - pts[b].x * pts[a].y).sum::<f64>().abs();
    rings.sort_by(|a, b| area(b).total_cmp(&area(a)));
    let interior: Vec<usize> = (0..verts.len()).filter(|&k| !on_ring[k]).collect();
    let out = cdt::triangulate_with(&pts, &rings, &interior)?;
    // Wound so each boundary side runs against the face beyond it; kept only when every one is used once that way and
    // no side twice (a periodic face's parameter plane doesn't close its rings: left as it was).
    let mut sides: HashMap<(u32, u32), u32> = HashMap::new();
    for tri in &out {
        for k in 0..3 {
            *sides.entry((verts[tri[k] as usize], verts[tri[(k + 1) % 3] as usize])).or_default() += 1;
        }
    }
    let flip = next.iter().all(|(&a, &b)| sides.contains_key(&(b, a)));
    if sides.values().any(|&n| n > 1) || next.iter().any(|(&a, &b)| sides.get(&if flip { (b, a) } else { (a, b) }) != Some(&1)) {
        return None;
    }
    let new = out.iter().flat_map(|tri| if flip { [tri[0], tri[2], tri[1]] } else { *tri }).map(|k| verts[k as usize]).collect();
    Some(new)
}
