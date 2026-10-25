//! Flatten surfaces (Onshape: Analysis tools ▸ Flatten surfaces): a planar
//! sheet with minimum strain energy for one or more contiguous faces. The
//! faces' triangles are welded into one sheet (split along rip edges),
//! projected onto their mean plane and relaxed so every triangle side keeps
//! its 3D length as closely as possible; the remaining stretch of each
//! vertex is its distortion.

use crate::thickness::SampleMesh;
use glam::{DVec2, DVec3};
use std::collections::HashMap;

/// A flattened sheet: 3D vertices (welded), their flat positions, the
/// triangles, each vertex's face (1-based pick id), and the strain at each
/// vertex (positive: stretched / tension, negative: compressed).
#[derive(Clone, Debug, Default)]
pub struct Flattened {
    pub positions: Vec<DVec3>,
    pub flat: Vec<DVec2>,
    pub indices: Vec<u32>,
    pub faces: Vec<u32>,
    /// Face (1-based pick id) of each triangle.
    pub tri_faces: Vec<u32>,
    pub strain: Vec<f64>,
    /// The mean plane the sheet was projected onto: its normal, and the
    /// 3D point and axes of the flat coordinates' origin.
    pub normal: DVec3,
    pub centroid: DVec3,
    pub u: DVec3,
    pub v: DVec3,
}

impl Flattened {
    /// Boundary sides of the sheet (and, with `inner`, sides between two
    /// faces), as vertex index pairs.
    pub fn outline(&self, inner: bool) -> Vec<[u32; 2]> {
        let mut count: HashMap<(u32, u32), (u32, u32)> = HashMap::new();
        for (k, t) in self.indices.as_chunks::<3>().0.iter().enumerate() {
            let face = self.tri_faces.get(k).copied().unwrap_or(0);
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let e = count.entry((a.min(b), a.max(b))).or_insert((0, face));
                e.0 += 1;
                if e.1 != face {
                    e.1 = u32::MAX;
                }
            }
        }
        let mut out: Vec<[u32; 2]> = count.into_iter().filter(|(_, (n, f))| *n == 1 || (inner && *f == u32::MAX)).map(|((a, b), _)| [a, b]).collect();
        out.sort_unstable();
        out
    }

    /// Closed boundary loops (vertex indices, in order) of the sheet, or with
    /// `per_face` of each original face's patch (for a sheet whose faces keep
    /// the inner edges), as (face, loops).
    pub fn loops(&self, per_face: bool) -> Vec<(u32, Vec<Vec<u32>>)> {
        let tris = self.indices.as_chunks::<3>().0;
        let mut groups: Vec<u32> = if per_face { self.tri_faces.clone() } else { vec![0] };
        groups.sort_unstable();
        groups.dedup();
        groups
            .into_iter()
            .map(|g| {
                let mut count: HashMap<(u32, u32), (u32, [u32; 2])> = HashMap::new();
                for (k, t) in tris.iter().enumerate() {
                    if per_face && self.tri_faces.get(k).copied().unwrap_or(0) != g {
                        continue;
                    }
                    for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                        count.entry((a.min(b), a.max(b))).or_insert((0, [a, b])).0 += 1;
                    }
                }
                // Boundary sides keep the triangles' winding, so each loop
                // runs one way; follow them vertex to vertex.
                let mut next: HashMap<u32, Vec<u32>> = HashMap::new();
                for (n, [a, b]) in count.into_values() {
                    if n == 1 {
                        next.entry(a).or_default().push(b);
                    }
                }
                let mut starts: Vec<u32> = next.keys().copied().collect();
                starts.sort_unstable();
                let mut loops = Vec::new();
                for s0 in starts {
                    while let Some(first) = next.get_mut(&s0).and_then(|v| v.pop()) {
                        let mut lp = vec![s0];
                        let mut cur = first;
                        while cur != s0 && lp.len() <= self.flat.len() {
                            lp.push(cur);
                            match next.get_mut(&cur).and_then(|v| v.pop()) {
                                Some(n) => cur = n,
                                None => break,
                            }
                        }
                        if cur == s0 && lp.len() >= 3 {
                            loops.push(lp);
                        }
                    }
                }
                (g, loops)
            })
            .collect()
    }

    /// ASCII STL of the sheet placed at `placed` (3D points, one per vertex), in mm.
    pub fn to_stl(&self, placed: &[DVec3]) -> String {
        let mut out = String::from("solid flattened\n");
        for t in self.indices.as_chunks::<3>().0 {
            let [a, b, c] = t.map(|i| placed[i as usize] * 1000.0);
            let n = (b - a).cross(c - a).normalize_or_zero();
            out += &format!("facet normal {} {} {}\n outer loop\n", n.x, n.y, n.z);
            for p in [a, b, c] {
                out += &format!("  vertex {} {} {}\n", p.x, p.y, p.z);
            }
            out += " endloop\nendfacet\n";
        }
        out + "endsolid flattened\n"
    }

    /// DXF (R12 lines, millimeters) of the flat outline.
    pub fn to_dxf(&self, inner: bool) -> String {
        let mut out = String::from("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n");
        for [a, b] in self.outline(inner) {
            let (p, q) = (self.flat[a as usize] * 1000.0, self.flat[b as usize] * 1000.0);
            out += &format!("0\nLINE\n8\n0\n10\n{}\n20\n{}\n30\n0\n11\n{}\n21\n{}\n31\n0\n", p.x, p.y, q.x, q.y);
        }
        out + "0\nENDSEC\n0\nEOF\n"
    }

    /// SVG (millimeters, y down) of the flat outline.
    pub fn to_svg(&self, inner: bool) -> String {
        let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
        for p in &self.flat {
            lo = lo.min(*p * 1000.0);
            hi = hi.max(*p * 1000.0);
        }
        let size = (hi - lo).max(DVec2::splat(1e-6));
        let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">\n", w = size.x, h = size.y);
        for [a, b] in self.outline(inner) {
            let f = |i: u32| {
                let p = self.flat[i as usize] * 1000.0;
                (p.x - lo.x, hi.y - p.y)
            };
            let ((x1, y1), (x2, y2)) = (f(a), f(b));
            out += &format!("<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"black\" stroke-width=\"0.1\"/>\n");
        }
        out + "</svg>\n"
    }

    /// Area of the sheet in 3D and flat (they match when nothing stretches).
    pub fn areas(&self) -> (f64, f64) {
        let mut a3 = 0.0;
        let mut a2 = 0.0;
        for t in self.indices.as_chunks::<3>().0 {
            let [a, b, c] = t.map(|i| i as usize);
            a3 += (self.positions[b] - self.positions[a]).cross(self.positions[c] - self.positions[a]).length() * 0.5;
            a2 += (self.flat[b] - self.flat[a]).perp_dot(self.flat[c] - self.flat[a]).abs() * 0.5;
        }
        (a3, a2)
    }
}

/// Weld the per-triangle vertices of `mesh` through the triangle sides
/// they share, except sides lying on `rips` (polylines): the sheet opens
/// along a rip.
type Welded = (Vec<DVec3>, Vec<u32>, Vec<u32>, Vec<u32>);

fn weld(mesh: &SampleMesh, rips: &[Vec<DVec3>], tol: f64) -> Welded {
    let on_rip = |p: DVec3| {
        rips.iter().any(|pl| {
            pl.windows(2).any(|w| {
                let d = w[1] - w[0];
                let t = ((p - w[0]).dot(d) / d.length_squared().max(1e-300)).clamp(0.0, 1.0);
                (w[0] + d * t).distance(p) <= tol
            })
        })
    };
    let q = |p: DVec3| ((p.x / tol).round() as i64, (p.y / tol).round() as i64, (p.z / tol).round() as i64);
    let n = mesh.positions.len();
    let mut parent: Vec<u32> = (0..n as u32).collect();
    fn find(parent: &mut [u32], mut i: u32) -> u32 {
        while parent[i as usize] != i {
            parent[i as usize] = parent[parent[i as usize] as usize];
            i = parent[i as usize];
        }
        i
    }
    // Sides by their (quantized) end positions.
    type Key = (i64, i64, i64);
    let mut sides: HashMap<(Key, Key), Vec<(u32, u32)>> = HashMap::new();
    for t in mesh.indices.as_chunks::<3>().0 {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (ka, kb) = (q(mesh.positions[a as usize]), q(mesh.positions[b as usize]));
            let (key, ends) = if ka <= kb { ((ka, kb), (a, b)) } else { ((kb, ka), (b, a)) };
            sides.entry(key).or_default().push(ends);
        }
    }
    for list in sides.values() {
        let Some(&(a0, b0)) = list.first() else { continue };
        if list.len() < 2 {
            continue;
        }
        let (pa, pb) = (mesh.positions[a0 as usize], mesh.positions[b0 as usize]);
        if on_rip(pa) && on_rip(pb) && on_rip((pa + pb) * 0.5) {
            continue;
        }
        for &(a, b) in &list[1..] {
            for (x, y) in [(a0, a), (b0, b)] {
                let (rx, ry) = (find(&mut parent, x), find(&mut parent, y));
                if rx != ry {
                    parent[ry as usize] = rx;
                }
            }
        }
    }
    let mut index_of: HashMap<u32, u32> = HashMap::new();
    let (mut positions, mut faces) = (Vec::new(), Vec::new());
    let remap: Vec<u32> = (0..n as u32)
        .map(|i| {
            let root = find(&mut parent, i);
            *index_of.entry(root).or_insert_with(|| {
                positions.push(mesh.positions[root as usize]);
                faces.push(mesh.faces[root as usize]);
                positions.len() as u32 - 1
            })
        })
        .collect();
    // Drop triangles collapsed by welding; each keeps its face.
    let (mut indices, mut tri_faces) = (Vec::new(), Vec::new());
    for t in mesh.indices.as_chunks::<3>().0 {
        let w = t.map(|i| remap[i as usize]);
        if w[0] != w[1] && w[1] != w[2] && w[2] != w[0] {
            indices.extend_from_slice(&w);
            tri_faces.push(mesh.faces[t[0] as usize]);
        }
    }
    (positions, indices, faces, tri_faces)
}

/// Lay the triangles out flat one by one from a seed, each keeping its 3D
/// shape across the side it shares with an already placed one (exact for
/// developable surfaces, a starting point otherwise).
fn unfold(positions: &[DVec3], indices: &[u32]) -> Vec<DVec2> {
    let tris: Vec<[u32; 3]> = indices.as_chunks::<3>().0.to_vec();
    let mut by_side: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (k, t) in tris.iter().enumerate() {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            by_side.entry((a.min(b), a.max(b))).or_default().push(k);
        }
    }
    let mut flat: Vec<Option<DVec2>> = vec![None; positions.len()];
    let mut done = vec![false; tris.len()];
    // Third corner c of a triangle given flat a, b: on the side away from `away`.
    let place = |pa: DVec2, pb: DVec2, a: DVec3, b: DVec3, c: DVec3, away: Option<DVec2>| -> DVec2 {
        let (ab, ac) = (b - a, c - a);
        let l = ab.length().max(1e-300);
        let x = ac.dot(ab) / l;
        let y = (ac.length_squared() - x * x).max(0.0).sqrt();
        let dir = (pb - pa) / (pb - pa).length().max(1e-300);
        let perp = dir.perp();
        let mut p = pa + dir * x + perp * y;
        if let Some(o) = away
            && (p - pa).perp_dot(pb - pa).signum() == (o - pa).perp_dot(pb - pa).signum()
        {
            p = pa + dir * x - perp * y;
        }
        p
    };
    for seed in 0..tris.len() {
        if done[seed] || tris[seed].iter().any(|v| flat[*v as usize].is_some()) {
            continue;
        }
        let [a, b, c] = tris[seed];
        let (pa, pb) = (DVec2::ZERO, DVec2::new(positions[a as usize].distance(positions[b as usize]), 0.0));
        flat[a as usize] = Some(pa);
        flat[b as usize] = Some(pb);
        flat[c as usize] = Some(place(pa, pb, positions[a as usize], positions[b as usize], positions[c as usize], None));
        done[seed] = true;
        let mut queue = std::collections::VecDeque::from([seed]);
        while let Some(k) = queue.pop_front() {
            let t = tris[k];
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let opposite = t.into_iter().find(|v| *v != a && *v != b).unwrap_or(a);
                for &m in by_side.get(&(a.min(b), a.max(b))).map(Vec::as_slice).unwrap_or(&[]) {
                    if done[m] {
                        continue;
                    }
                    done[m] = true;
                    let c = tris[m].into_iter().find(|v| *v != a && *v != b).unwrap_or(a);
                    if flat[c as usize].is_none()
                        && let (Some(pa), Some(pb)) = (flat[a as usize], flat[b as usize])
                    {
                        flat[c as usize] = Some(place(pa, pb, positions[a as usize], positions[b as usize], positions[c as usize], flat[opposite as usize]));
                    }
                    queue.push_back(m);
                }
            }
        }
    }
    flat.into_iter().map(Option::unwrap_or_default).collect()
}

/// Flatten a sheet of samples (see [`crate::thickness::refine`]) ripped
/// along `rips`: unfolded, then `iterations` of side-length relaxation.
pub fn flatten(mesh: &SampleMesh, rips: &[Vec<DVec3>], iterations: usize) -> Flattened {
    let size = {
        let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
        for p in &mesh.positions {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        (hi - lo).length().max(1e-9)
    };
    let (positions, indices, faces, tri_faces) = weld(mesh, rips, size * 1e-6);
    // Mean plane: the area-weighted normal through the centroid (the Top
    // plane's normal when the faces turn all around, like a whole cylinder).
    let mut normal = DVec3::ZERO;
    let mut centroid = DVec3::ZERO;
    let mut area = 0.0;
    for t in indices.as_chunks::<3>().0 {
        let [a, b, c] = t.map(|i| positions[i as usize]);
        let n = (b - a).cross(c - a);
        normal += n;
        centroid += (a + b + c) / 3.0 * n.length();
        area += n.length();
    }
    let normal = if normal.length() > 0.3 * area { normal.normalize() } else { DVec3::Z };
    let centroid = if area > 0.0 { centroid / area } else { DVec3::ZERO };
    let u = normal.any_orthonormal_vector();
    let v = normal.cross(u);
    let mut flat = unfold(&positions, &indices);
    // Turn (or mirror) the unfolded sheet to best match its projection on
    // the mean plane, so it lies the way the faces do.
    {
        let proj: Vec<DVec2> = positions.iter().map(|p| DVec2::new((*p - centroid).dot(u), (*p - centroid).dot(v))).collect();
        let mean = flat.iter().copied().sum::<DVec2>() / flat.len().max(1) as f64;
        let pmean = proj.iter().copied().sum::<DVec2>() / proj.len().max(1) as f64;
        let best = [false, true]
            .into_iter()
            .map(|mirror| {
                let q: Vec<DVec2> = flat.iter().map(|f| {
                    let d = *f - mean;
                    if mirror { DVec2::new(d.x, -d.y) } else { d }
                }).collect();
                let (mut c, mut s2) = (0.0, 0.0);
                for (a, b) in q.iter().zip(&proj) {
                    let b = *b - pmean;
                    c += a.dot(b);
                    s2 += a.perp_dot(b);
                }
                let rot = DVec2::from_angle(s2.atan2(c));
                let q: Vec<DVec2> = q.iter().map(|a| rot.rotate(*a) + pmean).collect();
                let err: f64 = q.iter().zip(&proj).map(|(a, b)| a.distance_squared(*b)).sum();
                (err, q)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, q)) = best {
            flat = q;
        }
    }
    // Sides and their 3D lengths.
    let mut sides: Vec<(u32, u32, f64)> = Vec::new();
    {
        let mut seen = std::collections::HashSet::new();
        for t in indices.as_chunks::<3>().0 {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                if seen.insert((a.min(b), a.max(b))) {
                    sides.push((a, b, positions[a as usize].distance(positions[b as usize])));
                }
            }
        }
    }
    // Relax: move both ends of each side toward its 3D length.
    for _ in 0..iterations {
        for &(a, b, l) in &sides {
            let (pa, pb) = (flat[a as usize], flat[b as usize]);
            let d = pb - pa;
            let len = d.length();
            if len <= 1e-300 {
                continue;
            }
            let corr = d * ((len - l) / len * 0.5);
            flat[a as usize] += corr;
            flat[b as usize] -= corr;
        }
    }
    // Strain: mean relative stretch of each vertex's sides.
    let mut sum = vec![0.0; positions.len()];
    let mut n = vec![0u32; positions.len()];
    for &(a, b, l) in &sides {
        if l <= 0.0 {
            continue;
        }
        let s = (flat[a as usize].distance(flat[b as usize]) - l) / l;
        for i in [a, b] {
            sum[i as usize] += s;
            n[i as usize] += 1;
        }
    }
    let strain = sum.iter().zip(&n).map(|(s, n)| if *n > 0 { s / *n as f64 } else { 0.0 }).collect();
    // Center the sheet on its flat centroid.
    let mean = flat.iter().copied().sum::<DVec2>() / flat.len().max(1) as f64;
    for p in &mut flat {
        *p -= mean;
    }
    let centroid = centroid + u * mean.x + v * mean.y;
    Flattened { positions, flat, indices, faces, tri_faces, strain, normal, centroid, u, v }
}

/// Seams of the given faces (edges with the same face on both sides,
/// like a cylinder's): the sheet must open along them.
pub fn seam_edges(ctx: &crate::regen::context::Context, faces: &[crate::reference::Reference]) -> Vec<Vec<DVec3>> {
    use crate::reference::Reference;
    let mut out = Vec::new();
    for b in &ctx.bodies {
        let selected: Vec<usize> = faces.iter().filter_map(|r| if let Reference::Face(n) = r { b.find_faces(n).first().copied() } else { None }).collect();
        if selected.is_empty() {
            continue;
        }
        let Ok(edge_faces) = b.shape.edge_faces() else { continue };
        for (e, fs) in edge_faces.iter().enumerate() {
            let Some(&f0) = fs.first() else { continue };
            if fs.iter().all(|f| *f == f0)
                && selected.contains(&(f0 as usize))
                && let Some(name) = b.edges.get(e)
                && let Some(pl) = crate::analysis::edge_polyline(ctx, &Reference::Edge((**name).clone()), 32)
            {
                out.push(pl);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Samples on a surface z = f(x, y) over [0, 1]², each triangle with
    /// its own vertices (like `refine`), its face from `face(centroid)`.
    fn sheet(f: impl Fn(f64, f64) -> f64, n: usize, face: impl Fn(DVec3) -> u32) -> SampleMesh {
        let mut m = SampleMesh::default();
        let p = |i: usize, j: usize| {
            let (x, y) = (i as f64 / n as f64, j as f64 / n as f64);
            DVec3::new(x, y, f(x, y))
        };
        for j in 0..n {
            for i in 0..n {
                for tri in [[p(i, j), p(i + 1, j), p(i + 1, j + 1)], [p(i, j), p(i + 1, j + 1), p(i, j + 1)]] {
                    let fc = face((tri[0] + tri[1] + tri[2]) / 3.0);
                    for v in tri {
                        m.indices.push(m.positions.len() as u32);
                        m.positions.push(v);
                        m.normals.push(DVec3::Z);
                        m.faces.push(fc);
                    }
                }
            }
        }
        m
    }

    #[test]
    fn a_cylinder_patch_unrolls_without_strain() {
        // A quarter cylinder of radius 1 is developable: it flattens to a
        // π/2 × 1 rectangle.
        let r = 1.0;
        let mut m = sheet(|_, _| 0.0, 16, |_| 1);
        for p in &mut m.positions {
            let a = p.x * std::f64::consts::FRAC_PI_2;
            *p = DVec3::new(r * a.sin(), p.y, r * (1.0 - a.cos()));
        }
        let f = flatten(&m, &[], 400);
        let (a3, a2) = f.areas();
        assert!((a2 - a3).abs() / a3 < 0.01, "{a3} vs {a2}");
        assert!(f.strain.iter().all(|s| s.abs() < 0.01), "{:?}", f.strain.iter().fold(0.0f64, |m, s| m.max(s.abs())));
    }

    #[test]
    fn a_whole_cylinder_opens_along_its_seam() {
        // A closed ring of radius 1 and height 1, seam at angle 0.
        let n = 24;
        let mut m = sheet(|_, _| 0.0, n, |_| 1);
        for p in &mut m.positions {
            let a = p.x * std::f64::consts::TAU;
            *p = DVec3::new(a.cos(), a.sin(), p.y);
        }
        let seam = vec![DVec3::new(1.0, 0.0, 0.0), DVec3::new(1.0, 0.0, 1.0)];
        let f = flatten(&m, &[seam], 300);
        let (a3, a2) = f.areas();
        assert!((a2 - a3).abs() / a3 < 0.01, "{a3} vs {a2}");
        // It unrolls to about 2π × 1.
        let (lo, hi) = f.flat.iter().fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        let ext = hi - lo;
        assert!((ext.max_element() - a3).abs() < 0.1, "{ext:?}");
    }

    #[test]
    fn a_dome_flattens_with_strain_and_rips_split_the_sheet() {
        // A spherical cap is not developable: some sides must stretch.
        let m = sheet(|x, y| 0.6 - ((x - 0.5).powi(2) + (y - 0.5).powi(2)), 12, |_| 1);
        let f = flatten(&m, &[], 200);
        assert!(f.strain.iter().any(|s| s.abs() > 0.005));
        // The flat area stays close to the 3D area.
        let (a3, a2) = f.areas();
        assert!((a2 - a3).abs() / a3 < 0.1, "{a3} vs {a2}");
        // Ripping along x = 0.5 (with two faces either side) splits its vertices.
        let two = sheet(|_, _| 0.0, 4, |c| if c.x < 0.5 { 1 } else { 2 });
        let rip = vec![DVec3::new(0.5, 0.0, 0.0), DVec3::new(0.5, 1.0, 0.0)];
        let whole = flatten(&two, &[], 10);
        let ripped = flatten(&two, &[rip], 10);
        assert!(ripped.positions.len() > whole.positions.len());
        // The outline of the plain sheet is its 16 boundary sides; inner
        // edges add the 4 sides between the two faces.
        assert_eq!(whole.outline(false).len(), 16);
        assert_eq!(whole.outline(true).len(), 20);
        // Exports: one STL facet per triangle, a DXF line and an SVG line per outline side.
        assert_eq!(whole.to_stl(&whole.positions).matches("facet normal").count(), whole.indices.len() / 3);
        assert_eq!(whole.to_dxf(false).matches("LINE").count(), 16);
        assert_eq!(whole.to_svg(true).matches("<line").count(), 20);
        // Parasolid export: the sheet's boundary is one closed loop of 16
        // vertices; per face, each half is bounded by 12.
        let loops = whole.loops(false);
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].1.iter().map(Vec::len).collect::<Vec<_>>(), [16]);
        let halves = whole.loops(true);
        assert_eq!(halves.len(), 2);
        assert!(halves.iter().all(|(_, l)| l.len() == 1 && l[0].len() == 12), "{halves:?}");
    }
}

// ------------------------------------------------------------ Flatten surfaces dialog

/// The display polyline of a part edge (display tessellation).
pub fn edge_polyline(bodies: &[crate::display::BodyDisplay], r: &crate::reference::Reference) -> Vec<DVec3> {
    let p3 = |p: &[f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
    bodies.iter().find_map(|b| b.edge_refs.iter().position(|x| x == r).and_then(|i| b.mesh.edges.get(i)).map(|pl| pl.iter().map(p3).collect())).unwrap_or_default()
}

/// The Flatten button: the faces' display triangles as one mesh, refined,
/// opened along the rip edges and the closed faces' seams, flattened.
pub fn flatten_faces(bodies: &[crate::display::BodyDisplay], faces: &[crate::reference::Reference], rips: &[crate::reference::Reference], ctx: Option<&crate::regen::context::Context>) -> Flattened {
    let mut m = crate::display::MeshBuffers::default();
    for b in bodies {
        for (fi, r) in b.face_refs.iter().enumerate() {
            if !faces.contains(r) {
                continue;
            }
            let Some(&(first, count)) = b.mesh.face_ranges.get(fi) else { continue };
            let start = m.indices.len() as u32;
            for &ix in b.mesh.indices.get(first as usize..(first + count) as usize).unwrap_or(&[]) {
                m.indices.push(m.positions.len() as u32);
                m.positions.push(b.mesh.positions[ix as usize]);
                m.normals.push(b.mesh.normals[ix as usize]);
            }
            m.face_ranges.push((start, m.indices.len() as u32 - start));
        }
    }
    let mut rip_lines: Vec<Vec<DVec3>> = rips.iter().map(|r| edge_polyline(bodies, r)).collect();
    // Faces that close on themselves (a cylinder) open along their seams.
    if let Some(ctx) = ctx {
        rip_lines.extend(seam_edges(ctx, faces));
    }
    let spacing = crate::thickness::spacing_for(&m, 4000);
    let sheet = crate::thickness::refine(&m, spacing);
    flatten(&sheet, &rip_lines, 300)
}

/// Where the flattened sheet is placed: at a mate connector (origin, x, z),
/// turned by `angle` (radians) and offset along its z; else on the XY
/// plane, turned to best match the faces' footprint (the outer side up),
/// with `origin_vertex`'s nearest sheet point on it.
#[derive(Clone, Copy, Debug, Default)]
pub struct SheetPlacement {
    pub connector: Option<(DVec3, DVec3, DVec3)>,
    pub angle: f64,
    pub offset: f64,
    pub origin_vertex: Option<DVec3>,
}

/// The sheet's vertices placed in 3D.
pub fn placed_sheet(f: &Flattened, p: &SheetPlacement) -> Vec<DVec3> {
    if let Some((origin, x, z)) = p.connector {
        let (a, b) = p.angle.sin_cos();
        let x = x.normalize_or_zero();
        let y = z.normalize_or_zero().cross(x);
        let (u, v) = (x * b + y * a, y * b - x * a);
        let origin = origin + z.normalize_or_zero() * p.offset;
        return f.flat.iter().map(|q| origin + u * q.x + v * q.y).collect();
    }
    // Rotation (no mirroring; the outer side up) best matching the footprint.
    let mirror = f.normal.z < 0.0;
    let q: Vec<DVec2> = f.flat.iter().map(|q| if mirror { DVec2::new(q.x, -q.y) } else { *q }).collect();
    let foot: Vec<DVec2> = f.positions.iter().map(|p| p.truncate()).collect();
    let fm = foot.iter().copied().sum::<DVec2>() / foot.len().max(1) as f64;
    let (mut c, mut s2) = (0.0, 0.0);
    for (a, b) in q.iter().zip(&foot) {
        c += a.dot(*b - fm);
        s2 += a.perp_dot(*b - fm);
    }
    let rot = DVec2::from_angle(s2.atan2(c));
    let mut placed: Vec<DVec2> = q.iter().map(|a| rot.rotate(*a) + fm).collect();
    if let Some(vtx) = p.origin_vertex
        && let Some(i) = (0..f.positions.len()).min_by(|&a, &b| f.positions[a].distance(vtx).total_cmp(&f.positions[b].distance(vtx)))
    {
        let shift = vtx.truncate() - placed[i];
        for p in &mut placed {
            *p += shift;
        }
    }
    placed.into_iter().map(|p| p.extend(0.0)).collect()
}

/// Export as Parasolid: a planar sheet where the flat pattern is placed,
/// one face per flattened face with `inner` (Export inner edges).
pub fn flat_sheet(f: &Flattened, placed: &[DVec3], inner: bool) -> Result<noshape_kernel::Shape, String> {
    let mut faces = Vec::new();
    for (_, loops) in f.loops(inner) {
        let pts: Vec<Vec<DVec3>> = loops.iter().map(|l| l.iter().map(|i| placed[*i as usize]).collect()).collect();
        // Newell normal of each loop; the largest is the outer one.
        let newell = |l: &[DVec3]| l.iter().zip(l.iter().cycle().skip(1)).fold(DVec3::ZERO, |n, (a, b)| n + DVec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y)));
        let Some(outer) = (0..pts.len()).max_by(|a, b| newell(&pts[*a]).length().total_cmp(&newell(&pts[*b]).length())) else { continue };
        let n = newell(&pts[outer]).normalize_or_zero();
        if n == DVec3::ZERO {
            continue;
        }
        let plane = noshape_kernel::Plane::from_normal(pts[outer][0], n);
        let wire = |l: &[DVec3]| noshape_kernel::make_polyline(l, true);
        let holes = pts.iter().enumerate().filter(|(i, _)| *i != outer).map(|(_, l)| wire(l)).collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
        faces.push(noshape_kernel::make_face(&plane, &wire(&pts[outer]).map_err(|e| e.to_string())?, &holes).map_err(|e| e.to_string())?);
    }
    match faces.len() {
        0 => Err("the flat pattern has no boundary".into()),
        1 => Ok(faces.remove(0)),
        _ => Ok(noshape_kernel::sew(&faces, 1e-7, false).map_err(|e| e.to_string())?.shape),
    }
}
