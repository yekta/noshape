//! Thickness analysis (Onshape: Analysis tools ▸ Thickness analysis): wall
//! thickness at points spread over a part's surface, by the ray method (the
//! distance along the inward normal to the opposite surface) or the rolling
//! ball method (the diameter of the largest sphere inside the part that
//! touches the surface there), and their gradients in percent.
//!
//! Works on the part's display triangles, refined so no triangle side is
//! longer than a target spacing.

use crate::display::MeshBuffers;
use glam::DVec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThicknessMethod {
    RollingBall,
    RollingBallGradient,
    Ray,
    RayGradient,
}

impl ThicknessMethod {
    pub const ALL: [ThicknessMethod; 4] = [ThicknessMethod::RollingBall, ThicknessMethod::RollingBallGradient, ThicknessMethod::Ray, ThicknessMethod::RayGradient];

    /// Onshape's Method menu label (`unit`: the length unit, e.g. "mm").
    pub fn label(self, unit: &str) -> String {
        match self {
            ThicknessMethod::RollingBall => format!("Rolling ball ({unit})"),
            ThicknessMethod::RollingBallGradient => "Rolling ball gradient (%)".into(),
            ThicknessMethod::Ray => format!("Ray ({unit})"),
            ThicknessMethod::RayGradient => "Ray gradient (%)".into(),
        }
    }

    pub fn is_gradient(self) -> bool {
        matches!(self, ThicknessMethod::RollingBallGradient | ThicknessMethod::RayGradient)
    }
}

/// A refined surface mesh: vertices with outward normals and the display
/// face (1-based pick id) each came from.
#[derive(Clone, Debug, Default)]
pub struct SampleMesh {
    pub positions: Vec<DVec3>,
    pub normals: Vec<DVec3>,
    pub faces: Vec<u32>,
    pub indices: Vec<u32>,
}

/// Split every triangle of `m` so no side is longer than `spacing`.
pub fn refine(m: &MeshBuffers, spacing: f64) -> SampleMesh {
    let p3 = |p: [f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
    let mut face_of = vec![0u32; m.positions.len()];
    for (fi, &(start, count)) in m.face_ranges.iter().enumerate() {
        for &ix in m.indices.get(start as usize..(start + count) as usize).unwrap_or(&[]) {
            if let Some(f) = face_of.get_mut(ix as usize) {
                *f = fi as u32 + 1;
            }
        }
    }
    let mut out = SampleMesh::default();
    for t in m.indices.as_chunks::<3>().0 {
        let [Some(&a), Some(&b), Some(&c)] = t.map(|i| m.positions.get(i as usize)) else { continue };
        let [Some(&na), Some(&nb), Some(&nc)] = t.map(|i| m.normals.get(i as usize)) else { continue };
        let (a, b, c, na, nb, nc) = (p3(a), p3(b), p3(c), p3(na), p3(nb), p3(nc));
        let longest = a.distance(b).max(b.distance(c)).max(c.distance(a));
        let k = ((longest / spacing.max(1e-12)).ceil() as usize).clamp(1, 64);
        let face = face_of[t[0] as usize];
        let base = out.positions.len() as u32;
        // Barycentric grid: row i has k - i + 1 points.
        let row_start = |i: usize| -> u32 { (0..i).map(|r| (k - r + 1) as u32).sum() };
        for i in 0..=k {
            for j in 0..=(k - i) {
                let (u, v) = (i as f64 / k as f64, j as f64 / k as f64);
                let w = 1.0 - u - v;
                out.positions.push(a * w + b * u + c * v);
                out.normals.push((na * w + nb * u + nc * v).normalize_or_zero());
                out.faces.push(face);
            }
        }
        for i in 0..k {
            for j in 0..(k - i) {
                let p = base + row_start(i) + j as u32;
                let q = base + row_start(i + 1) + j as u32;
                out.indices.extend_from_slice(&[p, q, p + 1]);
                if j + 1 < k - i {
                    out.indices.extend_from_slice(&[p + 1, q, q + 1]);
                }
            }
        }
    }
    out
}

/// A bounding volume hierarchy over triangles for ray casts.
struct Bvh {
    nodes: Vec<BvhNode>,
    tris: Vec<[DVec3; 3]>,
}

struct BvhNode {
    lo: DVec3,
    hi: DVec3,
    /// Leaf: triangles `first..first + count`; inner (count 0): children.
    first: u32,
    count: u32,
    children: [u32; 2],
}

impl Bvh {
    fn new(mut tris: Vec<[DVec3; 3]>) -> Self {
        let mut nodes = Vec::new();
        let n = tris.len();
        Self::build(&mut tris, 0, n, &mut nodes);
        Self { nodes, tris }
    }

    fn build(tris: &mut [[DVec3; 3]], start: usize, end: usize, nodes: &mut Vec<BvhNode>) -> u32 {
        let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
        for t in &tris[start..end] {
            for p in t {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
        }
        let me = nodes.len();
        nodes.push(BvhNode { lo, hi, first: start as u32, count: (end - start) as u32, children: [0; 2] });
        if end - start > 4 {
            let axis = (hi - lo).max_position();
            let centroid = |t: &[DVec3; 3]| (t[0] + t[1] + t[2])[axis];
            tris[start..end].sort_unstable_by(|a, b| centroid(a).total_cmp(&centroid(b)));
            let mid = (start + end) / 2;
            let left = Self::build(tris, start, mid, nodes);
            let right = Self::build(tris, mid, end, nodes);
            nodes[me].count = 0;
            nodes[me].children = [left, right];
        }
        me as u32
    }

    /// Nearest hit along `dir` (unit) from `origin` with t > `t_min`.
    fn cast(&self, origin: DVec3, dir: DVec3, t_min: f64) -> Option<f64> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = DVec3::ONE / dir;
        let mut best = f64::MAX;
        let mut stack = vec![0u32];
        while let Some(i) = stack.pop() {
            let n = &self.nodes[i as usize];
            let (t1, t2) = ((n.lo - origin) * inv, (n.hi - origin) * inv);
            let (tn, tf) = (t1.min(t2).max_element(), t1.max(t2).min_element());
            if tf < tn.max(0.0) || tn > best {
                continue;
            }
            if n.count > 0 {
                for t in &self.tris[n.first as usize..(n.first + n.count) as usize] {
                    if let Some(d) = ray_triangle(origin, dir, t)
                        && d > t_min
                        && d < best
                    {
                        best = d;
                    }
                }
            } else {
                stack.extend_from_slice(&n.children);
            }
        }
        (best < f64::MAX).then_some(best)
    }
}

/// Möller–Trumbore: distance along `dir` to the triangle, both sides.
fn ray_triangle(o: DVec3, d: DVec3, t: &[DVec3; 3]) -> Option<f64> {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-20 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - t[0];
    let u = s.dot(p) * inv;
    if !(-1e-9..=1.0 + 1e-9).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    Some(e2.dot(q) * inv)
}

/// Ray thickness at every sample: distance along the inward normal to the
/// first other surface (`None` where the ray leaves the part, e.g. an open
/// mesh). `tris`: the part's triangles.
pub fn ray_thickness(mesh: &SampleMesh, tris: Vec<[DVec3; 3]>) -> Vec<Option<f64>> {
    let scale = bounds_size(&mesh.positions).max(1e-9);
    let bvh = Bvh::new(tris);
    mesh.positions.iter().zip(&mesh.normals).map(|(p, n)| if *n == DVec3::ZERO { None } else { bvh.cast(*p, -*n, scale * 1e-7) }).collect()
}

/// Nearest hit of each ray (origin, unit direction) on `tris`, farther
/// than `t_min`.
pub fn cast_rays(tris: Vec<[DVec3; 3]>, rays: &[(DVec3, DVec3)], t_min: f64) -> Vec<Option<f64>> {
    let bvh = Bvh::new(tris);
    rays.iter().map(|(o, d)| bvh.cast(*o, *d, t_min)).collect()
}

fn bounds_size(pts: &[DVec3]) -> f64 {
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    for p in pts {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    if pts.is_empty() { 0.0 } else { (hi - lo).length() }
}

/// Rolling ball thickness at every sample: the diameter of the largest
/// sphere tangent to the surface there that contains no other sample. A
/// sphere of radius r tangent at p (outward normal n) contains q exactly
/// when |q − p|² < 2r·(p − q)·n, so the radius is the smallest
/// |d|² / (2 |d·n|) over samples q on the inner side; the ray thickness
/// bounds it, which limits the search to a ball around p.
pub fn rolling_ball_thickness(mesh: &SampleMesh, ray: &[Option<f64>]) -> Vec<Option<f64>> {
    use std::collections::HashMap;
    let size = bounds_size(&mesh.positions).max(1e-9);
    // Samples bucketed in a coarse grid; only occupied cells are visited,
    // skipping those farther than 2r (which can't shrink the ball, r ≥ |d| / 2).
    let cell = size / 16.0;
    let key = |p: DVec3| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64, (p.z / cell).floor() as i64);
    let mut grid: HashMap<(i64, i64, i64), Vec<u32>> = HashMap::new();
    for (i, p) in mesh.positions.iter().enumerate() {
        grid.entry(key(*p)).or_default().push(i as u32);
    }
    let cells: Vec<(DVec3, DVec3, Vec<u32>)> = grid
        .into_iter()
        .map(|((x, y, z), ids)| {
            let lo = DVec3::new(x as f64, y as f64, z as f64) * cell;
            (lo, lo + DVec3::splat(cell), ids)
        })
        .collect();
    let eps = size * 1e-9;
    mesh.positions
        .iter()
        .zip(&mesh.normals)
        .enumerate()
        .map(|(i, (p, n))| {
            if *n == DVec3::ZERO {
                return None;
            }
            let mut r = ray.get(i).copied().flatten().map_or(size, |t| t * 0.5);
            // Nearest cells first: the ball shrinks early and the rest are skipped.
            let mut near: Vec<(f64, usize)> = cells.iter().enumerate().map(|(k, (lo, hi, _))| (p.clamp(*lo, *hi).distance(*p), k)).filter(|(d, _)| *d <= 2.0 * r).collect();
            near.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
            for (dist, k) in near {
                if dist > 2.0 * r {
                    break;
                }
                for &j in &cells[k].2 {
                    let d = mesh.positions[j as usize] - *p;
                    let dn = d.dot(*n);
                    if dn < -eps {
                        r = r.min(d.length_squared() / (-2.0 * dn));
                    }
                }
            }
            Some(2.0 * r)
        })
        .collect()
}

/// Thickness gradient in percent: the steepest change of thickness per
/// distance along the surface, from each sample to its mesh neighbours.
pub fn gradient(mesh: &SampleMesh, values: &[Option<f64>]) -> Vec<Option<f64>> {
    let mut out = vec![None::<f64>; values.len()];
    for t in mesh.indices.as_chunks::<3>().0 {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (a as usize, b as usize);
            let (Some(Some(va)), Some(Some(vb))) = (values.get(a), values.get(b)) else { continue };
            let l = mesh.positions[a].distance(mesh.positions[b]);
            if l <= 0.0 {
                continue;
            }
            let g = (va - vb).abs() / l * 100.0;
            for i in [a, b] {
                out[i] = Some(out[i].map_or(g, |o| o.max(g)));
            }
        }
    }
    out
}

/// Sample spacing for a part: fine enough to resolve its walls, with
/// about `budget` samples at most.
pub fn spacing_for(m: &MeshBuffers, budget: usize) -> f64 {
    let p3 = |p: [f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
    let area: f64 = m
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .filter_map(|t| {
            let [a, b, c] = t.map(|i| m.positions.get(i as usize).copied().map(p3));
            Some((b? - a?).cross(c? - a?).length() * 0.5)
        })
        .sum();
    (2.0 * area / budget.max(1) as f64).sqrt().max(1e-9)
}

/// Thickness of a part's display mesh by `method`: the refined samples
/// and their values (length, or percent for gradients).
pub fn analyze(m: &MeshBuffers, method: ThicknessMethod, budget: usize) -> (SampleMesh, Vec<Option<f64>>) {
    let p3 = |p: [f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
    let tris: Vec<[DVec3; 3]> = m.indices.as_chunks::<3>().0.iter().filter_map(|t| Some([p3(*m.positions.get(t[0] as usize)?), p3(*m.positions.get(t[1] as usize)?), p3(*m.positions.get(t[2] as usize)?)])).collect();
    let mesh = refine(m, spacing_for(m, budget));
    let ray = ray_thickness(&mesh, tris);
    let values = match method {
        ThicknessMethod::Ray => ray,
        ThicknessMethod::RayGradient => gradient(&mesh, &ray),
        ThicknessMethod::RollingBall => rolling_ball_thickness(&mesh, &ray),
        ThicknessMethod::RollingBallGradient => {
            let rb = rolling_ball_thickness(&mesh, &ray);
            gradient(&mesh, &rb)
        }
    };
    (mesh, values)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4 × 2 × 1 box as display buffers (one face per side, outward normals).
    fn slab() -> MeshBuffers {
        let mut m = MeshBuffers::default();
        let (lo, hi) = (DVec3::ZERO, DVec3::new(4.0, 2.0, 1.0));
        let corner = |i: usize| DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
        // Faces as corner quads (counter-clockwise from outside) with normals.
        let faces = [([0, 2, 3, 1], -DVec3::Z), ([4, 5, 7, 6], DVec3::Z), ([0, 1, 5, 4], -DVec3::Y), ([2, 6, 7, 3], DVec3::Y), ([0, 4, 6, 2], -DVec3::X), ([1, 3, 7, 5], DVec3::X)];
        for (q, n) in faces {
            let base = m.positions.len() as u32;
            for c in q {
                let p = corner(c);
                m.positions.push([p.x as f32, p.y as f32, p.z as f32]);
                m.normals.push([n.x as f32, n.y as f32, n.z as f32]);
            }
            let first = m.indices.len() as u32;
            m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            m.face_ranges.push((first, 6));
        }
        m
    }

    #[test]
    fn refine_keeps_the_surface_and_splits_sides() {
        let m = slab();
        let r = refine(&m, 0.5);
        assert!(r.positions.len() > 100);
        let area = |pos: &[DVec3], idx: &[u32]| -> f64 { idx.as_chunks::<3>().0.iter().map(|t| (pos[t[1] as usize] - pos[t[0] as usize]).cross(pos[t[2] as usize] - pos[t[0] as usize]).length() * 0.5).sum() };
        assert!((area(&r.positions, &r.indices) - 2.0 * (8.0 + 4.0 + 2.0)).abs() < 1e-9);
        for t in r.indices.as_chunks::<3>().0 {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                assert!(r.positions[a as usize].distance(r.positions[b as usize]) <= 0.5 * 2f64.sqrt() + 1e-9);
            }
        }
        // Every sample keeps its face (1-based) and normal.
        assert!(r.faces.iter().all(|f| (1..=6).contains(f)));
    }

    #[test]
    fn ray_and_rolling_ball_thickness_of_a_slab() {
        let m = slab();
        let (mesh, ray) = analyze(&m, ThicknessMethod::Ray, 4000);
        // The middle of the top face: the ray goes straight through the 1 thick slab.
        let mid = |want: DVec3, n: DVec3| (0..mesh.positions.len()).filter(|&i| mesh.normals[i].dot(n) > 0.99).min_by(|&a, &b| mesh.positions[a].distance(want).total_cmp(&mesh.positions[b].distance(want))).unwrap();
        let top = mid(DVec3::new(2.0, 1.0, 1.0), DVec3::Z);
        assert!((ray[top].unwrap() - 1.0).abs() < 1e-9, "{:?}", ray[top]);
        // Through the length of the slab from an end face.
        let end = mid(DVec3::new(4.0, 1.0, 0.5), DVec3::X);
        assert!((ray[end].unwrap() - 4.0).abs() < 1e-9);
        let rb = rolling_ball_thickness(&mesh, &ray);
        // Rolling ball: the slab's thickness in the middle of the top face,
        // shrinking to nothing at the edges.
        assert!((rb[top].unwrap() - 1.0).abs() < 0.05, "{:?}", rb[top]);
        let near_edge = mid(DVec3::new(2.0, 0.0, 1.0), DVec3::Z);
        assert!(rb[near_edge].unwrap() < 0.3, "{:?}", rb[near_edge]);
        // The end face is no thicker than the slab either.
        assert!(rb[end].unwrap() < 1.0 + 0.05);
        // Gradients: zero where the thickness is constant, positive near edges.
        let g = gradient(&mesh, &rb);
        assert!(g[top].unwrap() < 1.0, "{:?}", g[top]);
        assert!(g[near_edge].unwrap() > 50.0, "{:?}", g[near_edge]);
    }
}
