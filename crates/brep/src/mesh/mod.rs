//! Display meshes of analytic faces (lead 10-28: our own mesher, BRepMesh only for the faces it declines).
//!
//! A body is given as its edges (each sampled once here, so the two faces on an edge share its nodes and the
//! mesh stays watertight) and faces bounded by loops of those edges. Each face is meshed on its own and either
//! gets a triangulation or is declined (`None`): the kernel meshes the declined ones with BRepMesh. Meshes are in
//! the surface's own orientation (its normal; the kernel turns them for reversed faces), every boundary node a
//! mesh node.
//!
//! Faces taken so far: planes (constrained Delaunay of their boundary nodes, as the kernel's planar
//! triangulation), cylinders, cones and tori between two rims at constant v (rows of nodes zipped, as the
//! kernel's strips: a torus's rows where its sides' nodes are). Edges: lines and circles (arcs split as BRepMesh splits them, so a face it meshes next to
//! one meshed here shares the nodes). A face with a circle another face on it declines is declined too.

pub mod cdt;

use crate::{Curve, Use};
use glam::{DVec2, DVec3};
use rayon::prelude::*;

/// A face's surface (the kernel reads it from OCCT's geometry).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshSurface {
    /// The plane through `origin` with unit `normal` (the surface's own, `xdir` × `ydir`) and unit `xdir` in it.
    Plane { origin: DVec3, normal: DVec3, xdir: DVec3 },
    /// OCCT's cylinder `origin + radius (cos u x + sin u y) + v axis` (`y` = `axis` × `xdir`, or minus that
    /// when the frame isn't `direct`): its normal is ∂u × ∂v.
    Cylinder { origin: DVec3, axis: DVec3, xdir: DVec3, direct: bool, radius: f64 },
    /// OCCT's cone `origin + (radius + v sin a) (cos u x + sin u y) + v cos a axis` (`a` the half-angle).
    Cone { origin: DVec3, axis: DVec3, xdir: DVec3, direct: bool, radius: f64, angle: f64 },
    /// OCCT's torus `origin + (radius + minor cos v) (cos u x + sin u y) + minor sin v axis`.
    Torus { origin: DVec3, axis: DVec3, xdir: DVec3, direct: bool, radius: f64, minor: f64 },
    /// Any other surface: the face is declined (BRepMesh meshes it), its loops tell which edges it is on.
    Other,
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
pub fn edge_nodes(vertices: &[DVec3], edge: &MeshEdge, defl: Deflection) -> Option<EdgeNodes> {
    let (a, b) = (*vertices.get(edge.v[0] as usize)?, *vertices.get(edge.v[1] as usize)?);
    match edge.curve {
        Some(Curve::Line) => Some(EdgeNodes { points: vec![a, b], params: vec![edge.t0, edge.t1] }),
        Some(Curve::Circle { center, normal, xdir, radius }) => {
            // BRepMesh's split, equal steps; the ends are the vertices.
            let arc = edge.t1 - edge.t0;
            if !(arc > 0.0 && arc <= std::f64::consts::TAU + 1e-9 && radius > 1e-7) {
                return None;
            }
            let n = arc_steps(arc, radius, defl);
            let du = arc / n as f64;
            let ydir = normal.cross(xdir);
            let mut points = Vec::with_capacity(n + 1);
            let mut params = Vec::with_capacity(n + 1);
            for k in 0..=n {
                let t = if k == n { edge.t1 } else { edge.t0 + k as f64 * du };
                params.push(t);
                points.push(match k {
                    0 => a,
                    _ if k == n => b,
                    _ => center + radius * (t.cos() * xdir + t.sin() * ydir),
                });
            }
            Some(EdgeNodes { points, params })
        }
        _ => None,
    }
}

/// How many equal steps an arc of `arc` radians on a circle of `radius` takes under `defl`: BRepMesh's split
/// (BRepMesh_CurveTessellator, GCPnts_TangentialDeflection's circle): half the deflections, at least 3.
fn arc_steps(arc: f64, radius: f64, defl: Deflection) -> usize {
    let (lin, ang) = (0.5 * defl.linear, 0.5 * defl.angular);
    let chord = 2.0 * (1.0 - lin / radius).max(0.0).acos();
    let step = chord.min(ang).max((1e-7 / radius).min(std::f64::consts::FRAC_PI_2));
    ((arc / step).ceil().min(1e6) as usize).max(3)
}

/// Meshes the faces of a body (`vertices`, `edges`, `faces`) that are meshed here.
pub fn mesh_body(vertices: &[DVec3], edges: &[MeshEdge], faces: &[MeshFace], defl: Deflection) -> BodyMesh {
    // A circle on a face declined here is split by BRepMesh for that face: the faces meshed here on it would
    // not share its nodes. Declined in turn, until none is left (before meshing, so none is meshed for
    // nothing; again after, for the faces that failed). Lines are split the same either way: only the other
    // edges' faces count.
    let mut on_edge: Vec<(u32, usize)> = Vec::new();
    for (i, f) in faces.iter().enumerate() {
        on_edge.extend(f.loops.iter().flatten().filter(|u| !matches!(edges.get(u.edge as usize).map(|e| e.curve), Some(Some(Curve::Line)))).map(|u| (u.edge, i)));
    }
    on_edge.sort_unstable();
    on_edge.dedup();
    let on_edge: Vec<Vec<usize>> = on_edge.chunk_by(|a, b| a.0 == b.0).map(|c| c.iter().map(|&(_, i)| i).collect()).collect();
    let settle = |ok: &mut [bool]| loop {
        let mut changed = false;
        for l in &on_edge {
            if l.iter().all(|&i| ok[i]) || l.iter().all(|&i| !ok[i]) {
                continue;
            }
            for &i in l {
                changed |= std::mem::replace(&mut ok[i], false);
            }
        }
        if !changed {
            break;
        }
    };
    let mut ok: Vec<bool> = faces.iter().map(|f| !matches!(f.surface, MeshSurface::Other)).collect();
    settle(&mut ok);
    // Each edge's nodes once, shared by the faces on it.
    let mut out = BodyMesh { edges: vec![None; edges.len()], faces: vec![None; faces.len()] };
    for (f, _) in faces.iter().zip(&ok).filter(|(_, ok)| **ok) {
        for u in f.loops.iter().flatten() {
            let e = u.edge as usize;
            if let (Some(slot @ None), Some(edge)) = (out.edges.get_mut(e), edges.get(e)) {
                *slot = edge_nodes(vertices, edge, defl);
            }
        }
    }
    // The faces on their own, in parallel (as BRepMesh meshes them).
    let edge_nodes = &out.edges;
    out.faces = faces.par_iter().zip(&ok).map(|(f, &ok)| if ok { mesh_face(vertices, edges, f, edge_nodes, defl) } else { None }).collect();
    for (ok, m) in ok.iter_mut().zip(&out.faces) {
        *ok = m.is_some();
    }
    settle(&mut ok);
    for (m, ok) in out.faces.iter_mut().zip(&ok) {
        if !ok {
            *m = None;
        }
    }
    out
}

/// One face, its edges' nodes from `nodes`.
fn mesh_face(vertices: &[DVec3], edges: &[MeshEdge], face: &MeshFace, nodes: &[Option<EdgeNodes>], defl: Deflection) -> Option<FaceMesh> {
    match face.surface {
        MeshSurface::Plane { origin, normal, xdir } => mesh_plane(vertices, edges, face, origin, normal, xdir, nodes),
        MeshSurface::Cylinder { origin, axis, xdir, direct, radius } => mesh_strip(edges, face, &Revolved::new(origin, axis, xdir, direct, radius, 0.0, 0.0), nodes, defl),
        MeshSurface::Cone { origin, axis, xdir, direct, radius, angle } => mesh_strip(edges, face, &Revolved::new(origin, axis, xdir, direct, radius, angle, 0.0), nodes, defl),
        // A spindle or horn torus (the tube through its axis) isn't meshed here.
        MeshSurface::Torus { origin, axis, xdir, direct, radius, minor } if minor > 0.0 && radius > minor => mesh_strip(edges, face, &Revolved::new(origin, axis, xdir, direct, radius, 0.0, minor), nodes, defl),
        MeshSurface::Torus { .. } => None,
        MeshSurface::Other => None,
    }
}

/// A cylinder (`angle` 0), cone or torus (`minor` > 0, `radius` the major one) in OCCT's parameters.
struct Revolved {
    origin: DVec3,
    axis: DVec3,
    xdir: DVec3,
    /// `axis` × `xdir`, or minus that when the frame isn't direct.
    ydir: DVec3,
    radius: f64,
    /// A cone's half-angle: its sine and cosine.
    angle: (f64, f64),
    minor: f64,
}

impl Revolved {
    #[allow(clippy::too_many_arguments)]
    fn new(origin: DVec3, axis: DVec3, xdir: DVec3, direct: bool, radius: f64, angle: f64, minor: f64) -> Revolved {
        let y = axis.cross(xdir);
        Revolved { origin, axis, xdir, ydir: if direct { y } else { -y }, radius, angle: angle.sin_cos(), minor }
    }
    fn torus(&self) -> bool {
        self.minor > 0.0
    }
    /// (u in (−π, π], v) of a point on the surface (a torus's v in (−π, π] too).
    fn uv(&self, p: DVec3) -> DVec2 {
        let d = p - self.origin;
        let (x, y, z) = (d.dot(self.xdir), d.dot(self.ydir), d.dot(self.axis));
        let v = if self.torus() { z.atan2(x.hypot(y) - self.radius) } else { z / self.angle.1 };
        DVec2::new(y.atan2(x), v)
    }
    /// The radius of the circle at constant v.
    fn ring(&self, v: f64) -> f64 {
        if self.torus() { self.radius + self.minor * v.cos() } else { self.radius + v * self.angle.0 }
    }
    /// The sine and cosine of v that [`Revolved::at`] wants (a torus's; a cone's aren't used).
    fn v_trig(&self, v: f64) -> (f64, f64) {
        if self.torus() { v.sin_cos() } else { (0.0, 1.0) }
    }
    /// The point and the surface's own normal (∂u × ∂v, unit; none at an apex) at u (its sine and cosine) and
    /// v (and its, [`Revolved::v_trig`]): the trigonometry once per row and column, not per node.
    fn at(&self, (su, cu): (f64, f64), v: f64, (sv, cv): (f64, f64)) -> (DVec3, Option<DVec3>) {
        let radial = cu * self.xdir + su * self.ydir;
        let tangent = -su * self.xdir + cu * self.ydir;
        let (ring, dv, z) = if self.torus() {
            (self.radius + self.minor * cv, self.minor * (-sv * radial + cv * self.axis), self.minor * sv)
        } else {
            let (s, c) = self.angle;
            (self.radius + v * s, s * radial + c * self.axis, v * c)
        };
        let n = (ring * tangent).cross(dv);
        (self.origin + ring * radial + z * self.axis, (n.length() > 1e-12 * self.radius.max(1e-9) * self.minor.max(1.0)).then(|| n.normalize()))
    }
}

/// A cylinder, cone or torus between two rims at constant v (circles round the axis) and at most two sides at
/// constant u (seams; a cylinder's or cone's straight, a torus's arcs round its tube): rows of nodes at the
/// rims and at the sides' nodes in between, each zipped to the next by u, as the kernel's strips.
fn mesh_strip(edges: &[MeshEdge], face: &MeshFace, s: &Revolved, nodes: &[Option<EdgeNodes>], defl: Deflection) -> Option<FaceMesh> {
    use std::f64::consts::TAU;
    if face.loops.len() > 2 {
        return None;
    }
    let wrap = |d: f64| d - (d / TAU).round() * TAU;
    // Every use's nodes in (u, v), unwrapped along each loop (a seam's two uses at u and u + 2π).
    struct Side {
        uv: Vec<DVec2>,
        points: Vec<DVec3>,
    }
    let mut sides: Vec<(usize, usize, Side)> = Vec::new();
    for (l, uses) in face.loops.iter().enumerate() {
        let (pts, per_use) = loop_nodes(edges, uses, nodes)?;
        let mut uv: Vec<DVec2> = pts.iter().map(|&p| s.uv(p)).collect();
        for k in 1..uv.len() {
            uv[k].x = uv[k - 1].x + wrap(uv[k].x - uv[k - 1].x);
            if s.torus() {
                uv[k].y = uv[k - 1].y + wrap(uv[k].y - uv[k - 1].y);
            }
        }
        // The closing node (the loop's first again) at its unwrapped place after the last.
        let last = uv[uv.len() - 1];
        let close = DVec2::new(last.x + wrap(uv[0].x - last.x), if s.torus() { last.y + wrap(uv[0].y - last.y) } else { uv[0].y });
        for (k, idx) in per_use.iter().enumerate() {
            // Along the use: from v[0] to v[1] reversed when the use runs backwards.
            let along: Vec<usize> = if uses[k].reversed { idx.iter().rev().copied().collect() } else { idx.clone() };
            let n = along.len();
            let side_uv: Vec<DVec2> = along
                .iter()
                .enumerate()
                .map(|(j, &i)| {
                    // The last node of the loop's last use is the loop's first, one turn on.
                    if j + 1 == n && i == 0 { close } else { uv[i] }
                })
                .collect();
            sides.push((l, k, Side { uv: side_uv, points: along.iter().map(|&i| pts[i]).collect() }));
        }
    }
    let (umin, umax) = sides.iter().flat_map(|(_, _, sd)| sd.uv.iter()).fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.x), b.max(q.x)));
    let span = sides.iter().flat_map(|(_, _, sd)| sd.uv.iter()).fold(0.0f64, |m, q| m.max(q.y.abs())).max(umax - umin).max(1.0);
    let tol = 1e-9 * span.max(s.radius);
    // Rims (v constant) and sides (u constant).
    let mut rims: Vec<(f64, Vec<(f64, DVec3)>)> = Vec::new();
    let mut cols: Vec<(f64, Vec<(f64, DVec3)>)> = Vec::new();
    for (_, _, sd) in &sides {
        let (v0, v1) = sd.uv.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.y), b.max(q.y)));
        let (u0, u1) = sd.uv.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.x), b.max(q.x)));
        let (list, key, by_u) = if v1 - v0 <= tol {
            (&mut rims, v0, true)
        } else if u1 - u0 <= tol {
            (&mut cols, u0, false)
        } else {
            return None;
        };
        let at = match list.iter().position(|r| (r.0 - key).abs() <= tol) {
            Some(i) => i,
            None => {
                list.push((key, Vec::new()));
                list.len() - 1
            }
        };
        list[at].1.extend(sd.uv.iter().zip(&sd.points).map(|(q, p)| (if by_u { q.x } else { q.y }, *p)));
    }
    if rims.len() != 2 || cols.len() > 2 || cols.iter().any(|c| (c.0 - umin).abs() > tol && (c.0 - umax).abs() > tol) || (s.torus() && cols.is_empty()) {
        return None;
    }
    rims.sort_by(|a, b| a.0.total_cmp(&b.0));
    cols.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (vlo, vhi) = (rims[0].0, rims[1].0);
    // Each rim and side by u or v, shared ends once.
    for r in rims.iter_mut().chain(cols.iter_mut()) {
        r.1.sort_by(|a, b| a.0.total_cmp(&b.0));
        r.1.dedup_by(|b, a| b.0 - a.0 <= tol);
    }
    for r in &rims {
        if r.1.len() < 2 || (r.1[0].0 - umin).abs() > tol || (r.1[r.1.len() - 1].0 - umax).abs() > tol {
            return None;
        }
    }
    // The sides run from rim to rim, through the same v's (a cylinder's or cone's none between).
    for c in &cols {
        if (c.1[0].0 - vlo).abs() > tol || (c.1[c.1.len() - 1].0 - vhi).abs() > tol || c.1.len() != cols[0].1.len() || c.1.iter().zip(&cols[0].1).any(|(a, b)| (a.0 - b.0).abs() > tol) {
            return None;
        }
    }
    if !s.torus() && cols.iter().any(|c| c.1.len() > 2) {
        return None;
    }
    // Rows from vlo to vhi: the rims, and between them a row at each side node's v (its ends the sides' nodes,
    // split round the axis as a circle there would be).
    // Per row its v and nodes: u, point, normal.
    type Row = (f64, Vec<(f64, DVec3, DVec3)>);
    let mut rows: Vec<Row> = Vec::new();
    let rim = |r: &(f64, Vec<(f64, DVec3)>)| -> Option<Row> {
        let vt = s.v_trig(r.0);
        Some((r.0, r.1.iter().map(|&(u, p)| Some((u, p, s.at(u.sin_cos(), r.0, vt).1?))).collect::<Option<_>>()?))
    };
    rows.push(rim(&rims[0])?);
    if let Some(c0) = cols.first() {
        let ends = [cols.iter().find(|c| (c.0 - umin).abs() <= tol), cols.iter().find(|c| (c.0 - umax).abs() <= tol)];
        for k in 1..c0.1.len().saturating_sub(1) {
            let v = c0.1[k].0;
            let vt = s.v_trig(v);
            let n = arc_steps(umax - umin, s.ring(v), defl);
            let mut row = Vec::with_capacity(n + 1);
            for j in 0..=n {
                let u = if j == n { umax } else { umin + (umax - umin) * j as f64 / n as f64 };
                let (p, normal) = s.at(u.sin_cos(), v, vt);
                let end = if j == 0 { ends[0] } else if j == n { ends[1] } else { None };
                row.push((u, end.map_or(p, |c| c.1[k].1), normal?));
            }
            rows.push((v, row));
        }
    }
    rows.push(rim(&rims[1])?);
    let mut points = Vec::new();
    let mut normals = Vec::new();
    let mut base = Vec::with_capacity(rows.len());
    for (_, r) in &rows {
        base.push(points.len());
        points.extend(r.iter().map(|x| x.1));
        normals.extend(r.iter().map(|x| x.2));
    }
    // Each row zipped to the next (counter-clockwise in (u, v)), advancing on the row whose next node comes
    // first.
    let mut triangles = Vec::with_capacity(2 * points.len());
    for w in 0..rows.len() - 1 {
        let (lo, hi) = (&rows[w].1, &rows[w + 1].1);
        let (blo, bhi) = (base[w] as u32, base[w + 1] as u32);
        let (nlo, nhi) = (lo.len(), hi.len());
        let (mut i, mut j) = (0, 0);
        while i + 1 < nlo || j + 1 < nhi {
            let on_lo = j + 1 == nhi || (i + 1 < nlo && lo[i + 1].0 <= hi[j + 1].0);
            if on_lo {
                triangles.push([blo + i as u32, blo + (i + 1) as u32, bhi + j as u32]);
                i += 1;
            } else {
                triangles.push([blo + i as u32, bhi + (j + 1) as u32, bhi + j as u32]);
                j += 1;
            }
        }
    }
    // Each use's nodes: found by (u, v), listed from the edge's start.
    let node_of = |q: DVec2| -> Option<u32> {
        let w = rows.partition_point(|r| r.0 < q.y - tol);
        let r = &rows.get(w).filter(|r| (r.0 - q.y).abs() <= tol)?.1;
        let k = r.partition_point(|x| x.0 < q.x - tol);
        (k < r.len() && (r[k].0 - q.x).abs() <= tol).then_some((base[w] + k) as u32)
    };
    let mut edge_nodes: Vec<Vec<Vec<u32>>> = face.loops.iter().map(|l| vec![Vec::new(); l.len()]).collect();
    for (l, k, sd) in &sides {
        let mut ids: Vec<u32> = sd.uv.iter().map(|&q| node_of(q)).collect::<Option<_>>()?;
        if face.loops[*l][*k].reversed {
            ids.reverse();
        }
        edge_nodes[*l][*k] = ids;
    }
    Some(FaceMesh { points, normals, triangles, edge_nodes })
}

/// A loop's nodes in order round it (each shared vertex once), and per use the indices of its edge's nodes
/// (from `v[0]` to `v[1]`) into that list. `None` when the uses don't join end to start.
fn loop_nodes(edges: &[MeshEdge], uses: &[Use], nodes: &[Option<EdgeNodes>]) -> Option<(Vec<DVec3>, Vec<Vec<usize>>)> {
    let ends = |u: &Use| -> Option<(u32, u32)> {
        let e = edges.get(u.edge as usize)?;
        Some(if u.reversed { (e.v[1], e.v[0]) } else { (e.v[0], e.v[1]) })
    };
    // In order round the loop (wires needn't list their edges so): each use's end the next one's start. At a
    // vertex several uses start from (a seam's: the rim on and the seam back), not back along the edge just
    // walked, then the next one in the given order.
    let mut from: Vec<(u32, usize)> = uses.iter().enumerate().map(|(k, u)| Some((ends(u)?.0, k))).collect::<Option<_>>()?;
    from.sort_unstable();
    let mut used = vec![false; uses.len()];
    let mut order = Vec::with_capacity(uses.len());
    let mut i = 0;
    loop {
        order.push(i);
        used[i] = true;
        if order.len() == uses.len() {
            break;
        }
        let end = ends(&uses[i])?.1;
        let at = from.partition_point(|&(v, _)| v < end);
        let next = from[at..]
            .iter()
            .take_while(|&&(v, _)| v == end)
            .map(|&(_, k)| k)
            .filter(|&k| !used[k])
            .min_by_key(|&k| (uses[k].edge == uses[i].edge, (k + uses.len() - i) % uses.len()))?;
        i = next;
    }
    if ends(&uses[*order.last()?])?.1 != ends(&uses[order[0]])?.0 {
        return None;
    }
    let mut pts: Vec<DVec3> = Vec::new();
    // Per use (in `uses`' order): its first node's place round the loop, its node count, direction.
    let mut spans: Vec<(usize, usize, bool)> = vec![(0, 0, false); uses.len()];
    for &k in &order {
        let u = &uses[k];
        let n = nodes.get(u.edge as usize)?.as_ref()?;
        let count = n.points.len();
        if count < 2 {
            return None;
        }
        spans[k] = (pts.len(), count, u.reversed);
        // Along the use; its last node is the next use's first.
        if u.reversed {
            pts.extend(n.points.iter().rev().take(count - 1));
        } else {
            pts.extend(n.points.iter().take(count - 1));
        }
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
fn mesh_plane(vertices: &[DVec3], edges: &[MeshEdge], face: &MeshFace, origin: DVec3, normal: DVec3, xdir: DVec3, nodes: &[Option<EdgeNodes>]) -> Option<FaceMesh> {
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
