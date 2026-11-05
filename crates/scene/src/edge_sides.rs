//! What the renderer needs to leave out a solid's edge segments that lie behind the solid itself
//! ([`noshape_render::scene::EdgeSides`]): each segment's two facet normals, the clearance around it (how far the
//! solid's other faces are) and whether a side curves away along it.

use glam::Vec3;
use noshape_model::display::MeshBuffers;
use noshape_render::scene::{EdgeFacetNormals, EdgeSides};
use std::collections::HashMap;

/// Each edge segment's sides (`[edge][segment]`): the two triangles of two different faces whose side is the segment
/// (same end points, bit for bit), each normal turned outward like its vertex normals, and the clearance to every
/// other face's triangles; `None` where there aren't exactly two (an open sheet's border, a mesh that doesn't share
/// the edge's points).
pub fn edge_facet_normals(m: &MeshBuffers) -> EdgeFacetNormals {
    type Key = ([u32; 3], [u32; 3]);
    let key = |a: [f32; 3], b: [f32; 3]| -> Key {
        let (ka, kb) = (a.map(f32::to_bits), b.map(f32::to_bits));
        if ka <= kb { (ka, kb) } else { (kb, ka) }
    };
    let wanted: std::collections::HashSet<Key> = m.edges.iter().flat_map(|pl| pl.windows(2).map(|w| key(w[0], w[1]))).collect();
    let mut sides: HashMap<Key, Vec<(Vec3, u32)>> = HashMap::new();
    let mut tris: Vec<([Vec3; 3], u32)> = Vec::new();
    let p = |i: u32| m.positions.get(i as usize).copied();
    for (fi, &(start, count)) in m.face_ranges.iter().enumerate() {
        let Some(face) = m.indices.get(start as usize..(start + count) as usize) else { continue };
        for t in face.as_chunks::<3>().0 {
            let (Some(a), Some(b), Some(c)) = (p(t[0]), p(t[1]), p(t[2])) else { continue };
            let (va, vb, vc) = (Vec3::from(a), Vec3::from(b), Vec3::from(c));
            tris.push(([va, vb, vc], fi as u32));
            let mut n = (vb - va).cross(vc - va).normalize_or_zero();
            if n == Vec3::ZERO {
                continue;
            }
            let shading: Vec3 = t.iter().filter_map(|&i| m.normals.get(i as usize)).map(|v| Vec3::from(*v)).sum();
            if n.dot(shading) < 0.0 {
                n = -n;
            }
            for (x, y) in [(a, b), (b, c), (c, a)] {
                let k = key(x, y);
                if wanted.contains(&k) {
                    sides.entry(k).or_default().push((n, fi as u32));
                }
            }
        }
    }
    let grid = TriGrid::new(tris);
    let mut seen = vec![0u32; grid.tris.len()];
    let mut query = 0u32;
    m.edges
        .iter()
        .map(|pl| {
            (0..pl.len().saturating_sub(1))
                .map(|si| {
                    let (a, b) = (pl[si], pl[si + 1]);
                    let &[(n0, f0), (n1, f1)] = sides.get(&key(a, b)).map(Vec::as_slice)? else { return None };
                    if f0 == f1 {
                        return None;
                    }
                    let (normals, faces) = if f0 < f1 { ([n0, n1], [f0, f1]) } else { ([n1, n0], [f1, f0]) };
                    query += 1;
                    let mut clearance = grid.clearance(Vec3::from(a), Vec3::from(b), faces, &mut seen, query);
                    let mut convex = false;
                    for n in normals {
                        if let Some(r) = convex_radius(pl, si, n) {
                            clearance = clearance.min(r);
                            convex = true;
                        }
                    }
                    Some(EdgeSides { normals, clearance, convex })
                })
                .collect()
        })
        .collect()
}

/// The radius at which a side with facet normal `n` curves away from the viewer along segment `si` of `pl` (it
/// bends against `n` at either end point), if it does.
fn convex_radius(pl: &[[f32; 3]], si: usize, n: Vec3) -> Option<f32> {
    let closed = pl.len() > 3 && pl.first() == pl.last();
    let at = |i: isize| -> Option<Vec3> {
        let last = pl.len() as isize - 1;
        let j = if closed { i.rem_euclid(last) } else { i };
        (0..=last).contains(&j).then(|| Vec3::from(pl[j as usize]))
    };
    let mut radius: Option<f32> = None;
    for i in [si as isize, si as isize + 1] {
        let (Some(p), Some(q), Some(r)) = (at(i - 1), at(i), at(i + 1)) else { continue };
        let (u, v) = (q - p, r - q);
        let len = u.length() + v.length();
        if len <= 0.0 {
            continue;
        }
        // The curvature vector at q: the turn of the unit direction over the half lengths.
        let k = 2.0 * (v.normalize_or_zero() - u.normalize_or_zero()) / len;
        let bend = -k.dot(n);
        if bend > 1e-9 {
            radius = Some(radius.map_or(1.0 / bend, |x| x.min(1.0 / bend)));
        }
    }
    radius
}

/// A body's triangles in a uniform grid, for the distance from a segment to the nearest triangle of other faces.
struct TriGrid {
    tris: Vec<([Vec3; 3], u32)>,
    cells: HashMap<[i32; 3], Vec<u32>>,
    lo: Vec3,
    cell: f32,
    /// Distances past this say nothing more (the renderer's largest clearance code).
    cap: f32,
}

impl TriGrid {
    fn new(tris: Vec<([Vec3; 3], u32)>) -> Self {
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for (t, _) in &tris {
            for p in t {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
        }
        let diag = (hi - lo).length();
        let cell = (diag / 48.0).max(1e-6);
        let mut cells: HashMap<[i32; 3], Vec<u32>> = HashMap::new();
        let ix = |p: Vec3| ((p - lo) / cell).floor().as_ivec3().to_array();
        for (i, (t, _)) in tris.iter().enumerate() {
            let (a, b) = (ix(t[0].min(t[1]).min(t[2])), ix(t[0].max(t[1]).max(t[2])));
            for x in a[0]..=b[0] {
                for y in a[1]..=b[1] {
                    for z in a[2]..=b[2] {
                        cells.entry([x, y, z]).or_default().push(i as u32);
                    }
                }
            }
        }
        Self { tris, cells, lo, cell, cap: 0.25f32.min(diag.max(1e-6)) }
    }

    /// The distance from segment `a`–`b` to the nearest triangle of a face other than `faces`, up to the cap
    /// (`seen[triangle] == query`: already measured in this query).
    fn clearance(&self, a: Vec3, b: Vec3, faces: [u32; 2], seen: &mut [u32], query: u32) -> f32 {
        let mut best = f32::INFINITY;
        let mut reach = self.cell;
        loop {
            let ix = |p: Vec3| ((p - self.lo) / self.cell).floor().as_ivec3().to_array();
            let (lo, hi) = (ix(a.min(b) - Vec3::splat(reach)), ix(a.max(b) + Vec3::splat(reach)));
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        for &ti in self.cells.get(&[x, y, z]).map(Vec::as_slice).unwrap_or_default() {
                            let (t, f) = &self.tris[ti as usize];
                            if faces.contains(f) || seen[ti as usize] == query {
                                continue;
                            }
                            seen[ti as usize] = query;
                            best = best.min(segment_triangle_distance(a, b, t));
                        }
                    }
                }
            }
            if best <= reach || reach >= self.cap {
                return best.min(self.cap);
            }
            reach *= 2.0;
        }
    }
}

/// The distance between segment `p`–`q` and triangle `t`.
fn segment_triangle_distance(p: Vec3, q: Vec3, t: &[Vec3; 3]) -> f32 {
    if segment_crosses_triangle(p, q, t) {
        return 0.0;
    }
    let mut d = point_triangle_distance(p, t).min(point_triangle_distance(q, t));
    for (u, v) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
        d = d.min(segment_segment_distance(p, q, u, v));
    }
    d
}

fn segment_crosses_triangle(p: Vec3, q: Vec3, t: &[Vec3; 3]) -> bool {
    let (e1, e2, dir) = (t[1] - t[0], t[2] - t[0], q - p);
    let h = dir.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-20 {
        return false;
    }
    let s = p - t[0];
    let u = s.dot(h) / det;
    let qv = s.cross(e1);
    let v = dir.dot(qv) / det;
    let w = e2.dot(qv) / det;
    (0.0..=1.0).contains(&u) && v >= 0.0 && u + v <= 1.0 && (0.0..=1.0).contains(&w)
}

/// Ericson, Real-Time Collision Detection 5.1.5: the closest point of a triangle.
fn point_triangle_distance(p: Vec3, t: &[Vec3; 3]) -> f32 {
    let [a, b, c] = *t;
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return p.distance(a);
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return p.distance(b);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return p.distance(a + ab * (d1 / (d1 - d3)));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return p.distance(c);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return p.distance(a + ac * (d2 / (d2 - d6)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return p.distance(b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6))));
    }
    let denom = 1.0 / (va + vb + vc);
    p.distance(a + ab * (vb * denom) + ac * (vc * denom))
}

/// Ericson 5.1.9: the distance between two segments.
fn segment_segment_distance(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> f32 {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let eps = 1e-20;
    let (s, t) = if a <= eps && e <= eps {
        (0.0, 0.0)
    } else if a <= eps {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= eps {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s = if denom > eps { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };
    (p1 + d1 * s).distance(p2 + d2 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_between_segments_and_triangles() {
        let t = [Vec3::ZERO, Vec3::X, Vec3::Y];
        // Above the triangle's inside, crossing it, beside its long side, past a corner.
        assert!((segment_triangle_distance(Vec3::new(0.2, 0.2, 0.5), Vec3::new(0.3, 0.2, 0.5), &t) - 0.5).abs() < 1e-6);
        assert_eq!(segment_triangle_distance(Vec3::new(0.2, 0.2, -1.0), Vec3::new(0.2, 0.2, 1.0), &t), 0.0);
        let d = segment_triangle_distance(Vec3::new(1.0, 1.0, -1.0), Vec3::new(1.0, 1.0, 1.0), &t);
        assert!((d - 0.5f32.sqrt()).abs() < 1e-6, "{d}");
        assert!((segment_triangle_distance(Vec3::new(-1.0, -1.0, 0.0), Vec3::new(-2.0, -1.0, 0.0), &t) - 2f32.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn a_boss_rim_curves_away_a_hole_rim_does_not() {
        // A circle in z = 0 around the z axis, with the side's outward normal at one segment: radial outward is a
        // boss's wall (convex along the rim), radial inward a hole's.
        let r = 0.01f32;
        let pl: Vec<[f32; 3]> = (0..=24).map(|i| {
            let a = i as f32 / 24.0 * std::f32::consts::TAU;
            [r * a.cos(), r * a.sin(), 0.0]
        }).collect();
        let mid = (Vec3::from(pl[3]) + Vec3::from(pl[4])) * 0.5;
        let out = Vec3::new(mid.x, mid.y, 0.0).normalize();
        let radius = convex_radius(&pl, 3, out).unwrap();
        assert!((radius - r).abs() < 0.02 * r, "{radius}");
        assert_eq!(convex_radius(&pl, 3, -out), None);
        assert_eq!(convex_radius(&pl, 3, Vec3::Z), None);
    }
}
