//! Display meshes of analytic faces (lead 10-28: our own mesher, BRepMesh only for the faces it declines).
//!
//! A body is given as its edges (each sampled once here, so the two faces on an edge share its nodes and the
//! mesh stays watertight) and faces bounded by loops of those edges. Each face is meshed on its own and either
//! gets a triangulation or is declined (`None`): the kernel meshes the declined ones with BRepMesh. Meshes are in
//! the surface's own orientation (its normal; the kernel turns them for reversed faces), every boundary node a
//! mesh node.
//!
//! Faces taken so far: planes (constrained Delaunay of their boundary nodes, as the kernel's planar
//! triangulation), cylinders, cones, spheres and tori between two rims at constant v (rows of nodes zipped, as
//! the kernel's strips: a torus's or sphere's rows where its sides' nodes are; an apex or pole one node). Edges: lines and circles (arcs split as BRepMesh splits them, so a face it meshes next to
//! one meshed here shares the nodes). A face with a circle another face on it declines is declined too.

pub mod cdt;

use crate::nurbs::{NurbsCurve, NurbsSurface};
use crate::{Curve, Use};
use glam::{DVec2, DVec3};
use std::f64::consts::{FRAC_PI_2, TAU};

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
    /// OCCT's sphere `origin + radius cos v (cos u x + sin u y) + radius sin v axis` (a torus with no hole).
    Sphere { origin: DVec3, axis: DVec3, xdir: DVec3, direct: bool, radius: f64 },
    /// A B-spline (made non-periodic): [`MeshNurbs::surfaces`]`[index]`.
    Nurbs { index: u32 },
    /// Any other surface: the face is declined (BRepMesh meshes it), its loops tell which edges it is on.
    Other,
}

/// The B-spline curves (edges' [`Curve::Nurbs`]) and surfaces ([`MeshSurface::Nurbs`]) of a body.
#[derive(Clone, Copy, Debug, Default)]
pub struct MeshNurbs<'a> {
    pub curves: &'a [NurbsCurve],
    pub surfaces: &'a [NurbsSurface],
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
    /// How long its edges' nodes and its faces took (the kernel's profile shows them).
    pub took: [std::time::Duration; 2],
}

/// The nodes of `edge` under `defl`, if it is a curve meshed here.
pub fn edge_nodes(vertices: &[DVec3], edge: &MeshEdge, defl: Deflection) -> Option<EdgeNodes> {
    edge_nodes_with(vertices, edge, defl, MeshNurbs::default())
}

/// [`edge_nodes`] with the body's B-spline curves.
pub fn edge_nodes_with(vertices: &[DVec3], edge: &MeshEdge, defl: Deflection, nurbs: MeshNurbs) -> Option<EdgeNodes> {
    let (a, b) = (*vertices.get(edge.v[0] as usize)?, *vertices.get(edge.v[1] as usize)?);
    match edge.curve {
        Some(Curve::Nurbs { index, t0, t1 }) => nurbs_edge_nodes(nurbs.curves.get(index as usize)?, a, b, t0, t1, defl),
        Some(Curve::Line) => Some(EdgeNodes { points: vec![a, b], params: vec![edge.t0, edge.t1] }),
        // A cone's apex or a sphere's pole: its point at both ends (its parameters are the surface's u there).
        Some(Curve::Degenerate) if edge.v[0] == edge.v[1] => Some(EdgeNodes { points: vec![a, a], params: vec![edge.t0, edge.t1] }),
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

/// A B-spline edge's nodes from `t0` (vertex `a`) to `t1` (vertex `b`): each knot span split in halves until a
/// piece's chord is within half the linear deflection of the curve at its middle and its ends' tangents turn less
/// than half the angular one (BRepMesh's tangential deflection takes half of each too).
fn nurbs_edge_nodes(c: &NurbsCurve, a: DVec3, b: DVec3, t0: f64, t1: f64, defl: Deflection) -> Option<EdgeNodes> {
    let (lo, hi) = c.range();
    if !(t0.is_finite() && t1.is_finite()) || t0.min(t1) < lo - 1e-9 * (hi - lo).abs().max(1.0) || t0.max(t1) > hi + 1e-9 * (hi - lo).abs().max(1.0) || (t1 - t0).abs() <= 0.0 {
        return None;
    }
    // BRepMesh's split (BRepMesh_CurveTessellator, GCPnts_TangentialDeflection): half the deflections, a step turning
    // at most that angle and its middle within that of the chord.
    let (lin, ang) = (0.5 * defl.linear, 0.5 * defl.angular);
    let (s0, s1) = (t0.min(t1), t0.max(t1));
    let span = s1 - s0;
    // Breaks: the ends and the knots where the curve may kink (as often as the degree).
    let mut breaks = vec![t0];
    let mut kinks: Vec<f64> = c.knots.chunk_by(|x, y| x == y).filter(|k| k.len() >= c.degree.max(1)).map(|k| k[0]).filter(|&k| k > s0 + 1e-12 * span && k < s1 - 1e-12 * span).collect();
    if t1 < t0 {
        kinks.reverse();
    }
    breaks.extend(kinks);
    breaks.push(t1);
    // Marched from each node: the step its turn rate (|C' × C''| / |C'|², radians a parameter) and its sag
    // (curvature |C' × C''| / |C'|³, chord L sags L² κ / 8) allow, checked by the tangents' turn, the middle's offset
    // from the chord and the middle's tangent against it; shortened until it passes.
    let fits = |x: f64, y: f64| -> bool {
        let (px, py, pm) = (c.derivs(x), c.derivs(y), c.derivs(0.5 * (x + y)));
        let chord = py[0] - px[0];
        let turn = px[1].angle_between(py[1]);
        let off = (pm[0] - 0.5 * (px[0] + py[0])).length();
        let skew = pm[1].angle_between(chord);
        off <= lin && (turn.is_nan() || turn <= ang) && (skew.is_nan() || skew.min(std::f64::consts::PI - skew) <= 0.5 * ang)
    };
    let mut params = vec![t0];
    let mut points = vec![a];
    for w in breaks.windows(2) {
        let (from, to) = (w[0], w[1]);
        let dir = (to - from).signum();
        let mut u = from;
        let most = (to - from).abs();
        while (to - u) * dir > 1e-12 * span {
            let d = c.derivs(u);
            let speed = d[1].length();
            let bend = d[1].cross(d[2]).length();
            let mut h = most;
            if speed > 0.0 && bend > 0.0 {
                h = h.min(ang * speed * speed / bend);
                h = h.min((8.0 * lin * speed / bend).sqrt());
            }
            let left = (to - u).abs();
            // No sliver at the end: the rest in one step, or two equal ones.
            if h >= left {
                h = left;
            } else if h > 0.5 * left {
                h = 0.5 * left;
            }
            h = h.max(span * 1e-9);
            let mut tries = 0;
            while !fits(u, u + dir * h) && tries < 40 {
                h *= 0.7;
                tries += 1;
            }
            u = if (to - (u + dir * h)) * dir <= 1e-12 * span { to } else { u + dir * h };
            params.push(u);
            points.push(c.at(u));
            if params.len() > 100_000 {
                return None;
            }
        }
    }
    // The ends are the vertices (within the edge's tolerance of the curve's).
    *points.last_mut()? = b;
    Some(EdgeNodes { points, params })
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
    mesh_body_with(vertices, edges, faces, defl, MeshNurbs::default())
}

/// [`mesh_body`] with the body's B-spline curves and surfaces.
pub fn mesh_body_with(vertices: &[DVec3], edges: &[MeshEdge], faces: &[MeshFace], defl: Deflection, nurbs: MeshNurbs) -> BodyMesh {
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
    // A face with no loop, or a loop with no edge (a part a mirror left inverted, CLI fuzz seed 355), is BRepMesh's
    // (MESH-EMPTY-LOOP-PANIC).
    let mut ok: Vec<bool> = faces.iter().map(|f| !matches!(f.surface, MeshSurface::Other) && !f.loops.is_empty() && f.loops.iter().all(|l| !l.is_empty())).collect();
    let before = ok.iter().filter(|o| **o).count();
    settle(&mut ok);
    if std::env::var_os("NOSHAPE_MESH_DEBUG").is_some() {
        let other = faces.iter().filter(|f| matches!(f.surface, MeshSurface::Other)).count();
        let nurbs_faces = faces.iter().filter(|f| matches!(f.surface, MeshSurface::Nurbs { .. })).count();
        let no_curve = edges.iter().filter(|e| e.curve.is_none()).count();
        eprintln!("mesh: {} faces ({other} other, {nurbs_faces} B-spline), {before} meshable, {} after their neighbours; {no_curve} of {} edges without a curve here", faces.len(), ok.iter().filter(|o| **o).count(), edges.len());
    }
    // Each edge's nodes once, shared by the faces on it.
    let mut out = BodyMesh { edges: vec![None; edges.len()], faces: vec![None; faces.len()], took: Default::default() };
    let t = std::time::Instant::now();
    let mut used = vec![false; edges.len()];
    for (f, _) in faces.iter().zip(&ok).filter(|(_, ok)| **ok) {
        for u in f.loops.iter().flatten() {
            if let Some(x) = used.get_mut(u.edge as usize) {
                *x = true;
            }
        }
    }
    let used: Vec<usize> = (0..edges.len()).filter(|&e| used[e]).collect();
    // In parallel only for many (a small part's edges sample in microseconds, less than starting threads).
    let run = if used.len() < 512 { used.len().max(1) } else { 64 };
    for (e, n) in in_parallel(used.len(), run, |k| edge_nodes_with(vertices, &edges[used[k]], defl, nurbs)) {
        out.edges[used[e]] = n;
    }
    out.took[0] = t.elapsed();
    // A face on an edge not split here (a curve it doesn't take) is BRepMesh's: settled before any is meshed
    // for nothing.
    for (f, ok) in faces.iter().zip(ok.iter_mut()) {
        if *ok && f.loops.iter().flatten().any(|u| out.edges.get(u.edge as usize).is_none_or(|n| n.is_none())) {
            *ok = false;
        }
    }
    settle(&mut ok);
    let t = std::time::Instant::now();
    // The faces on their own, in parallel (as BRepMesh meshes them), the costliest first, one at a time: a
    // part's few tori and its plate's face round many holes can be most of the time, and started last they
    // kept the others' threads waiting. Cost by the boundary's nodes: a plane's triangulation ~1 µs a node,
    // a strip's ~0.15 µs a node of its rims (its longest use) by its sides (its shortest).
    let edge_nodes = &out.edges;
    let count = |u: &Use| edge_nodes.get(u.edge as usize).and_then(|n| n.as_ref()).map_or(0, |n| n.points.len());
    let cost = |f: &MeshFace| -> f64 {
        let n = f.loops.iter().flatten().map(count);
        match f.surface {
            MeshSurface::Plane { .. } => n.sum::<usize>() as f64,
            _ => {
                let (lo, hi) = n.fold((usize::MAX, 0), |(lo, hi), c| (lo.min(c), hi.max(c)));
                0.15 * (lo.max(1) * hi) as f64
            }
        }
    };
    // B-spline faces after the others (settled): a part's thread or text, many times the rest, isn't meshed
    // for nothing when a face beside it declines.
    let mut order: Vec<(f64, usize)> = Vec::new();
    for later in [false, true] {
        if later {
            for &(_, i) in &order {
                ok[i] = out.faces[i].is_some();
            }
            settle(&mut ok);
        }
        let run: Vec<(f64, usize)> = {
            let mut run: Vec<(f64, usize)> = (0..faces.len()).filter(|&i| ok[i] && matches!(faces[i].surface, MeshSurface::Nurbs { .. }) == later).map(|i| (cost(&faces[i]), i)).collect();
            run.sort_by(|a, b| b.0.total_cmp(&a.0));
            run
        };
        let mesh = |k: usize| mesh_face(vertices, edges, &faces[run[k].1], edge_nodes, defl, nurbs);
        for (k, m) in in_parallel(run.len(), 1, mesh) {
            out.faces[run[k].1] = m;
        }
        order.extend(run);
    }
    out.took[1] = t.elapsed();
    if std::env::var_os("NOSHAPE_MESH_DEBUG").is_some() {
        let mut failed = std::collections::BTreeMap::<&str, usize>::new();
        for &(_, i) in &order {
            if out.faces[i].is_none() {
                let kind = match faces[i].surface {
                    MeshSurface::Plane { .. } => "plane",
                    MeshSurface::Cylinder { .. } => "cylinder",
                    MeshSurface::Nurbs { .. } => "B-spline",
                    _ => "other",
                };
                *failed.entry(kind).or_default() += 1;
            }
        }
        eprintln!("mesh: failed here: {failed:?}");
    }
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

/// `f` of each of 0..n, in runs of `run` on scoped threads (not a shared pool, whose idle callers would take up
/// other jobs: another body's mesh, waiting on the kernel's mesh lock this one holds); (index, value) in no
/// particular order.
fn in_parallel<T: Send>(n: usize, run: usize, f: impl Fn(usize) -> T + Sync) -> Vec<(usize, T)> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(8).min(n.div_ceil(run));
    if threads <= 1 {
        return (0..n).map(|i| (i, f(i))).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let work = || {
        let mut got = Vec::new();
        loop {
            let a = next.fetch_add(run, std::sync::atomic::Ordering::Relaxed);
            if a >= n {
                return got;
            }
            got.extend((a..(a + run).min(n)).map(|i| (i, f(i))));
        }
    };
    std::thread::scope(|sc| {
        let others: Vec<_> = (1..threads).map(|_| sc.spawn(work)).collect();
        let mut all = work();
        for h in others {
            all.extend(h.join().unwrap_or_default());
        }
        all
    })
}

/// One face, its edges' nodes from `nodes`.
fn mesh_face(vertices: &[DVec3], edges: &[MeshEdge], face: &MeshFace, nodes: &[Option<EdgeNodes>], defl: Deflection, nurbs: MeshNurbs) -> Option<FaceMesh> {
    match face.surface {
        MeshSurface::Nurbs { index } => mesh_grid(edges, face, nurbs.surfaces.get(index as usize)?, nodes, defl),
        MeshSurface::Plane { origin, normal, xdir } => mesh_plane(vertices, edges, face, origin, normal, xdir, nodes),
        // A strip's rows, else (a piece bounded by other curves: a knurl's lands) a grid as a B-spline's.
        MeshSurface::Cylinder { origin, axis, xdir, direct, radius } => mesh_revolved(edges, face, &Revolved::new(origin, axis, xdir, direct, radius, 0.0, 0.0), nodes, defl),
        MeshSurface::Cone { origin, axis, xdir, direct, radius, angle } => mesh_revolved(edges, face, &Revolved::new(origin, axis, xdir, direct, radius, angle, 0.0), nodes, defl),
        // A spindle or horn torus (the tube through its axis) isn't meshed here.
        MeshSurface::Torus { origin, axis, xdir, direct, radius, minor } if minor > 0.0 && radius > minor => mesh_revolved(edges, face, &Revolved::new(origin, axis, xdir, direct, radius, 0.0, minor), nodes, defl),
        MeshSurface::Torus { .. } => None,
        MeshSurface::Sphere { origin, axis, xdir, direct, radius } if radius > 0.0 => mesh_revolved(edges, face, &Revolved::new(origin, axis, xdir, direct, 0.0, 0.0, radius), nodes, defl),
        MeshSurface::Sphere { .. } => None,
        MeshSurface::Other => None,
    }
}

fn mesh_revolved(edges: &[MeshEdge], face: &MeshFace, s: &Revolved, nodes: &[Option<EdgeNodes>], defl: Deflection) -> Option<FaceMesh> {
    mesh_strip(edges, face, s, nodes, defl).or_else(|| mesh_grid(edges, face, s, nodes, defl))
}

/// A cylinder (`angle` 0), cone or torus (`minor` > 0, `radius` the major one; a sphere's 0) in OCCT's
/// parameters.
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
    /// 1 when (`xdir`, `ydir`, `axis`) is right-handed, else −1.
    hand: f64,
}

impl Revolved {
    #[allow(clippy::too_many_arguments)]
    fn new(origin: DVec3, axis: DVec3, xdir: DVec3, direct: bool, radius: f64, angle: f64, minor: f64) -> Revolved {
        let y = axis.cross(xdir);
        Revolved { origin, axis, xdir, ydir: if direct { y } else { -y }, radius, angle: angle.sin_cos(), minor, hand: if direct { 1.0 } else { -1.0 } }
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
    /// The normal of the side line through u at v (u's sine and cosine, v's [`Revolved::v_trig`]) leaving the
    /// apex or pole there: ∂u × ∂v without ∂u's length (none there), `sign` that length's sign beside it.
    fn side_normal(&self, (su, cu): (f64, f64), (sv, cv): (f64, f64), sign: f64) -> Option<DVec3> {
        let radial = cu * self.xdir + su * self.ydir;
        let tangent = -su * self.xdir + cu * self.ydir;
        let dv = if self.torus() { self.minor * (-sv * radial + cv * self.axis) } else { self.angle.0 * radial + self.angle.1 * self.axis };
        (sign * tangent.cross(dv)).try_normalize()
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
        let p = self.origin + ring * radial + z * self.axis;
        if self.torus() {
            // ∂u × ∂v is ring · minor · (cos v · radial + sin v · axis), handed as the frame: unit as it is.
            let ok = ring.abs() * self.minor > 1e-12 * self.radius.max(1e-9) * self.minor.max(1.0);
            return (p, ok.then(|| (self.hand * ring.signum()) * (cv * radial + sv * self.axis)));
        }
        let n = (ring * tangent).cross(dv);
        (p, (n.length() > 1e-12 * self.radius.max(1e-9) * self.minor.max(1.0)).then(|| n.normalize()))
    }
}

/// A cylinder, cone or torus between two rims at constant v (circles round the axis) and at most two sides at
/// constant u (seams; a cylinder's or cone's straight, a torus's arcs round its tube): rows of nodes at the
/// rims and at the sides' nodes in between, each zipped to the next by u, as the kernel's strips.
fn mesh_strip(edges: &[MeshEdge], face: &MeshFace, s: &Revolved, nodes: &[Option<EdgeNodes>], defl: Deflection) -> Option<FaceMesh> {
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
        // An apex or pole (a degenerate edge) has no u of its own: its use runs from its start parameter to its
        // end one (the surface's u), a whole turn kept as it is.
        let mut turn: Vec<Option<f64>> = vec![None; uv.len()];
        for (k, idx) in per_use.iter().enumerate() {
            let e = edges.get(uses[k].edge as usize)?;
            if e.curve == Some(Curve::Degenerate) {
                let (a, b, ta, tb) = if uses[k].reversed { (idx[1], idx[0], e.t1, e.t0) } else { (idx[0], idx[1], e.t0, e.t1) };
                if b != a + 1 {
                    return None;
                }
                uv[a].x = ta;
                turn[b] = Some(tb - ta);
            }
        }
        for k in 1..uv.len() {
            uv[k].x = uv[k - 1].x + turn[k].unwrap_or_else(|| wrap(uv[k].x - uv[k - 1].x));
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
    let apex = |v: f64| s.ring(v).abs() <= 1e-9 * (s.radius + s.minor);
    let rim = |r: &(f64, Vec<(f64, DVec3)>)| -> Option<Row> {
        if apex(r.0) {
            // Its point for now (its nodes once the next row's are known, below).
            return Some((r.0, vec![(f64::NAN, r.1[0].1, DVec3::ZERO)]));
        }
        let vt = s.v_trig(r.0);
        Some((r.0, r.1.iter().map(|&(u, p)| Some((u, p, s.at(u.sin_cos(), r.0, vt).1?))).collect::<Option<_>>()?))
    };
    rows.push(rim(&rims[0])?);
    if let Some(c0) = cols.first() {
        let ends = [cols.iter().find(|c| (c.0 - umin).abs() <= tol), cols.iter().find(|c| (c.0 - umax).abs() <= tol)];
        let mut trig: Vec<(f64, (f64, f64))> = Vec::new();
        for k in 1..c0.1.len().saturating_sub(1) {
            let v = c0.1[k].0;
            let vt = s.v_trig(v);
            let n = arc_steps(umax - umin, s.ring(v), defl);
            // The u's and their sines and cosines, the same for each row of as many nodes.
            if trig.len() != n + 1 {
                trig = (0..=n)
                    .map(|j| {
                        let u = if j == n { umax } else { umin + (umax - umin) * j as f64 / n as f64 };
                        (u, u.sin_cos())
                    })
                    .collect();
            }
            let mut row = Vec::with_capacity(n + 1);
            for (j, &(u, su)) in trig.iter().enumerate() {
                let (p, normal) = s.at(su, v, vt);
                let end = if j == 0 { ends[0] } else if j == n { ends[1] } else { None };
                row.push((u, end.map_or(p, |c| c.1[k].1), normal?));
            }
            rows.push((v, row));
        }
    }
    rows.push(rim(&rims[1])?);
    // An apex or pole: a node per triangle of the fan to it, at the middle u of the triangle's side, with the
    // normal of the side line there (a cone's is the same all along it; a sphere's pole's the axis), not one node
    // with one normal, which shaded a cone's tip as a round blob (PSUI-CONE-SHADING).
    let is_apex: Vec<bool> = rows.iter().map(|r| r.1.len() == 1 && r.1[0].0.is_nan()).collect();
    let last = rows.len() - 1;
    for (w, next) in [(0, 1), (last, last - 1)] {
        if !is_apex[w] {
            continue;
        }
        if is_apex[next] || rows[next].1.len() < 2 {
            return None;
        }
        let (v, p) = (rows[w].0, rows[w].1[0].1);
        let (vt, sign) = (s.v_trig(v), s.ring(rows[next].0).signum());
        let fan = rows[next].1.windows(2).map(|q| {
            let u = 0.5 * (q[0].0 + q[1].0);
            Some((u, p, s.side_normal(u.sin_cos(), vt, sign)?))
        });
        rows[w].1 = fan.collect::<Option<_>>()?;
    }
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
        // A fan to an apex: its k-th node in the k-th triangle.
        if is_apex[w + 1] {
            triangles.extend((0..nlo - 1).map(|k| [blo + k as u32, blo + k as u32 + 1, bhi + k as u32]));
            continue;
        }
        if is_apex[w] {
            triangles.extend((0..nhi - 1).map(|k| [blo + k as u32, bhi + k as u32 + 1, bhi + k as u32]));
            continue;
        }
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
        if is_apex[w] {
            // Every node of an apex is its point: the nearest by u.
            return Some((base[w] + r.partition_point(|x| x.0 < q.x).min(r.len() - 1)) as u32);
        }
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
    // A wire with no edges (MESH-EMPTY-LOOP-PANIC): nothing to go round.
    if uses.is_empty() {
        return None;
    }
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

/// A B-spline face: its boundary nodes (its edges', shared with its neighbours) put on the surface's parameter
/// plane, a grid of nodes inside it fine enough for the deflections (each knot span cut into as many steps as its
/// second derivative over its first asks: chord `|S''| h² / 8` within the linear deflection, turn `|S''| h / |S'|`
/// within the angular), then all triangulated by [`cdt::triangulate_with`] on the plane scaled by the surface's
/// mean first derivatives (near isotropic). Declined: a seam (an edge twice in the face), a boundary node off the
/// surface, a degenerate parameterisation.
fn mesh_grid(edges: &[MeshEdge], face: &MeshFace, s: &impl GridSurface, nodes: &[Option<EdgeNodes>], defl: Deflection) -> Option<FaceMesh> {
    let r = mesh_grid_why(edges, face, s, nodes, defl);
    if let Err(why) = &r
        && std::env::var_os("NOSHAPE_MESH_DEBUG").is_some()
    {
        eprintln!("mesh: {} face declined: {why}", s.name());
    }
    r.ok()
}

/// A surface [`mesh_grid`] meshes: its parameters' box, its derivatives, a point's parameters.
trait GridSurface: Sync {
    fn name(&self) -> &'static str;
    /// Closed round u (a seam's two uses at u and a period on, from [`GridSurface::param_of`]'s seed).
    fn closed(&self) -> bool;
    fn range(&self) -> ((f64, f64), (f64, f64));
    /// The parameters along u (v) where the second derivatives may jump (a B-spline's knots).
    fn breaks(&self, along_u: bool) -> &[f64];
    /// [S, Su, Sv, Suu, Suv, Svv] at (u, v).
    fn derivs(&self, u: f64, v: f64) -> [DVec3; 6];
    fn at(&self, u: f64, v: f64) -> DVec3;
    /// A node's point and first derivatives: [S, Su, Sv].
    fn d1(&self, u: f64, v: f64) -> [DVec3; 3] {
        let d = self.derivs(u, v);
        [self.at(u, v), d[1], d[2]]
    }
    /// The parameters of a point near the surface (the nearest to `seed`'s where the surface closes) and its
    /// distance from there.
    fn param_of(&self, p: DVec3, seed: Option<DVec2>) -> (DVec2, f64);
}

impl GridSurface for NurbsSurface {
    fn name(&self) -> &'static str {
        "B-spline"
    }
    fn closed(&self) -> bool {
        self.closed_in_u()
    }
    fn range(&self) -> ((f64, f64), (f64, f64)) {
        NurbsSurface::range(self)
    }
    fn breaks(&self, along_u: bool) -> &[f64] {
        if along_u { &self.uknots } else { &self.vknots }
    }
    fn derivs(&self, u: f64, v: f64) -> [DVec3; 6] {
        NurbsSurface::derivs(self, u, v)
    }
    fn at(&self, u: f64, v: f64) -> DVec3 {
        NurbsSurface::at(self, u, v)
    }
    fn d1(&self, u: f64, v: f64) -> [DVec3; 3] {
        NurbsSurface::d1(self, u, v)
    }
    /// Closed round u: a node the iteration from `seed` can't reach across the seam is found afresh, and one on the
    /// seam is at u0 or u1, the one nearer `seed`.
    fn param_of(&self, p: DVec3, seed: Option<DVec2>) -> (DVec2, f64) {
        let mut r = NurbsSurface::param_of(self, p, seed);
        let Some(s) = seed else { return r };
        if !self.closed_in_u() {
            return r;
        }
        if r.1 > 1e-9 * (1.0 + p.abs().max_element()) {
            let fresh = NurbsSurface::param_of(self, p, None);
            if fresh.1 < r.1 {
                r = fresh;
            }
        }
        let ((u0, u1), _) = NurbsSurface::range(self);
        let eps = 1e-9 * (u1 - u0);
        let nearer_u1 = (s.x - u1).abs() < (s.x - u0).abs();
        if (r.0.x - u0).abs() <= eps && nearer_u1 {
            r.0.x = u1;
        } else if (r.0.x - u1).abs() <= eps && !nearer_u1 {
            r.0.x = u0;
        }
        r
    }
}

impl GridSurface for Revolved {
    fn name(&self) -> &'static str {
        "revolved"
    }
    fn closed(&self) -> bool {
        true
    }
    fn range(&self) -> ((f64, f64), (f64, f64)) {
        let v = if self.torus() && self.radius == 0.0 { (-FRAC_PI_2, FRAC_PI_2) } else { (f64::NEG_INFINITY, f64::INFINITY) };
        ((f64::NEG_INFINITY, f64::INFINITY), v)
    }
    fn breaks(&self, _: bool) -> &[f64] {
        &[]
    }
    fn derivs(&self, u: f64, v: f64) -> [DVec3; 6] {
        let (su, cu) = u.sin_cos();
        let radial = cu * self.xdir + su * self.ydir;
        let tangent = -su * self.xdir + cu * self.ydir;
        if self.torus() {
            let (sv, cv) = v.sin_cos();
            let ring = self.radius + self.minor * cv;
            let m = self.minor;
            [
                self.origin + ring * radial + m * sv * self.axis,
                ring * tangent,
                m * (-sv * radial + cv * self.axis),
                -ring * radial,
                -m * sv * tangent,
                -m * (cv * radial + sv * self.axis),
            ]
        } else {
            let (s, c) = self.angle;
            let ring = self.radius + v * s;
            [self.origin + ring * radial + v * c * self.axis, ring * tangent, s * radial + c * self.axis, -ring * radial, s * tangent, DVec3::ZERO]
        }
    }
    fn at(&self, u: f64, v: f64) -> DVec3 {
        self.at(u.sin_cos(), v, self.v_trig(v)).0
    }
    fn param_of(&self, p: DVec3, seed: Option<DVec2>) -> (DVec2, f64) {
        let wrap = |d: f64| d - (d / TAU).round() * TAU;
        let mut q = self.uv(p);
        if let Some(s) = seed {
            q.x = s.x + wrap(q.x - s.x);
            if self.torus() && self.radius > 0.0 {
                q.y = s.y + wrap(q.y - s.y);
            }
        }
        (q, (GridSurface::at(self, q.x, q.y) - p).length())
    }
}

/// [`mesh_grid`], or why it declines.
fn mesh_grid_why(edges: &[MeshEdge], face: &MeshFace, s: &impl GridSurface, nodes: &[Option<EdgeNodes>], defl: Deflection) -> Result<FaceMesh, &'static str> {
    let mut seen = std::collections::HashSet::new();
    if !s.closed() && !face.loops.iter().flatten().all(|u| seen.insert(u.edge)) {
        return Err("a seam");
    }
    // An apex or pole has no parameters of its own.
    if face.loops.iter().flatten().any(|u| edges.get(u.edge as usize).is_none_or(|e| e.curve == Some(Curve::Degenerate))) {
        return Err("a degenerate edge");
    }
    let mut points: Vec<DVec3> = Vec::new();
    let mut rings: Vec<Vec<usize>> = Vec::with_capacity(face.loops.len());
    let mut edge_nodes = Vec::with_capacity(face.loops.len());
    for l in &face.loops {
        let (pts, per_use) = loop_nodes(edges, l, nodes).ok_or("an edge not meshed here")?;
        let base = points.len();
        rings.push((base..base + pts.len()).collect());
        edge_nodes.push(per_use.into_iter().map(|idx| idx.into_iter().map(|i| (base + i) as u32).collect()).collect());
        points.extend(pts);
    }
    // The boundary on the parameter plane, each node from the last's parameters.
    let ((u0, u1), (v0, v1)) = s.range();
    let off = (defl.linear * 0.25).max(1e-7);
    let mut uv: Vec<DVec2> = Vec::with_capacity(points.len());
    let mut seed = None;
    for p in &points {
        let (q, d) = s.param_of(*p, seed);
        if d.is_nan() || d > off || !q.is_finite() {
            return Err("a boundary node off the surface");
        }
        uv.push(q);
        seed = Some(q);
    }
    // Each loop closes on the parameter plane (none round a closed surface's period).
    for r in &rings {
        let (q, _) = s.param_of(points[r[0]], Some(uv[r[r.len() - 1]]));
        if (q - uv[r[0]]).length() > 1e-6 * (1.0 + q.length()) {
            return Err("a loop round the surface");
        }
    }
    let (lo, hi) = uv.iter().fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(a, b), q| (a.min(*q), b.max(*q)));
    if !(hi.x > lo.x && hi.y > lo.y) {
        return Err("a flat parameter box");
    }
    // The steps along u (v): the knots inside the box, each span cut for its largest second derivative.
    let samples = 5;
    let steps = |along_u: bool| -> Result<Vec<f64>, &'static str> {
        let (a, b, knots) = if along_u { (lo.x, hi.x, s.breaks(true)) } else { (lo.y, hi.y, s.breaks(false)) };
        let (c0, c1) = if along_u { (lo.y, hi.y) } else { (lo.x, hi.x) };
        let mut br = vec![a];
        br.extend(knots.iter().copied().filter(|&k| k > a && k < b));
        br.push(b);
        br.dedup_by(|x, y| (*x - *y).abs() <= 1e-12 * (b - a));
        // Each span's largest step; the nodes then spread by the steps' density over the whole range (not
        // one at every knot: a thread's flank, flat across but for its knots, took a row of nodes there).
        let mut hs: Vec<f64> = Vec::with_capacity(br.len());
        for w in br.windows(2) {
            let (mut d2, mut d1) = (0.0f64, f64::MAX);
            for i in 0..samples {
                for j in 0..samples {
                    let t = w[0] + (w[1] - w[0]) * i as f64 / (samples - 1) as f64;
                    let c = c0 + (c1 - c0) * j as f64 / (samples - 1) as f64;
                    let d = if along_u { s.derivs(t, c) } else { s.derivs(c, t) };
                    let (first, second) = if along_u { (d[1], d[3]) } else { (d[2], d[5]) };
                    d2 = d2.max(second.length());
                    d1 = d1.min(first.length());
                }
            }
            let span = w[1] - w[0];
            let mut h = span;
            if d2 > 0.0 {
                h = h.min((8.0 * defl.linear / d2).sqrt());
                if d1 > 0.0 {
                    h = h.min(defl.angular * d1 / d2);
                }
            }
            hs.push(h.max(span * 1e-4));
        }
        // How many steps up to each break, and in all.
        let mut at = vec![0.0];
        for (w, h) in br.windows(2).zip(&hs) {
            at.push(at[at.len() - 1] + (w[1] - w[0]) / h);
        }
        let total = at[at.len() - 1];
        let n = total.ceil().clamp(1.0, 1e5) as usize;
        let mut out = vec![a];
        let mut k = 0;
        for i in 1..n {
            let c = total * i as f64 / n as f64;
            while k + 2 < at.len() && at[k + 1] < c {
                k += 1;
            }
            let f = ((c - at[k]) / (at[k + 1] - at[k]).max(1e-300)).clamp(0.0, 1.0);
            out.push(br[k] + (br[k + 1] - br[k]) * f);
        }
        out.push(b);
        Ok(out)
    };
    let (mut us, mut vs) = (steps(true)?, steps(false)?);
    // Scale: the mean first derivatives over the box.
    let (mut su, mut sv, mut n) = (0.0, 0.0, 0.0);
    for i in 0..samples {
        for j in 0..samples {
            let d = s.derivs(lo.x + (hi.x - lo.x) * i as f64 / (samples - 1) as f64, lo.y + (hi.y - lo.y) * j as f64 / (samples - 1) as f64);
            su += d[1].length();
            sv += d[2].length();
            n += 1.0;
        }
    }
    let (su, sv) = (su / n, sv / n);
    if su.is_nan() || sv.is_nan() || su <= 0.0 || sv <= 0.0 {
        return Err("no derivatives");
    }
    // No step more than ten times the other way's smallest (on the surface): no long slivers across a flat way
    // (three times put a row of nodes along a thread's flank, its 0.5 mm across flat, twice BRepMesh's
    // triangles).
    let smallest = |t: &[f64], scale: f64| t.windows(2).map(|w| (w[1] - w[0]) * scale).fold(f64::MAX, f64::min);
    let refine = |t: &mut Vec<f64>, scale: f64, cap: f64| {
        let mut out = vec![t[0]];
        for w in t.windows(2) {
            let n = ((w[1] - w[0]) * scale / cap).ceil().clamp(1.0, 4096.0) as usize;
            out.extend((1..=n).map(|k| w[0] + (w[1] - w[0]) * k as f64 / n as f64));
        }
        *t = out;
    };
    let (mu, mv) = (smallest(&us, su), smallest(&vs, sv));
    refine(&mut us, su, 10.0 * mv);
    refine(&mut vs, sv, 10.0 * mu);
    // Past the limit, thinned evenly both ways to it: a display mesh coarser than the deflections ask rather than
    // BRepMesh's (half an hour on a 512 × 512 pole displacement map).
    const MAX_NODES: usize = 400_000;
    if us.len() * vs.len() > MAX_NODES {
        let f = ((us.len() * vs.len()) as f64 / MAX_NODES as f64).sqrt();
        let thin = |t: &mut Vec<f64>| {
            let last = t.len() - 1;
            let n = ((last as f64 / f).floor() as usize).max(1);
            *t = (0..=n).map(|k| t[k * last / n]).collect();
        };
        thin(&mut us);
        thin(&mut vs);
    }
    let scaled = |q: DVec2| DVec2::new(q.x * su, q.y * sv);
    // Interior grid nodes: inside the boundary (even-odd), at least a third of the local step from it. Each
    // row's crossings of the boundary once, and the boundary's segments in cells (as wide as the largest such
    // distance) to look for one close by: not every segment for every node.
    let segs: Vec<(DVec2, DVec2)> = rings.iter().flat_map(|r| (0..r.len()).map(|k| (scaled(uv[r[k]]), scaled(uv[r[(k + 1) % r.len()]])))).collect();
    let gap = |t: &[f64], i: usize, scale: f64| (t[i + 1] - t[i - 1]) * 0.5 * scale;
    let du: Vec<f64> = (1..us.len().saturating_sub(1)).map(|i| gap(&us, i, su)).collect();
    let dv: Vec<f64> = (1..vs.len().saturating_sub(1)).map(|j| gap(&vs, j, sv)).collect();
    let cell = du.iter().chain(&dv).fold(0.0f64, |m, d| m.max(*d)) / 3.0;
    let mut cells: std::collections::HashMap<(i64, i64), Vec<u32>> = std::collections::HashMap::new();
    if cell > 0.0 {
        let key = |q: DVec2| ((q.x / cell).floor() as i64, (q.y / cell).floor() as i64);
        for (k, &(a, b)) in segs.iter().enumerate() {
            // Samples half a cell apart: a node within a cell of the segment is within two cells of one.
            let n = ((b - a).length() / (0.5 * cell)).ceil().min(1e6) as usize;
            let mut last = None;
            for i in 0..=n {
                let c = key(a + (b - a) * (i as f64 / n.max(1) as f64));
                if last != Some(c) {
                    cells.entry(c).or_default().push(k as u32);
                    last = Some(c);
                }
            }
        }
        for l in cells.values_mut() {
            l.dedup();
        }
    }
    let close = |q: DVec2, r: f64| -> bool {
        let (cx, cy) = ((q.x / cell).floor() as i64, (q.y / cell).floor() as i64);
        (cx - 2..=cx + 2).any(|x| {
            (cy - 2..=cy + 2).any(|y| {
                cells.get(&(x, y)).is_some_and(|l| {
                    l.iter().any(|&k| {
                        let (a, b) = segs[k as usize];
                        let d = b - a;
                        let t = ((q - a).dot(d) / d.length_squared().max(1e-300)).clamp(0.0, 1.0);
                        (a + d * t - q).length() < r
                    })
                })
            })
        })
    };
    let mut interior = Vec::new();
    let mut crossings: Vec<f64> = Vec::new();
    for (j, &v) in vs.iter().enumerate().skip(1).take(vs.len().saturating_sub(2)) {
        if v <= v0 || v >= v1 {
            continue;
        }
        let y = v * sv;
        crossings.clear();
        crossings.extend(segs.iter().filter(|(a, b)| (a.y > y) != (b.y > y)).map(|(a, b)| a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y)));
        crossings.sort_by(f64::total_cmp);
        for (i, &u) in us.iter().enumerate().skip(1).take(us.len().saturating_sub(2)) {
            if u <= u0 || u >= u1 {
                continue;
            }
            let sq = DVec2::new(u * su, y);
            // Inside: an odd number of crossings past it.
            let past = crossings.len() - crossings.partition_point(|&x| x <= sq.x);
            if past % 2 == 1 && !close(sq, du[i - 1].min(dv[j - 1]) / 3.0) {
                interior.push(DVec2::new(u, v));
            }
        }
    }
    let nb = uv.len();
    let all: Vec<cdt::Pt> = uv.iter().chain(&interior).map(|q| scaled(*q)).map(|q| cdt::Pt { x: q.x, y: q.y }).collect();
    let inner: Vec<usize> = (nb..nb + interior.len()).collect();
    let triangles = cdt::triangulate_with(&all, &rings, &inner).ok_or("the triangulation failed")?;
    // Each node's point and derivatives once (a big face's on several threads); normals ∂u × ∂v (at a degenerate
    // point, its triangles').
    let nodes: Vec<DVec2> = uv.iter().chain(&interior).copied().collect();
    let mut d1 = vec![[DVec3::ZERO; 3]; nodes.len()];
    for (k, d) in in_parallel(nodes.len(), 8192, |k| s.d1(nodes[k].x, nodes[k].y)) {
        d1[k] = d;
    }
    points.extend(d1[nb..].iter().map(|d| d[0]));
    let mut normals: Vec<DVec3> = d1.iter().map(|d| d[1].cross(d[2]).normalize_or_zero()).collect();
    if normals.contains(&DVec3::ZERO) {
        let mut acc = vec![DVec3::ZERO; normals.len()];
        for t in &triangles {
            let (a, b, c) = (points[t[0] as usize], points[t[1] as usize], points[t[2] as usize]);
            let n = (b - a).cross(c - a);
            for &k in t {
                acc[k as usize] += n;
            }
        }
        for (n, a) in normals.iter_mut().zip(acc) {
            if *n == DVec3::ZERO {
                *n = a.normalize_or_zero();
            }
        }
    }
    Ok(FaceMesh { points, normals, triangles, edge_nodes })
}

#[cfg(test)]
mod tests;
