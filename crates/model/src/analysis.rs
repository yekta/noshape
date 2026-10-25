//! Geometry behind Onshape's analysis tools: curvature combs on edges and
//! along face isocurves (Curve/surface analysis, Shift+C) and dihedral
//! angle combs along edges (Dihedral analysis, Shift+D). The app draws the
//! combs; scaling to the screen is left to it.

use crate::reference::Reference;
use crate::regen::context::{Body, Context};
use glam::DVec3;
use noshape_kernel::ShapeType;

/// A comb: teeth from points on a curve; the tips form its envelope.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Comb {
    pub base: Vec<DVec3>,
    /// Tooth vectors (direction and length before screen scaling).
    pub teeth: Vec<DVec3>,
}

impl Comb {
    pub fn max_len(&self) -> f64 {
        self.teeth.iter().map(|t| t.length()).fold(0.0, f64::max)
    }
}

/// Curvature comb of a polyline sampled densely along a curve: at each
/// interior sample, curvature 1/R from the circle through its neighbours;
/// the tooth points away from the center of curvature (Onshape draws combs
/// on the convex side). Returns the comb and the minimum radius (with its
/// point), `None` radius when the curve is straight.
pub fn polyline_comb(points: &[DVec3]) -> (Comb, Option<(f64, DVec3)>) {
    let mut comb = Comb::default();
    let mut min: Option<(f64, DVec3)> = None;
    if points.len() < 3 {
        return (comb, None);
    }
    for w in points.windows(3) {
        let (a, b, c) = (w[0], w[1], w[2]);
        let (ab, cb) = (a - b, c - b);
        let cross = ab.cross(cb);
        let denom = 2.0 * cross.length_squared();
        let tooth = if denom <= 1e-30 {
            DVec3::ZERO
        } else {
            // Circumcenter of a, b, c relative to b.
            let center = (ab.length_squared() * cb - cb.length_squared() * ab).cross(cross) / denom;
            let r = center.length();
            if r > 0.0 && min.is_none_or(|(m, _)| r < m) {
                min = Some((r, b));
            }
            -center.normalize_or_zero() / r.max(1e-30)
        };
        comb.base.push(b);
        comb.teeth.push(tooth);
    }
    (comb, min)
}

fn body_of<'a>(ctx: &'a Context, r: &Reference) -> Option<(&'a Body, ShapeType, usize)> {
    ctx.bodies.iter().find_map(|b| match r {
        Reference::Face(n) => b.find_faces(n).first().map(|i| (&**b, ShapeType::Face, *i)),
        Reference::Edge(n) => b.find_edges(n).first().map(|i| (&**b, ShapeType::Edge, *i)),
        _ => None,
    })
}

/// Points evenly spaced in parameter along a part edge.
fn edge_points(b: &Body, e: usize, samples: usize) -> Option<Vec<DVec3>> {
    let s = b.shape.sub_shape(ShapeType::Edge, e).ok()?;
    let info = s.edge_info().ok()?;
    let n = samples.max(3);
    (0..=n).map(|i| s.point_at(info.first_param + (info.last_param - info.first_param) * i as f64 / n as f64).ok()).collect()
}

/// Curvature comb of a part edge and its minimum radius.
pub fn edge_curvature(ctx: &Context, edge: &Reference, samples: usize) -> Option<(Comb, Option<(f64, DVec3)>)> {
    let (b, kind, e) = body_of(ctx, edge)?;
    if kind != ShapeType::Edge {
        return None;
    }
    Some(polyline_comb(&edge_points(b, e, samples)?))
}

/// Isocurve combs of a face: `u_curves` curves of constant u (running in
/// v, Onshape's "V curves", blue) and `v_curves` of constant v ("U curves",
/// red). Returns (u-direction combs, v-direction combs).
pub fn face_curvature(ctx: &Context, face: &Reference, u_curves: usize, v_curves: usize, samples: usize) -> Option<(Vec<Comb>, Vec<Comb>)> {
    let (b, kind, f) = body_of(ctx, face)?;
    if kind != ShapeType::Face {
        return None;
    }
    let s = b.shape.sub_shape(ShapeType::Face, f).ok()?;
    let [u0, u1, v0, v1] = s.uv_bounds().ok()?;
    let n = samples.max(3);
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    // Only points inside the trimmed face (isocurves of the underlying surface).
    let on_face = |p: DVec3| s.project_point(p).map(|q| q.point.distance(p) < 1e-7_f64.max(p.length() * 1e-7)).unwrap_or(false);
    let curve = |pts: Vec<DVec3>| -> Comb {
        let pts: Vec<DVec3> = pts.into_iter().filter(|p| on_face(*p)).collect();
        polyline_comb(&pts).0
    };
    let along_u: Vec<Comb> = (0..v_curves.max(1))
        .map(|j| {
            let v = lerp(v0, v1, (j as f64 + 0.5) / v_curves.max(1) as f64);
            curve((0..=n).filter_map(|i| s.point_at_uv(lerp(u0, u1, i as f64 / n as f64), v).ok()).collect())
        })
        .collect();
    let along_v: Vec<Comb> = (0..u_curves.max(1))
        .map(|j| {
            let u = lerp(u0, u1, (j as f64 + 0.5) / u_curves.max(1) as f64);
            curve((0..=n).filter_map(|i| s.point_at_uv(u, lerp(v0, v1, i as f64 / n as f64)).ok()).collect())
        })
        .collect();
    Some((along_u, along_v))
}

/// Dihedral comb of an edge between two faces: at each sample the tooth
/// length is the angle (radians) between the faces' normals there, pointing
/// out of the material; returns the comb and the min / max angle in degrees.
pub fn edge_dihedral(ctx: &Context, edge: &Reference, samples: usize) -> Option<(Comb, f64, f64)> {
    let (b, kind, e) = body_of(ctx, edge)?;
    if kind != ShapeType::Edge {
        return None;
    }
    let faces = b.shape.edge_faces().ok()?.get(e)?.clone();
    let (fa, fb) = match faces.as_slice() {
        [a, b2] => (*a as usize, *b2 as usize),
        _ => return None,
    };
    let sa = b.shape.sub_shape(ShapeType::Face, fa).ok()?;
    let sb = b.shape.sub_shape(ShapeType::Face, fb).ok()?;
    let mut comb = Comb::default();
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for p in edge_points(b, e, samples)? {
        let (Ok(na), Ok(nb)) = (sa.normal_at_point(p), sb.normal_at_point(p)) else { continue };
        let (na, nb) = (na.normalize_or_zero(), nb.normalize_or_zero());
        let angle = na.dot(nb).clamp(-1.0, 1.0).acos();
        lo = lo.min(angle.to_degrees());
        hi = hi.max(angle.to_degrees());
        comb.base.push(p);
        comb.teeth.push((na + nb).normalize_or_zero() * angle);
    }
    (!comb.base.is_empty()).then_some((comb, lo, hi))
}

/// Closest point to `p` on segment `a`–`b`, and whether it falls inside
/// the segment (not clamped to an end).
fn closest_on_segment(p: DVec3, a: DVec3, b: DVec3) -> (DVec3, bool) {
    let d = b - a;
    let l2 = d.length_squared();
    if l2 <= 1e-30 {
        return (a, false);
    }
    let t = (p - a).dot(d) / l2;
    (a + d * t.clamp(0.0, 1.0), (0.0..=1.0).contains(&t))
}

/// Closest point to `p` on a polyline; `interior` is false when the
/// closest point is one of the polyline's two ends.
fn closest_on_polyline(p: DVec3, pl: &[DVec3]) -> Option<(DVec3, bool)> {
    let mut best: Option<(f64, DVec3, bool)> = None;
    for (i, w) in pl.windows(2).enumerate() {
        let (q, inside) = closest_on_segment(p, w[0], w[1]);
        let at_end = !inside && ((i == 0 && q == w[0]) || (i + 2 == pl.len() && q == w[1]));
        let d = q.distance_squared(p);
        if best.is_none_or(|b| d < b.0) {
            best = Some((d, q, !at_end));
        }
    }
    best.map(|(_, q, inside)| (q, inside))
}

/// One extremum of a deviation: distance and the two points.
pub type Extremum = (f64, DVec3, DVec3);

/// Deviation between two curve chains (Onshape: Deviation analysis, Curve):
/// minimum and maximum distance from chain A's points to chain B over the
/// range where both overlap (points of A whose closest point on B is past
/// B's ends are left out, which trims chains that don't share endpoints).
pub fn curve_deviation(a: &[DVec3], b: &[DVec3]) -> Option<(Extremum, Extremum)> {
    let mut min: Option<Extremum> = None;
    let mut max: Option<Extremum> = None;
    for (i, p) in a.iter().enumerate() {
        let Some((q, interior)) = closest_on_polyline(*p, b) else { continue };
        // A's own ends pair with B's ends; other points past B's ends overhang.
        if !interior && i != 0 && i + 1 != a.len() {
            continue;
        }
        let d = p.distance(q);
        if min.is_none_or(|m| d < m.0) {
            min = Some((d, *p, q));
        }
        if max.is_none_or(|m| d > m.0) {
            max = Some((d, *p, q));
        }
    }
    Some((min?, max?))
}

/// Closest point on a triangle (Ericson, Real-Time Collision Detection 5.1.5).
pub fn closest_on_triangle(p: DVec3, a: DVec3, b: DVec3, c: DVec3) -> DVec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// Deviation between two surfaces given as triangle meshes (points of A
/// against B's triangles): minimum and maximum distance.
pub fn surface_deviation(a_points: &[DVec3], b_tris: &[[DVec3; 3]]) -> Option<(Extremum, Extremum)> {
    let mut min: Option<Extremum> = None;
    let mut max: Option<Extremum> = None;
    for p in a_points {
        let Some(q) = b_tris.iter().map(|t| closest_on_triangle(*p, t[0], t[1], t[2])).min_by(|x, y| x.distance_squared(*p).total_cmp(&y.distance_squared(*p))) else { continue };
        let d = p.distance(q);
        if min.is_none_or(|m| d < m.0) {
            min = Some((d, *p, q));
        }
        if max.is_none_or(|m| d > m.0) {
            max = Some((d, *p, q));
        }
    }
    Some((min?, max?))
}

/// Principal curvatures (k1 ≥ k2, signed: positive where the surface bends
/// away from its normal, like a convex part) at each vertex of a triangle
/// mesh with vertex normals, by fitting each triangle's second fundamental
/// form to the normal change along its sides and averaging it at the
/// vertices (Rusinkiewicz 2004).
pub fn principal_curvatures(positions: &[[f32; 3]], normals: &[[f32; 3]], indices: &[u32]) -> Vec<(f64, f64)> {
    let p3 = |v: [f32; 3]| DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64);
    let n = positions.len();
    // Per vertex: accumulated tensor [a, b, c] in the vertex frame, and weight.
    let mut acc = vec![([0.0f64; 3], 0.0f64); n];
    let frame = |nrm: DVec3| {
        let u = nrm.any_orthonormal_vector();
        (u, nrm.cross(u))
    };
    for t in indices.as_chunks::<3>().0 {
        let [i0, i1, i2] = t.map(|i| i as usize);
        if i0 >= n || i1 >= n || i2 >= n {
            continue;
        }
        let p = [p3(positions[i0]), p3(positions[i1]), p3(positions[i2])];
        let nv = [p3(normals[i0]).normalize_or_zero(), p3(normals[i1]).normalize_or_zero(), p3(normals[i2]).normalize_or_zero()];
        let face_n = (p[1] - p[0]).cross(p[2] - p[0]);
        let area = face_n.length() * 0.5;
        if area <= 0.0 {
            continue;
        }
        let face_n = face_n.normalize();
        let u = (p[1] - p[0]).normalize();
        let v = face_n.cross(u);
        // Least squares for [a b; b c] from the three sides: e·II = dn.
        let (mut m, mut r) = ([[0.0f64; 3]; 3], [0.0f64; 3]);
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let e = p[b] - p[a];
            let dn = nv[b] - nv[a];
            let (eu, ev) = (e.dot(u), e.dot(v));
            for (row, rhs) in [([eu, ev, 0.0], dn.dot(u)), ([0.0, eu, ev], dn.dot(v))] {
                for i in 0..3 {
                    for j in 0..3 {
                        m[i][j] += row[i] * row[j];
                    }
                    r[i] += row[i] * rhs;
                }
            }
        }
        let Some(abc) = solve3(m, r) else { continue };
        // Into each vertex's frame (project the face frame onto its tangent plane).
        for (k, &vi) in [i0, i1, i2].iter().enumerate() {
            let (vu, vv) = frame(nv[k]);
            let (uu, uv, vu2, vv2) = (vu.dot(u), vu.dot(v), vv.dot(u), vv.dot(v));
            let ii = |x: [f64; 2], y: [f64; 2]| x[0] * (abc[0] * y[0] + abc[1] * y[1]) + x[1] * (abc[1] * y[0] + abc[2] * y[1]);
            let (a2, b2, c2) = (ii([uu, uv], [uu, uv]), ii([uu, uv], [vu2, vv2]), ii([vu2, vv2], [vu2, vv2]));
            let w = area / 3.0;
            acc[vi].0[0] += a2 * w;
            acc[vi].0[1] += b2 * w;
            acc[vi].0[2] += c2 * w;
            acc[vi].1 += w;
        }
    }
    acc.into_iter()
        .map(|(t, w)| {
            if w <= 0.0 {
                return (0.0, 0.0);
            }
            let (a, b, c) = (t[0] / w, t[1] / w, t[2] / w);
            // Eigenvalues of [a b; b c] (outward normals: convex is positive).
            let mean = (a + c) * 0.5;
            let d = (((a - c) * 0.5).powi(2) + b * b).sqrt();
            (mean + d, mean - d)
        })
        .collect()
}

/// Solve a 3×3 linear system (Cramer's rule); `None` when singular.
fn solve3(m: [[f64; 3]; 3], r: [f64; 3]) -> Option<[f64; 3]> {
    let det = |m: [[f64; 3]; 3]| m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let d = det(m);
    if d.abs() < 1e-300 {
        return None;
    }
    let mut out = [0.0; 3];
    for (k, o) in out.iter_mut().enumerate() {
        let mut mk = m;
        for i in 0..3 {
            mk[i][k] = r[i];
        }
        *o = det(mk) / d;
    }
    Some(out)
}

/// Continuity of a connection (Onshape: Connection analysis), weakest
/// first; each class implies the ones before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Continuity {
    None,
    G0,
    G1,
    G2,
    G3,
}

/// Connection analysis tolerances: G1 angle in degrees, G2 / G3 relative
/// differences of curvature and of its rate of change (Onshape's defaults
/// 0.3 deg, 0.025, 0.05); G0 is the model tolerance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContinuityTolerances {
    pub g1_deg: f64,
    pub g2: f64,
    pub g3: f64,
}

impl Default for ContinuityTolerances {
    fn default() -> Self {
        Self { g1_deg: 0.3, g2: 0.025, g3: 0.05 }
    }
}

/// Distance under which two endpoints coincide (G0).
const G0_TOLERANCE: f64 = 1e-6;

/// Whether two values agree within a relative tolerance (with an absolute
/// floor so two near-zero values agree).
fn agree(a: f64, b: f64, rel: f64, floor: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs()) + floor
}

/// Circle through three points: (center, radius), `None` when collinear.
fn circle3(a: DVec3, b: DVec3, c: DVec3) -> Option<(DVec3, f64)> {
    let (ab, cb) = (a - b, c - b);
    let cross = ab.cross(cb);
    let denom = 2.0 * cross.length_squared();
    if denom <= 1e-30 * (ab.length_squared() * cb.length_squared()).max(1e-300) {
        return None;
    }
    let center = b + (ab.length_squared() * cb - cb.length_squared() * ab).cross(cross) / denom;
    Some((center, center.distance(b)))
}

/// End geometry of a polyline at its first point, heading into the curve:
/// unit tangent, curvature vector (toward the center) and the rate of
/// change of curvature along the curve.
fn curve_end(pl: &[DVec3]) -> Option<(DVec3, DVec3, f64)> {
    let chord = (*pl.get(1)? - pl[0]).normalize_or_zero();
    if chord == DVec3::ZERO {
        return None;
    }
    if pl.len() < 3 {
        return Some((chord, DVec3::ZERO, 0.0));
    }
    let Some((c0, r0)) = circle3(pl[0], pl[1], pl[2]) else { return Some((chord, DVec3::ZERO, 0.0)) };
    // Tangent of the circle at the end, oriented along the curve.
    let radial = (pl[0] - c0) / r0;
    let mut t = radial.cross((pl[1] - pl[0]).cross(radial)).normalize_or_zero();
    if t.dot(chord) < 0.0 {
        t = -t;
    }
    let k0 = 1.0 / r0;
    let dk = match pl.get(3) {
        Some(&p3) => {
            let k1 = circle3(pl[1], pl[2], p3).map_or(0.0, |(_, r)| 1.0 / r);
            (k1 - k0) / pl[0].distance(pl[1]).max(1e-30)
        }
        None => 0.0,
    };
    Some((t, -radial * k0, dk))
}

/// Continuity where two curves meet; each polyline starts at the joint.
fn joint_continuity(a: &[DVec3], b: &[DVec3], tol: &ContinuityTolerances) -> Continuity {
    let (Some((ta, ka, da)), Some((tb, kb, db))) = (curve_end(a), curve_end(b)) else { return Continuity::G0 };
    // Smooth through the joint: b's tangent continues a's backwards.
    if ta.dot(-tb).clamp(-1.0, 1.0).acos().to_degrees() > tol.g1_deg {
        return Continuity::G0;
    }
    let scale = a.iter().chain(b).map(|p| p.distance(a[0])).fold(0.0, f64::max).max(1e-9);
    if (ka - kb).length() > tol.g2 * ka.length().max(kb.length()) + 1e-6 / scale {
        return Continuity::G1;
    }
    // Rate of change of curvature along the crossing direction: a runs
    // backwards from the joint.
    if !agree(-da, db, tol.g3, 1e-4 / (scale * scale)) {
        return Continuity::G2;
    }
    Continuity::G3
}

/// Connection analysis of curves (polylines in order along each curve):
/// every endpoint with its continuity. Endpoints meeting no other curve
/// are `Continuity::None`; endpoints where more than two curves meet, and
/// a closed curve's own start / end, are not analyzed (Onshape).
pub fn curve_connections(curves: &[Vec<DVec3>], tol: &ContinuityTolerances) -> Vec<(DVec3, Continuity)> {
    // (curve, at start?) of every endpoint.
    let ends: Vec<(usize, bool, DVec3)> = curves
        .iter()
        .enumerate()
        .filter(|(_, c)| c.len() >= 2)
        .flat_map(|(i, c)| [(i, true, c[0]), (i, false, c[c.len() - 1])])
        .collect();
    let mut out = Vec::new();
    let mut done = vec![false; ends.len()];
    for i in 0..ends.len() {
        if done[i] {
            continue;
        }
        let group: Vec<usize> = (0..ends.len()).filter(|&j| ends[j].2.distance(ends[i].2) <= G0_TOLERANCE).collect();
        for &j in &group {
            done[j] = true;
        }
        let from_joint = |k: usize| -> Vec<DVec3> {
            let (c, start, _) = ends[k];
            if start { curves[c].clone() } else { curves[c].iter().rev().copied().collect() }
        };
        match group.as_slice() {
            [_] => out.push((ends[i].2, Continuity::None)),
            [a, b] if ends[*a].0 != ends[*b].0 => out.push((ends[i].2, joint_continuity(&from_joint(*a), &from_joint(*b), tol))),
            _ => {}
        }
    }
    out
}

/// Polyline of a part edge evaluated by the kernel (`samples` segments).
pub fn edge_polyline(ctx: &Context, edge: &Reference, samples: usize) -> Option<Vec<DVec3>> {
    let (b, kind, e) = body_of(ctx, edge)?;
    (kind == ShapeType::Edge).then(|| edge_points(b, e, samples)).flatten()
}

/// Result of a surface connection analysis: segments of the edges shared
/// by two selected faces with their continuity, and the boundary edges of
/// selected faces that touch no other selected face (no connection).
#[derive(Clone, Debug, Default)]
pub struct FaceConnections {
    pub segments: Vec<([DVec3; 2], Continuity)>,
    pub unconnected: Vec<Vec<DVec3>>,
}

/// Normal curvature of a face across an edge at `p`: stepping `h` and `2h`
/// along the surface in direction `d` (into the face). Returns the
/// curvature at the edge and its rate of change moving into the face.
fn cross_curvature(face: &noshape_kernel::Shape, p: DVec3, n0: DVec3, d: DVec3, h: f64) -> Option<(f64, f64)> {
    let q1 = face.project_point(p + d * h).ok()?.point;
    let q2 = face.project_point(p + d * 2.0 * h).ok()?.point;
    let n1 = face.normal_at_point(q1).ok()?.normalize_or_zero();
    let n2 = face.normal_at_point(q2).ok()?.normalize_or_zero();
    let k = |na: DVec3, nb: DVec3, a: DVec3, b: DVec3| (nb - na).dot(b - a) / (b - a).length_squared().max(1e-300);
    let (k1, k2) = (k(n0, n1, p, q1), k(n1, n2, q1, q2));
    let step = q1.distance(q2).max(1e-300);
    // Mean curvatures over [0, h] and [h, 2h] sit at h/2 and 3h/2.
    Some((k1 - 0.5 * (k2 - k1), (k2 - k1) / step))
}

/// Direction in the face's tangent plane, across the edge (tangent `t`)
/// and into the face.
fn into_face(face: &noshape_kernel::Shape, p: DVec3, n: DVec3, t: DVec3, h: f64) -> Option<DVec3> {
    let d = n.cross(t).normalize_or_zero();
    if d == DVec3::ZERO {
        return None;
    }
    let dist = |d: DVec3| face.project_point(p + d * h).map_or(f64::MAX, |q| q.distance);
    Some(if dist(d) <= dist(-d) { d } else { -d })
}

/// Continuity across an edge between faces `a` and `b` at edge parameter `u`.
fn edge_continuity(edge: &noshape_kernel::Shape, a: &noshape_kernel::Shape, b: &noshape_kernel::Shape, u: f64, h: f64, scale: f64, tol: &ContinuityTolerances) -> Option<(DVec3, Continuity)> {
    let p = edge.point_at(u).ok()?;
    let t = edge.tangent_at(u).ok()?.normalize_or_zero();
    let na = a.normal_at_point(p).ok()?.normalize_or_zero();
    let nb = b.normal_at_point(p).ok()?.normalize_or_zero();
    if na.dot(nb).clamp(-1.0, 1.0).acos().to_degrees() > tol.g1_deg {
        return Some((p, Continuity::G0));
    }
    let (ka, da) = cross_curvature(a, p, na, into_face(a, p, na, t, h)?, h)?;
    let (kb, db) = cross_curvature(b, p, nb, into_face(b, p, nb, t, h)?, h)?;
    if !agree(ka, kb, tol.g2, 1e-6 / scale) {
        return Some((p, Continuity::G1));
    }
    // Crossing from a to b: a's rate runs backwards.
    if !agree(-da, db, tol.g3, 1e-4 / (scale * scale)) {
        return Some((p, Continuity::G2));
    }
    Some((p, Continuity::G3))
}

/// Connection analysis of faces: continuity along every edge shared by two
/// of the selected faces (`samples` segments per edge), plus the boundaries
/// of faces connected to none of the others.
pub fn face_connections(ctx: &Context, faces: &[Reference], samples: usize, tol: &ContinuityTolerances) -> FaceConnections {
    let mut out = FaceConnections::default();
    for b in &ctx.bodies {
        let selected: Vec<usize> = faces.iter().filter_map(|r| if let Reference::Face(n) = r { b.find_faces(n).first().copied() } else { None }).collect();
        if selected.is_empty() {
            continue;
        }
        let Ok(edge_faces) = b.shape.edge_faces() else { continue };
        let mut connected = vec![false; selected.len()];
        for (e, fs) in edge_faces.iter().enumerate() {
            let [fa, fb] = fs.as_slice() else { continue };
            let (fa, fb) = (*fa as usize, *fb as usize);
            let (Some(ia), Some(ib)) = (selected.iter().position(|&f| f == fa), selected.iter().position(|&f| f == fb)) else { continue };
            if fa == fb {
                continue;
            }
            connected[ia] = true;
            connected[ib] = true;
            let (Ok(edge), Ok(sa), Ok(sb)) = (b.shape.sub_shape(ShapeType::Edge, e), b.shape.sub_shape(ShapeType::Face, fa), b.shape.sub_shape(ShapeType::Face, fb)) else { continue };
            let Ok(info) = edge.edge_info() else { continue };
            let (u0, u1, len) = (info.first_param, info.last_param, info.length);
            let scale = len.max(1e-9);
            let h = (len * 1e-3).max(1e-7);
            let n = samples.max(2);
            let at = |i: f64| u0 + (u1 - u0) * i / n as f64;
            for i in 0..n {
                let (Ok(p0), Ok(p1)) = (edge.point_at(at(i as f64)), edge.point_at(at(i as f64 + 1.0))) else { continue };
                // Classified at the segment's middle, which keeps clear of the ends.
                if let Some((_, c)) = edge_continuity(&edge, &sa, &sb, at(i as f64 + 0.5), h, scale, tol) {
                    out.segments.push(([p0, p1], c));
                }
            }
        }
        for (k, &f) in selected.iter().enumerate() {
            if connected[k] {
                continue;
            }
            // Its boundary: edges it shares with another face (not seams).
            for (e, fs) in edge_faces.iter().enumerate() {
                if fs.contains(&(f as u32))
                    && fs.iter().any(|&x| x as usize != f)
                    && let Some(pl) = edge_points(b, e, samples)
                {
                    out.unconnected.push(pl);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_comb_has_constant_curvature() {
        let r = 2.0;
        let pts: Vec<DVec3> = (0..=32).map(|i| {
            let a = i as f64 / 32.0 * std::f64::consts::PI;
            DVec3::new(r * a.cos(), r * a.sin(), 0.0)
        }).collect();
        let (comb, min) = polyline_comb(&pts);
        assert_eq!(comb.base.len(), 31);
        for (b, t) in comb.base.iter().zip(&comb.teeth) {
            assert!((t.length() - 1.0 / r).abs() < 1e-3, "{}", t.length());
            // Away from the center (the origin).
            assert!(t.dot(*b) > 0.0);
        }
        assert!((min.unwrap().0 - r).abs() < 1e-3);
        // A straight line: no curvature, no radius.
        let line: Vec<DVec3> = (0..5).map(|i| DVec3::new(i as f64, 0.0, 0.0)).collect();
        let (c, m) = polyline_comb(&line);
        assert!(c.max_len() == 0.0 && m.is_none());
    }

    #[test]
    fn plate_edges_dihedral_and_curvature() {
        let ps = crate::demo::plate_studio();
        let ctx = crate::regen::Regenerator::new().regenerate(&ps, &Default::default(), &noshape_units::UnitPrefs::default(), &crate::regen::RegenOptions::default()).context;
        let b = &ctx.bodies[0];
        let r = |e: usize| Reference::Edge((*b.edges[e]).clone());
        // Some edge meets at 90° (a plate corner edge).
        let square = (0..b.edges.len()).filter_map(|e| edge_dihedral(&ctx, &r(e), 8)).any(|(_, lo, hi)| (lo - 90.0).abs() < 1.0 && (hi - 90.0).abs() < 1.0);
        assert!(square);
        // A circular edge has a comb with a finite minimum radius.
        let curved = (0..b.edges.len()).filter_map(|e| edge_curvature(&ctx, &r(e), 16)).any(|(c, m)| c.max_len() > 0.0 && m.is_some());
        assert!(curved);
    }

    #[test]
    fn deviation_between_curves_and_surfaces() {
        // A line and an arc-like polyline above it: min 1 at the ends, max 2 in the middle.
        let a: Vec<DVec3> = (0..=10).map(|i| DVec3::new(i as f64, 0.0, 0.0)).collect();
        let b: Vec<DVec3> = (0..=10).map(|i| {
            let x = i as f64;
            DVec3::new(x, 1.0 + (1.0 - ((x - 5.0) / 5.0).powi(2)), 0.0)
        }).collect();
        let (min, max) = curve_deviation(&a, &b).unwrap();
        assert!((min.0 - 1.0).abs() < 0.05, "{}", min.0);
        assert!((max.0 - 2.0).abs() < 0.05, "{}", max.0);
        // Two parallel squares 3 apart.
        let tri = [[DVec3::ZERO, DVec3::X, DVec3::Y], [DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y]];
        let pts = [DVec3::new(0.2, 0.2, 3.0), DVec3::new(0.8, 0.5, 3.0)];
        let (lo, hi) = surface_deviation(&pts, &tri).unwrap();
        assert!((lo.0 - 3.0).abs() < 1e-9 && (hi.0 - 3.0).abs() < 1e-9);
        assert_eq!(closest_on_triangle(DVec3::new(-1.0, -1.0, 0.0), DVec3::ZERO, DVec3::X, DVec3::Y), DVec3::ZERO);
    }

    #[test]
    fn connection_continuity_of_curves() {
        let tol = ContinuityTolerances::default();
        let arc = |c: DVec3, r: f64, a0: f64, a1: f64| -> Vec<DVec3> {
            (0..=24).map(|i| {
                let a = a0 + (a1 - a0) * i as f64 / 24.0;
                c + DVec3::new(r * a.cos(), r * a.sin(), 0.0)
            }).collect()
        };
        use std::f64::consts::{FRAC_PI_2, PI};
        // A line ending where a tangent arc starts: G1 (curvature jumps).
        let line = vec![DVec3::new(-1.0, 0.0, 0.0), DVec3::new(0.0, 0.0, 0.0)];
        let quarter = arc(DVec3::new(0.0, 1.0, 0.0), 1.0, -FRAC_PI_2, 0.0);
        let res = curve_connections(&[line.clone(), quarter.clone()], &tol);
        let at = |res: &[(DVec3, Continuity)], p: DVec3| res.iter().find(|(q, _)| q.distance(p) < 1e-9).map(|r| r.1);
        assert_eq!(at(&res, DVec3::ZERO), Some(Continuity::G1));
        // The free ends: no connection.
        assert_eq!(at(&res, DVec3::new(-1.0, 0.0, 0.0)), Some(Continuity::None));
        assert_eq!(at(&res, DVec3::new(1.0, 1.0, 0.0)), Some(Continuity::None));
        // Two lines at a corner: G0.
        let up = vec![DVec3::ZERO, DVec3::new(0.0, 1.0, 0.0)];
        assert_eq!(at(&curve_connections(&[line.clone(), up.clone()], &tol), DVec3::ZERO), Some(Continuity::G0));
        // Two arcs of one circle: G3 at both joints.
        let res = curve_connections(&[arc(DVec3::ZERO, 2.0, 0.0, PI), arc(DVec3::ZERO, 2.0, PI, 2.0 * PI)], &tol);
        assert_eq!(res.len(), 2);
        assert!(res.iter().all(|r| r.1 == Continuity::G3), "{res:?}");
        // Arcs of different radii meeting tangentially: G1.
        let small = arc(DVec3::new(1.0, 0.0, 0.0), 1.0, 0.0, FRAC_PI_2);
        let big = arc(DVec3::new(0.0, 0.0, 0.0), 2.0, 0.0, -FRAC_PI_2);
        assert_eq!(at(&curve_connections(&[small, big], &tol), DVec3::new(2.0, 0.0, 0.0)), Some(Continuity::G1));
        // Three curves at one point are not analyzed.
        let down = vec![DVec3::ZERO, DVec3::new(0.0, -1.0, 0.0)];
        assert_eq!(at(&curve_connections(&[line, up, down], &tol), DVec3::ZERO), None);
    }

    #[test]
    fn connection_continuity_of_faces() {
        let ps = crate::demo::plate_studio();
        let ctx = crate::regen::Regenerator::new().regenerate(&ps, &Default::default(), &noshape_units::UnitPrefs::default(), &crate::regen::RegenOptions::default()).context;
        let b = &ctx.bodies[0];
        let faces: Vec<Reference> = b.faces.iter().map(|n| Reference::Face((**n).clone())).collect();
        let tol = ContinuityTolerances::default();
        let all = face_connections(&ctx, &faces, 8, &tol);
        let classes: std::collections::HashSet<Continuity> = all.segments.iter().map(|s| s.1).collect();
        // Fillets meet the flat faces tangentially; the plate's sharp edges are G0.
        assert!(classes.contains(&Continuity::G1), "{classes:?}");
        assert!(classes.contains(&Continuity::G0), "{classes:?}");
        assert!(all.unconnected.is_empty());
        // One face alone connects to nothing: its boundary is shown.
        let one = face_connections(&ctx, &faces[..1], 8, &tol);
        assert!(one.segments.is_empty() && !one.unconnected.is_empty());
    }

    #[test]
    fn principal_curvatures_of_a_sphere_and_a_cylinder() {
        // Sphere of radius 2 (outward normals): both curvatures 1/2.
        let r = 2.0f64;
        let (mut pos, mut nrm, mut idx) = (Vec::new(), Vec::new(), Vec::new());
        let (nu, nv) = (32, 16);
        for j in 0..=nv {
            for i in 0..=nu {
                let (th, ph) = (j as f64 / nv as f64 * std::f64::consts::PI, i as f64 / nu as f64 * std::f64::consts::TAU);
                let d = DVec3::new(th.sin() * ph.cos(), th.sin() * ph.sin(), th.cos());
                pos.push([(d.x * r) as f32, (d.y * r) as f32, (d.z * r) as f32]);
                nrm.push([d.x as f32, d.y as f32, d.z as f32]);
            }
        }
        let at = |i: usize, j: usize| (j * (nu + 1) + i) as u32;
        for j in 0..nv {
            for i in 0..nu {
                idx.extend_from_slice(&[at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i, j), at(i + 1, j + 1), at(i + 1, j)]);
            }
        }
        let k = principal_curvatures(&pos, &nrm, &idx);
        let mid = at(nu / 4, nv / 2) as usize;
        assert!((k[mid].0 - 0.5).abs() < 0.03 && (k[mid].1 - 0.5).abs() < 0.03, "{:?}", k[mid]);
        // Cylinder of radius 2 around Z: 1/2 and 0.
        for (p, n) in pos.iter_mut().zip(nrm.iter_mut()) {
            let d = DVec3::new(p[0] as f64, p[1] as f64, 0.0).normalize_or_zero();
            let z = p[2];
            *p = [(d.x * r) as f32, (d.y * r) as f32, z];
            *n = [d.x as f32, d.y as f32, 0.0];
        }
        let k = principal_curvatures(&pos, &nrm, &idx);
        assert!((k[mid].0 - 0.5).abs() < 0.05 && k[mid].1.abs() < 0.05, "{:?}", k[mid]);
    }
}

/// What the curvature color map shows (its Curvature menu).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CurvatureKind {
    #[default]
    Gaussian,
    Mean,
    MaxRadius,
    MinRadius,
}

impl CurvatureKind {
    pub const ALL: [CurvatureKind; 4] = [CurvatureKind::Gaussian, CurvatureKind::Mean, CurvatureKind::MaxRadius, CurvatureKind::MinRadius];

    pub fn label(self, unit: &str) -> String {
        match self {
            CurvatureKind::Gaussian => format!("Gaussian (1/{unit}^2)"),
            CurvatureKind::Mean => format!("Mean (1/{unit})"),
            CurvatureKind::MaxRadius => format!("Max radius ({unit})"),
            CurvatureKind::MinRadius => format!("Min radius ({unit})"),
        }
    }

    /// The value at a point with principal curvatures k1, k2 (1/m), in
    /// display units (`m`: meters per unit); `None` where nothing curves.
    pub fn value(self, (k1, k2): (f64, f64), m: f64, flat: f64) -> Option<f64> {
        if k1.abs() < flat && k2.abs() < flat {
            return None;
        }
        Some(match self {
            CurvatureKind::Gaussian => k1 * k2 * m * m,
            CurvatureKind::Mean => (k1 + k2) * 0.5 * m,
            CurvatureKind::MaxRadius => 1.0 / k1.abs().min(k2.abs()).max(flat) / m,
            CurvatureKind::MinRadius => 1.0 / k1.abs().max(k2.abs()) / m,
        })
    }
}

/// Per mesh vertex: the curvature color map's value (`kind`, display units
/// with `meters_per_unit`), `None` where the part is flat. Flat means both
/// principal curvatures under 1e-3 / the part's box diagonal.
pub fn curvature_values(mesh: &crate::display::MeshBuffers, size: f64, kind: CurvatureKind, meters_per_unit: f64) -> Vec<Option<f64>> {
    principal_curvatures(&mesh.positions, &mesh.normals, &mesh.indices).into_iter().map(|k| kind.value(k, meters_per_unit, 1e-3 / size.max(1e-9))).collect()
}


/// Draft analysis: how a mesh vertex draws from the mold (Onshape's legend).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftClass {
    /// Draft angle under the minimum.
    Steep,
    /// Material in the way when pulling the face out.
    Undercut,
    /// Draft at least the minimum toward the pull direction.
    Side1,
    /// Draft at least the minimum away from it.
    Side2,
}

/// Each vertex's draft angle (radians, asin(n · pull)) and class, for
/// pull direction `pull`, minimum draft `min_angle` (radians) and, with
/// `undercut`, undercut detection (rays along the pull hitting the part).
/// `size`: the part's size (ray offsets).
pub fn draft_classes(mesh: &crate::display::MeshBuffers, size: f64, pull: DVec3, min_angle: f64, undercut: bool) -> Vec<(f64, DraftClass)> {
    let p3 = |v: [f32; 3]| DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64);
    let d = pull.normalize_or(DVec3::Z);
    let lim = min_angle.sin();
    let size = size.max(1e-9);
    let blocked: Vec<bool> = if undercut {
        let tris: Vec<[DVec3; 3]> = mesh.indices.as_chunks::<3>().0.iter().filter_map(|t| Some([p3(*mesh.positions.get(t[0] as usize)?), p3(*mesh.positions.get(t[1] as usize)?), p3(*mesh.positions.get(t[2] as usize)?)])).collect();
        let rays: Vec<(DVec3, DVec3)> = mesh
            .positions
            .iter()
            .zip(&mesh.normals)
            .map(|(p, n)| {
                let n = p3(*n).normalize_or_zero();
                let dir = if n.dot(d) >= 0.0 { d } else { -d };
                // Start just off the surface so the face itself isn't hit.
                (p3(*p) + n * size * 1e-5, dir)
            })
            .collect();
        crate::thickness::cast_rays(tris, &rays, size * 1e-5).into_iter().map(|h| h.is_some()).collect()
    } else {
        vec![false; mesh.positions.len()]
    };
    mesh.normals
        .iter()
        .zip(blocked)
        .map(|(n, blocked)| {
            let s = p3(*n).normalize_or_zero().dot(d);
            let class = if s.abs() < lim - 1e-9 {
                DraftClass::Steep
            } else if blocked {
                DraftClass::Undercut
            } else if s > 0.0 {
                DraftClass::Side1
            } else {
                DraftClass::Side2
            };
            (s.clamp(-1.0, 1.0).asin(), class)
        })
        .collect()
}
