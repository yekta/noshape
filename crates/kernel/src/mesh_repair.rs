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
use std::hash::{BuildHasherDefault, Hasher};

/// Integer keys hashed by a multiply (an export's millions of welded points and edges: SipHash took most of an STL
/// export's repairs, pulley02-maxwell at Fine 21 of 25 s).
#[derive(Default)]
struct Mix(u64);

impl Hasher for Mix {
    fn finish(&self) -> u64 {
        self.0 ^ (self.0 >> 31)
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }
    fn write_u32(&mut self, x: u32) {
        self.write_u64(x as u64);
    }
    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(26) ^ x).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

type Fast = BuildHasherDefault<Mix>;

/// The directed welded edges of triangles `indices`, three per triangle.
fn edge_keys<'a>(indices: &'a [u32], weld: &'a [u32]) -> impl Iterator<Item = u64> + 'a {
    indices.as_chunks::<3>().0.iter().flat_map(move |tri| {
        let w = tri.map(|i| weld[i as usize]);
        (0..3).map(move |k| edge(w[k], w[(k + 1) % 3]))
    })
}

/// A directed welded edge as one key.
fn edge(a: u32, b: u32) -> u64 {
    (a as u64) << 32 | b as u64
}

/// Each triangle wound against its neighbours (an edge it uses the way another triangle does) turned when that makes
/// fewer such edges: a mesher's triangle wound the other way, without turning one the corners' normals mislead
/// about. Returns how many.
pub fn orient_by_neighbours(t: &mut Tessellation) -> usize {
    let weld = weld(t);
    // No edge used twice the same way: nothing to turn (the usual case, told by sorting).
    let mut sorted: Vec<u64> = edge_keys(&t.indices, &weld).collect();
    sorted.sort_unstable();
    if !sorted.windows(2).any(|p| p[0] == p[1]) {
        return 0;
    }
    drop(sorted);
    let mut used: HashMap<u64, u32, Fast> = HashMap::with_capacity_and_hasher(t.indices.len(), Fast::default());
    for k in edge_keys(&t.indices, &weld) {
        *used.entry(k).or_default() += 1;
    }
    let mut turned = 0;
    // A few passes: a turned triangle may settle its neighbour's count.
    for _ in 0..4 {
        let before = turned;
        for tri in t.indices.as_chunks_mut::<3>().0 {
            let w = tri.map(|i| weld[i as usize]);
            let sides = [(w[0], w[1]), (w[1], w[2]), (w[2], w[0])];
            let against = sides.iter().filter(|(a, b)| used.get(&edge(*a, *b)).copied().unwrap_or(0) > 1).count();
            if against == 0 {
                continue;
            }
            let turned_against = sides.iter().filter(|(a, b)| used.get(&edge(*b, *a)).copied().unwrap_or(0) > 0).count();
            if turned_against >= against {
                continue;
            }
            for (a, b) in sides {
                *used.get_mut(&edge(a, b)).expect("counted") -= 1;
                *used.entry(edge(b, a)).or_default() += 1;
            }
            tri.swap(1, 2);
            turned += 1;
        }
        if turned == before {
            break;
        }
    }
    turned
}

/// The triangles with two corners at one point dropped (a pole's: its other two sides are one edge both ways, so the
/// triangles beyond them meet). Returns how many.
pub fn drop_pinched_triangles(t: &mut Tessellation) -> usize {
    let weld = weld(t);
    let mut out = Vec::with_capacity(t.indices.len());
    for r in &mut t.faces {
        let first = out.len() as u32;
        for tri in t.indices[r.first_index as usize..(r.first_index + r.index_count) as usize].as_chunks::<3>().0 {
            let [a, b, c] = tri.map(|i| weld[i as usize]);
            if a != b && b != c && c != a {
                out.extend_from_slice(tri);
            }
        }
        r.first_index = first;
        r.index_count = out.len() as u32 - first;
    }
    let dropped = (t.indices.len() - out.len()) / 3;
    t.indices = out;
    dropped
}

/// Each vertex's first vertex at the same position (the faces' meshes share their edges' points, not indices).
fn weld(t: &Tessellation) -> Vec<u32> {
    let mut id: HashMap<[u32; 3], u32, Fast> = HashMap::with_capacity_and_hasher(t.positions.len(), Fast::default());
    t.positions
        .iter()
        .map(|p| {
            let n = id.len() as u32;
            *id.entry(p.map(f32::to_bits)).or_insert(n)
        })
        .collect()
}

/// Folded faces of `t` (the tessellation of `shape`, its faces in order) re-triangulated; returns how many.
pub fn repair_folded_faces(shape: &Shape, t: &mut Tessellation) -> usize {
    let weld = weld(t);
    let tris_of = |r: &crate::mesh::FaceRange| &t.indices[r.first_index as usize..(r.first_index + r.index_count) as usize];
    // The folded faces: an edge used twice the same way within the face.
    let mut keys: Vec<u64> = Vec::new();
    let folded: Vec<usize> = (0..t.faces.len())
        .filter(|&fi| {
            keys.clear();
            keys.extend(edge_keys(tris_of(&t.faces[fi]), &weld));
            keys.sort_unstable();
            keys.windows(2).any(|p| p[0] == p[1])
        })
        .collect();
    if folded.is_empty() {
        return 0;
    }
    // Each directed welded edge: the faces using it.
    let mut faces_of: HashMap<u64, Vec<usize>, Fast> = HashMap::default();
    for (fi, r) in t.faces.iter().enumerate() {
        for tri in tris_of(r).as_chunks::<3>().0 {
            let w = tri.map(|i| weld[i as usize]);
            for k in 0..3 {
                let f = faces_of.entry(edge(w[k], w[(k + 1) % 3])).or_default();
                if !f.contains(&fi) {
                    f.push(fi);
                }
            }
        }
    }
    let mut fixed: Vec<(usize, Vec<u32>)> = Vec::new();
    for fi in folded {
        let tris = tris_of(&t.faces[fi]);
        let Ok(face) = shape.sub_shape(ShapeType::Face, fi) else { continue };
        let others = |a: u32, b: u32| faces_of.get(&edge(a, b)).is_some_and(|f| f.iter().any(|&g| g != fi));
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
