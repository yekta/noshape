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
    /// OCCT's surface of revolution: [`MeshNurbs::curves`]`[curve]` (in a plane through the axis) turned by u
    /// round the line through `origin` along unit `axis`; v is the curve's parameter.
    Revolution { curve: u32, origin: DVec3, axis: DVec3 },
    /// Any other surface: the face is declined (BRepMesh meshes it), its loops tell which edges it is on.
    Other,
}

/// The B-spline curves (edges' [`Curve::Nurbs`]) and surfaces ([`MeshSurface::Nurbs`]) of a body.
#[derive(Clone, Copy, Debug, Default)]
pub struct MeshNurbs<'a> {
    pub curves: &'a [NurbsCurve],
    pub surfaces: &'a [NurbsSurface],
    /// Per face, per loop, per use: its edge's curve on a B-spline face (in `curves`, (u, v, 0) at the edge's own
    /// parameters), where the kernel gave one.
    pub face_pcurves: &'a [Vec<Vec<Option<u32>>>],
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
        Some(Curve::Ellipse { center, normal, xdir, major, minor, .. }) => {
            // Steps of its parameter by its bending where they are (half the deflections, as a circle's): a chord's
            // sag under `|P''| h² / 8`, a step's turn (`a b / |P'|²` per unit of t) under the angle; spread by their
            // density over the range, as a B-spline face's grid steps.
            let arc = edge.t1 - edge.t0;
            if !(arc > 0.0 && arc <= std::f64::consts::TAU + 1e-9 && minor > 1e-7 && major >= minor) {
                return None;
            }
            let (lin, ang) = (0.5 * defl.linear, 0.5 * defl.angular);
            let density = |t: f64| {
                let (c, s) = (t.cos(), t.sin());
                let second = (major * major * c * c + minor * minor * s * s).sqrt();
                let turn = major * minor / (major * major * s * s + minor * minor * c * c);
                let h = (8.0 * lin / second).sqrt().min(ang / turn).max(1e-7 / major);
                1.0 / h
            };
            const SAMPLES: usize = 256;
            let dt = arc / SAMPLES as f64;
            let mut at = Vec::with_capacity(SAMPLES + 1);
            at.push(0.0);
            for k in 0..SAMPLES {
                // (The denser end of each sample: the bending peaks at the axes' ends.)
                let d = density(edge.t0 + k as f64 * dt).max(density(edge.t0 + (k + 1) as f64 * dt));
                at.push(at[k] + d * dt);
            }
            let total = at[SAMPLES];
            let n = (total.ceil().min(1e6) as usize).max(3);
            let ydir = normal.cross(xdir);
            let mut points = Vec::with_capacity(n + 1);
            let mut params = Vec::with_capacity(n + 1);
            let mut k = 0;
            for i in 0..=n {
                let t = if i == n {
                    edge.t1
                } else {
                    let c = total * i as f64 / n as f64;
                    while k + 1 < SAMPLES && at[k + 1] < c {
                        k += 1;
                    }
                    edge.t0 + (k as f64 + ((c - at[k]) / (at[k + 1] - at[k]).max(1e-300)).clamp(0.0, 1.0)) * dt
                };
                params.push(t);
                points.push(match i {
                    0 => a,
                    _ if i == n => b,
                    _ => center + major * t.cos() * xdir + minor * t.sin() * ydir,
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
            let mut too_long = None;
            while !fits(u, u + dir * h) && tries < 40 {
                too_long = Some(h);
                h *= 0.7;
                tries += 1;
            }
            // Back up towards the step that failed: the longest that fits, within a few percent (cut by 0.7 alone,
            // steps were up to 30 % short: a thread's helical edges 18 % finer than BRepMesh's, and its faces as
            // many triangles more).
            if let Some(mut long) = too_long {
                for _ in 0..4 {
                    let mid = 0.5 * (h + long);
                    if fits(u, u + dir * mid) {
                        h = mid;
                    } else {
                        long = mid;
                    }
                }
                // No sliver left at the end of the span: the rest in two equal steps.
                let left = (to - u).abs();
                if left - h < 0.3 * h && left > h {
                    h = 0.5 * left;
                }
            }
            u = if (to - (u + dir * h)) * dir <= 1e-12 * span { to } else { u + dir * h };
            params.push(u);
            points.push(c.at(u));
            // (A million: a displacement map's rim at 1024² wiggles past 100 000 nodes; an edge's grow with the map's
            // side, not its area.)
            if params.len() > 1_000_000 || (params.len() % 4096 == 0 && crate::cancelled()) {
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
    // A face closed both ways (an o-ring's torus: every edge a seam of its own, used twice by it and never drawn
    // as an edge) has its seams split as its rows, by the deflections themselves, not half of them as an edge: it
    // took 144 × 144 rows, four times BRepMesh's triangles. (A fillet's band keeps BRepMesh's split of its seam:
    // its rims are its neighbours' too.)
    let mut owner: Vec<Option<(usize, bool)>> = vec![None; edges.len()];
    for (i, f) in faces.iter().enumerate() {
        for u in f.loops.iter().flatten() {
            if let Some(o) = owner.get_mut(u.edge as usize) {
                *o = match *o {
                    None => Some((i, false)),
                    Some((j, _)) if j == i => Some((i, true)),
                    _ => Some((usize::MAX, false)),
                };
            }
        }
    }
    let own_seam = |e: usize| owner[e].is_some_and(|(i, twice)| twice && i != usize::MAX);
    let closed: Vec<bool> = faces.iter().map(|f| f.loops.iter().flatten().all(|u| own_seam(u.edge as usize))).collect();
    let seam = |e: usize| own_seam(e) && owner[e].is_some_and(|(i, _)| closed[i]);
    let seam_defl = Deflection { linear: 2.0 * defl.linear, angular: 2.0 * defl.angular };
    // In parallel only for many (a small part's edges sample in microseconds, less than starting threads).
    let run = if used.len() < 512 { used.len().max(1) } else { 64 };
    for (e, n) in in_parallel(used.len(), run, |k| edge_nodes_with(vertices, &edges[used[k]], if seam(used[k]) { seam_defl } else { defl }, nurbs)) {
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
        let mesh = |k: usize| mesh_face(vertices, edges, &faces[run[k].1], edge_nodes, defl, nurbs, nurbs.face_pcurves.get(run[k].1).map(|v| v.as_slice()));
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
            if a >= n || crate::cancelled() {
                return got;
            }
            got.extend((a..(a + run).min(n)).map(|i| (i, f(i))));
        }
    };
    // (Each worker with the caller's cancel flag and the kernel's state, as the Boolean's.)
    let (flag, state) = (crate::thread_cancel(), crate::thread_state());
    let worker = || {
        crate::set_thread_cancel(flag);
        let back = state.map(|(h, s)| (h, (h.enter)(s)));
        let got = work();
        if let Some((h, b)) = back {
            (h.leave)(b);
        }
        got
    };
    std::thread::scope(|sc| {
        let others: Vec<_> = (1..threads).map(|_| sc.spawn(worker)).collect();
        let mut all = work();
        for h in others {
            all.extend(h.join().unwrap_or_default());
        }
        all
    })
}

/// One face, its edges' nodes from `nodes`.
fn mesh_face(vertices: &[DVec3], edges: &[MeshEdge], face: &MeshFace, nodes: &[Option<EdgeNodes>], defl: Deflection, nurbs: MeshNurbs, pcurves: Option<&[Vec<Option<u32>>]>) -> Option<FaceMesh> {
    match face.surface {
        MeshSurface::Nurbs { index } => {
            let s = nurbs.surfaces.get(index as usize)?;
            let pc = pcurves.map(|p| (p, nurbs.curves));
            mesh_ruled(edges, face, s, nodes, pc).or_else(|| mesh_grid(edges, face, s, nodes, defl, pc))
        }
        MeshSurface::Plane { origin, normal, xdir } => mesh_plane(vertices, edges, face, origin, normal, xdir, nodes),
        // A strip's rows, else (a piece bounded by other curves: a knurl's lands) a grid as a B-spline's.
        MeshSurface::Cylinder { origin, axis, xdir, direct, radius } => mesh_revolved(edges, face, &Revolved::new(origin, axis, xdir, direct, radius, 0.0, 0.0), nodes, defl),
        MeshSurface::Cone { origin, axis, xdir, direct, radius, angle } => mesh_revolved(edges, face, &Revolved::new(origin, axis, xdir, direct, radius, angle, 0.0), nodes, defl),
        // A spindle or horn torus (the tube through its axis) isn't meshed here.
        MeshSurface::Torus { origin, axis, xdir, direct, radius, minor } if minor > 0.0 && radius > minor => mesh_revolved(edges, face, &Revolved::new(origin, axis, xdir, direct, radius, 0.0, minor), nodes, defl),
        MeshSurface::Torus { .. } => None,
        MeshSurface::Sphere { origin, axis, xdir, direct, radius } if radius > 0.0 => mesh_revolved(edges, face, &Revolved::new(origin, axis, xdir, direct, 0.0, 0.0, radius), nodes, defl),
        MeshSurface::Sphere { .. } => None,
        MeshSurface::Revolution { curve, origin, axis } => {
            let s = Revolution::new(nurbs.curves.get(curve as usize)?, origin, axis)?;
            mesh_grid(edges, face, &s, nodes, defl, pcurves.map(|p| (p, nurbs.curves)))
        }
        MeshSurface::Other => None,
    }
}

/// A curve in a plane through an axis turned round it (OCCT's surface of revolution): `origin + R(u) (C(v) - origin)`.
struct Revolution<'a> {
    c: &'a NurbsCurve,
    origin: DVec3,
    axis: DVec3,
    /// The curve's plane's direction away from the axis (u 0's).
    e0: DVec3,
    vknots: Vec<f64>,
}

impl<'a> Revolution<'a> {
    fn new(c: &'a NurbsCurve, origin: DVec3, axis: DVec3) -> Option<Revolution<'a>> {
        let axis = axis.try_normalize()?;
        let off = |p: DVec3| (p - origin) - axis * (p - origin).dot(axis);
        let e0 = c.poles.iter().map(|&p| off(p)).max_by(|a, b| a.length().total_cmp(&b.length()))?.try_normalize()?;
        let mut vknots = c.knots.clone();
        vknots.dedup();
        Some(Revolution { c, origin, axis, e0, vknots })
    }
    /// `x` (a vector from the axis' origin) turned by `u` round the axis.
    fn turn(&self, x: DVec3, (s, c): (f64, f64)) -> DVec3 {
        x * c + self.axis.cross(x) * s + self.axis * (self.axis.dot(x) * (1.0 - c))
    }
}

impl GridSurface for Revolution<'_> {
    fn degenerate_edges_run_along_u(&self) -> bool {
        true
    }
    fn name(&self) -> &'static str {
        "revolution"
    }
    fn closed(&self) -> bool {
        true
    }
    fn range(&self) -> ((f64, f64), (f64, f64)) {
        ((f64::NEG_INFINITY, f64::INFINITY), self.c.range())
    }
    fn breaks(&self, along_u: bool) -> &[f64] {
        if along_u { &[] } else { &self.vknots }
    }
    fn derivs(&self, u: f64, v: f64) -> [DVec3; 6] {
        let t = u.sin_cos();
        let [c, c1, c2] = self.c.derivs(v);
        let p = self.origin + self.turn(c - self.origin, t);
        let su = self.axis.cross(p - self.origin);
        let sv = self.turn(c1, t);
        [p, su, sv, self.axis.cross(su), self.axis.cross(sv), self.turn(c2, t)]
    }
    fn at(&self, u: f64, v: f64) -> DVec3 {
        self.origin + self.turn(self.c.at(v) - self.origin, u.sin_cos())
    }
    fn param_of(&self, p: DVec3, seed: Option<DVec2>) -> (DVec2, f64) {
        let wrap = |d: f64| d - (d / TAU).round() * TAU;
        // u: the point's angle round the axis from the curve's plane; v: its place turned back into that plane.
        let x = p - self.origin;
        let r = x - self.axis * x.dot(self.axis);
        let mut u = if r.length() > 0.0 { self.e0.cross(r).dot(self.axis).atan2(self.e0.dot(r)) } else { seed.map_or(0.0, |s| s.x) };
        if let Some(s) = seed {
            u = s.x + wrap(u - s.x);
        }
        let back = self.origin + self.turn(x, (-u).sin_cos());
        let (v, _) = self.c.param_of(back);
        (DVec2::new(u, v), (GridSurface::at(self, u, v) - p).length())
    }
}

/// A B-spline straight across v (degree 1, one span: an extruded curve, a gear's tooth flank) in one loop of two rims
/// (constant v) and two straight sides (constant u, a node at each end): the rims' nodes zipped by u, no grid. The
/// grid put ~1000 rows along the straight way of cf-drive-train-generator's flanks (590,000 triangles each; core 11-06).
fn mesh_ruled(edges: &[MeshEdge], face: &MeshFace, s: &NurbsSurface, nodes: &[Option<EdgeNodes>], pcurves: Option<FaceCurves>) -> Option<FaceMesh> {
    if s.vdegree != 1 || s.nv != 2 || face.loops.len() != 1 || face.loops[0].len() != 4 || s.nu == 0 {
        return None;
    }
    // Its rims each other's translates (an extrusion): the straight lines between them are the surface's, so the
    // rims' own nodes are fine enough across it.
    let along = s.poles[1] - s.poles[0];
    let size = s.poles.iter().fold(0.0f64, |m, p| m.max(p.abs().max_element())).max(along.length());
    if (0..s.nu).any(|i| (s.poles[i * 2 + 1] - s.poles[i * 2] - along).length() > 1e-9 * size) {
        return None;
    }
    let (points, per_use) = loop_nodes(edges, &face.loops[0], nodes)?;
    let edge_nodes: Vec<Vec<Vec<u32>>> = vec![per_use.iter().map(|idx| idx.iter().map(|&i| i as u32).collect()).collect()];
    let uv = uv_from_curves(face, &edge_nodes, nodes, pcurves?, s, &points)?;
    let ((u0, u1), (v0, v1)) = GridSurface::range(s);
    let (tu, tv) = (1e-9 * (u1 - u0).abs().max(1.0), 1e-9 * (v1 - v0).abs().max(1.0));
    // Rims by their v, each its nodes by u; the sides straight (their two ends only).
    let mut rims: Vec<(f64, Vec<usize>)> = Vec::new();
    let mut sides = 0;
    for idx in &per_use {
        let (lo, hi) = idx.iter().fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(lo, hi), &i| (lo.min(uv[i]), hi.max(uv[i])));
        if hi.y - lo.y <= tv {
            rims.push((lo.y, idx.clone()));
        } else if hi.x - lo.x <= tu && idx.len() == 2 {
            sides += 1;
        } else {
            return None;
        }
    }
    if rims.len() != 2 || sides != 2 {
        return None;
    }
    rims.sort_by(|a, b| a.0.total_cmp(&b.0));
    for r in &mut rims {
        r.1.sort_by(|&a, &b| uv[a].x.total_cmp(&uv[b].x));
    }
    let (a, b) = (&rims[0].1, &rims[1].1);
    if (uv[a[0]].x - uv[b[0]].x).abs() > tu || (uv[a[a.len() - 1]].x - uv[b[b.len() - 1]].x).abs() > tu {
        return None;
    }
    // Zipped by u, counter-clockwise on (u, v) (about Su × Sv).
    let mut triangles = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i + 1 < a.len() || j + 1 < b.len() {
        let next_a = i + 1 < a.len() && (j + 1 >= b.len() || uv[a[i + 1]].x <= uv[b[j + 1]].x);
        if next_a {
            triangles.push([a[i] as u32, a[i + 1] as u32, b[j] as u32]);
            i += 1;
        } else {
            triangles.push([a[i] as u32, b[j + 1] as u32, b[j] as u32]);
            j += 1;
        }
    }
    let normals = uv.iter().map(|q| {
        let d = s.d1(q.x, q.y);
        d[1].cross(d[2]).normalize_or_zero()
    });
    let normals: Vec<DVec3> = normals.collect();
    if normals.contains(&DVec3::ZERO) {
        return None;
    }
    Some(FaceMesh { points, normals, triangles, edge_nodes })
}

fn mesh_revolved(edges: &[MeshEdge], face: &MeshFace, s: &Revolved, nodes: &[Option<EdgeNodes>], defl: Deflection) -> Option<FaceMesh> {
    mesh_strip(edges, face, s, nodes, defl).or_else(|| mesh_grid(edges, face, s, nodes, defl, None))
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
    // walked, then the next one in the given order; a turn that can't go round every use is taken back (a rim cut in
    // two at the seam's vertex: its other piece first leads back to a dead end, a Replace face's displaced tube).
    let mut from: Vec<(u32, usize)> = uses.iter().enumerate().map(|(k, u)| Some((ends(u)?.0, k))).collect::<Option<_>>()?;
    from.sort_unstable();
    let n = uses.len();
    let mut used = vec![false; n];
    used[0] = true;
    let mut order = vec![0usize];
    // The other choices left at each step after the first (the last one tried next).
    let mut alts: Vec<Vec<usize>> = Vec::new();
    let mut budget = 4 * n * n + 64;
    loop {
        budget = budget.checked_sub(1)?;
        let i = *order.last()?;
        let end = ends(&uses[i])?.1;
        if order.len() == n {
            if end == ends(&uses[order[0]])?.0 {
                break;
            }
        } else {
            let at = from.partition_point(|&(v, _)| v < end);
            let mut next: Vec<usize> = from[at..].iter().take_while(|&&(v, _)| v == end).map(|&(_, k)| k).filter(|&k| !used[k]).collect();
            next.sort_by_key(|&k| std::cmp::Reverse((uses[k].edge == uses[i].edge, (k + n - i) % n)));
            if let Some(k) = next.pop() {
                alts.push(next);
                used[k] = true;
                order.push(k);
                continue;
            }
        }
        // Back to the last step with another choice.
        loop {
            let mut a = alts.pop()?;
            let k = order.pop()?;
            used[k] = false;
            if let Some(k) = a.pop() {
                alts.push(a);
                used[k] = true;
                order.push(k);
                break;
            }
        }
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
/// plane, nodes inside it at the corners of cells cut as fine as the surface's bending there asks (chord `|S''·N| h²
/// / 8` within the linear deflection, turn within the angular: by the region, not the face's tightest spot), then all
/// triangulated by [`cdt::triangulate_with`] on the plane scaled by the surface's mean first derivatives and stretched
/// so a typical cell is square. Declined: a seam (an edge twice in the face), a boundary node off the surface, a
/// degenerate parameterisation.
/// A face's edges' curves on it (per loop, per use) and the curves they index.
type FaceCurves<'a> = (&'a [Vec<Option<u32>>], &'a [NurbsCurve]);

fn mesh_grid(edges: &[MeshEdge], face: &MeshFace, s: &impl GridSurface, nodes: &[Option<EdgeNodes>], defl: Deflection, pcurves: Option<FaceCurves>) -> Option<FaceMesh> {
    let r = mesh_grid_why(edges, face, s, nodes, defl, pcurves);
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
    /// Whether an apex's or pole's degenerate edge runs along its u by its own parameters (a cone's, sphere's or
    /// torus's, as the bridge reads them).
    fn degenerate_edges_run_along_u(&self) -> bool {
        false
    }
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
    /// Closed round u: a node the iteration from `seed` can't reach across the seam (it stops short, as far off as
    /// the step from the seed's point) is found afresh, and one on the seam is at u0 or u1, the one nearer `seed`.
    /// (Not every node a little off the surface: an intersection curve's are, and each searched afresh took 12-54 ms
    /// on a displacement map's tube.)
    fn param_of(&self, p: DVec3, seed: Option<DVec2>) -> (DVec2, f64) {
        let mut r = NurbsSurface::param_of(self, p, seed);
        let Some(s) = seed else { return r };
        // (Open too: on a bumpy surface the iteration from the last node can settle in a dip beside it, a displaced
        // top's rim.)
        let step = (p - NurbsSurface::at(self, s.x, s.y)).length();
        let off = (1e-9 * (1.0 + p.abs().max_element())).max(0.25 * step);
        if r.1 > off {
            // Near the seed first (a dip beside it: a few spans' samples, not the whole surface's, 14 ms a node on a
            // million-pole displacement map), then the whole surface if that's off too.
            let near = self.param_near(p, s, 2.0);
            if near.1 < r.1 {
                r = near;
            }
            if r.1 > off {
                let fresh = NurbsSurface::param_of(self, p, None);
                if fresh.1 < r.1 {
                    r = fresh;
                }
            }
        }
        if !self.closed_in_u() {
            return r;
        }
        let ((u0, u1), _) = NurbsSurface::range(self);
        let eps = 1e-9 * (u1 - u0);
        // Stopped on the seam but not on the surface there: the clamped iteration from a node on the seam can't
        // step inside (a swept hose's rim, its first nodes from the seam's corner all at u1): from just inside
        // either side.
        if r.1 > 1e-9 * (1.0 + p.abs().max_element()) && ((r.0.x - u0).abs() <= eps || (r.0.x - u1).abs() <= eps) {
            let h = 1e-3 * (u1 - u0);
            for inside in [u1 - h, u0 + h] {
                let t = NurbsSurface::param_of(self, p, Some(DVec2::new(inside, r.0.y)));
                if t.1 < r.1 {
                    r = t;
                }
            }
        }
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
    fn degenerate_edges_run_along_u(&self) -> bool {
        true
    }
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
/// The boundary nodes' parameters from their edges' curves on the face, each use's at its nodes' edge parameters: exact
/// and without a search (a million-pole displacement map's 100 000 rim nodes took seconds to project, some into a dip
/// of its bumps). None unless every use has one, every node lands in the surface's range and the ones checked on it.
#[allow(clippy::too_many_arguments)]
fn uv_from_curves(face: &MeshFace, edge_nodes: &[Vec<Vec<u32>>], nodes: &[Option<EdgeNodes>], (pc, curves): FaceCurves, s: &impl GridSurface, points: &[DVec3]) -> Option<Vec<DVec2>> {
    let mut uv = vec![DVec2::NAN; points.len()];
    for (l, uses) in face.loops.iter().enumerate() {
        for (k, u) in uses.iter().enumerate() {
            let c = curves.get((*pc.get(l)?.get(k)?)? as usize)?;
            let n = nodes.get(u.edge as usize)?.as_ref()?;
            let ids = edge_nodes.get(l)?.get(k)?;
            if ids.len() != n.params.len() || ids.is_empty() {
                return None;
            }
            // Each use's own nodes (its last along it is the next use's first).
            let last = ids.len() - 1;
            for (j, (&id, &t)) in ids.iter().zip(&n.params).enumerate() {
                if if u.reversed { j > 0 } else { j < last } {
                    let q = c.at(t);
                    *uv.get_mut(id as usize)? = DVec2::new(q.x, q.y);
                }
            }
        }
    }
    let ((u0, u1), (v0, v1)) = s.range();
    let (eu, ev) = (1e-9 * (u1 - u0).abs().max(1.0), 1e-9 * (v1 - v0).abs().max(1.0));
    if uv.iter().any(|q| !q.is_finite() || q.x < u0.min(u1) - eu || q.x > u0.max(u1) + eu || q.y < v0.min(v1) - ev || q.y > v0.max(v1) + ev) {
        return None;
    }
    let every = (points.len() / 64).max(1);
    for i in (0..points.len()).step_by(every) {
        if (s.at(uv[i].x, uv[i].y) - points[i]).length() > 1e-5 * (1.0 + points[i].abs().max_element()) {
            return None;
        }
    }
    Some(uv)
}

fn mesh_grid_why(edges: &[MeshEdge], face: &MeshFace, s: &impl GridSurface, nodes: &[Option<EdgeNodes>], defl: Deflection, pcurves: Option<FaceCurves>) -> Result<FaceMesh, &'static str> {
    let mut seen = std::collections::HashSet::new();
    if !s.closed() && !face.loops.iter().flatten().all(|u| seen.insert(u.edge)) {
        return Err("a seam");
    }
    if face.loops.iter().flatten().any(|u| edges.get(u.edge as usize).is_none()) {
        return Err("an edge not meshed here");
    }
    let degenerate = |u: &Use| edges[u.edge as usize].curve == Some(Curve::Degenerate);
    // An apex or pole has no parameters of its own: its edge's (along u) where the surface reads them so, else its
    // curve's on the face (below).
    let has_pole = face.loops.iter().flatten().any(degenerate);
    if has_pole && !s.degenerate_edges_run_along_u() && pcurves.is_none() {
        return Err("a degenerate edge");
    }
    let mut points: Vec<DVec3> = Vec::new();
    let mut rings: Vec<Vec<usize>> = Vec::with_capacity(face.loops.len());
    let mut edge_nodes: Vec<Vec<Vec<u32>>> = Vec::with_capacity(face.loops.len());
    for l in &face.loops {
        let (pts, per_use) = loop_nodes(edges, l, nodes).ok_or("an edge not meshed here")?;
        let base = points.len();
        rings.push((base..base + pts.len()).collect());
        edge_nodes.push(per_use.into_iter().map(|idx| idx.into_iter().map(|i| (base + i) as u32).collect()).collect());
        points.extend(pts);
    }
    // A pole's nodes: their u (the edge's parameters), one point but kept apart below.
    let mut pole_u: Vec<Option<f64>> = vec![None; points.len()];
    for (l, uses) in face.loops.iter().enumerate() {
        for (k, u) in uses.iter().enumerate() {
            if !degenerate(u) {
                continue;
            }
            let n = nodes.get(u.edge as usize).and_then(|n| n.as_ref()).ok_or("an edge not meshed here")?;
            let ids = &edge_nodes[l][k];
            if ids.len() != n.params.len() {
                return Err("a degenerate edge");
            }
            for (&id, &t) in ids.iter().zip(&n.params) {
                pole_u[id as usize] = Some(t);
            }
        }
    }
    // The boundary on the parameter plane, each node from the last's parameters.
    let dbg_t = std::time::Instant::now();
    let dbg = std::env::var_os("NOSHAPE_MESH_DEBUG").is_some();
    let ((u0, u1), (v0, v1)) = s.range();
    let off = (defl.linear * 0.25).max(1e-7);
    let from_curves = pcurves.and_then(|pc| uv_from_curves(face, &edge_nodes, nodes, pc, s, &points));
    let from_curves_used = from_curves.is_some();
    let mut uv: Vec<DVec2> = Vec::with_capacity(points.len());
    if let Some(u) = from_curves {
        uv = u;
    } else {
        let mut seed = None;
        for (k, p) in points.iter().enumerate() {
            if k % 1024 == 1023 && crate::cancelled() {
                return Err("cancelled");
            }
            let (mut q, mut d) = s.param_of(*p, seed);
            // Also from the last step repeated (a rim's coarse nodes, a display preview's, are far apart: from the last
            // node's parameters alone the iteration settled in a dip of a displacement map's bumps).
            if k >= 2 && d > 1e-9 * (1.0 + p.abs().max_element()) {
                let ahead = uv[k - 1] * 2.0 - uv[k - 2];
                let (q2, d2) = s.param_of(*p, Some(ahead));
                if d2 < d {
                    (q, d) = (q2, d2);
                }
            }
            if d.is_nan() || d > off || !q.is_finite() {
                return Err("a boundary node off the surface");
            }
            if let Some(u) = pole_u[k].filter(|_| s.degenerate_edges_run_along_u()) {
                q.x = u;
            }
            // A closed surface's first node on its seam: on the side its next node goes on from (a rim running towards
            // smaller u from u0 crossed the whole period: a Replace face's displaced tube).
            if k == 0 && s.closed() && points.len() > 1 {
                let ((a, b), _) = s.range();
                let eps = 1e-9 * (b - a).abs().max(1.0);
                if (q.x - a).abs() <= eps || (q.x - b).abs() <= eps {
                    let other = DVec2::new(if (q.x - a).abs() <= eps { b } else { a }, q.y);
                    let step = |c: DVec2| (s.param_of(points[1], Some(c)).0 - c).length();
                    if step(other) < step(q) {
                        q = other;
                    }
                }
            }
            // On a closed surface a node on its seam is at either end of the period: the end nearest the last node (the
            // projection gave a seam node u 0 where the rim before ran up to u 1 at a displacement map's coarse preview,
            // and the loop didn't close: BRepMesh took the face, 650 s at 512²).
            if k > 0 && s.closed() {
                let ((a, b), _) = s.range();
                let (period, eps) = (b - a, 1e-9 * (b - a).abs().max(1.0));
                let last = uv[k - 1].x;
                for c in [q.x - period, q.x + period] {
                    if c >= a - eps && c <= b + eps && (c - last).abs() < (q.x - last).abs() {
                        q.x = c;
                    }
                }
            }
            uv.push(q);
            seed = Some(q);
        }
        // A run of nodes on a closed surface's seam (up a seam edge) on the side of the node after it (or before): the side
        // the loop goes on from.
        if s.closed() {
            let ((a, b), _) = s.range();
            let eps = 1e-9 * (b - a).abs().max(1.0);
            let on_seam = |u: f64| (u - a).abs() <= eps || (u - b).abs() <= eps;
            for r in &rings {
                let mut k = 0;
                while k < r.len() {
                    if !on_seam(uv[r[k]].x) {
                        k += 1;
                        continue;
                    }
                    let start = k;
                    while k < r.len() && on_seam(uv[r[k]].x) {
                        k += 1;
                    }
                    let next = if k < r.len() { Some(uv[r[k]].x) } else { None };
                    let before = if start > 0 { Some(uv[r[start - 1]].x) } else { r.iter().rev().map(|&i| uv[i].x).find(|&u| !on_seam(u)) };
                    if let Some(u) = next.or(before) {
                        let side = if (u - b).abs() < (u - a).abs() { b } else { a };
                        for &i in &r[start..k] {
                            uv[i].x = side;
                        }
                    }
                }
            }
        }
    }
    // A closed surface: each loop by continuity across the seam (a step over half a period is one across it: a
    // swept hose's wall walked its seam at u0 and then its rim towards smaller u, so the rim came out on the other
    // side), then shifted by whole periods into the surface's range.
    let periodic = s.closed() && u0.is_finite() && u1.is_finite() && u1 > u0;
    if periodic {
        let period = u1 - u0;
        for r in &rings {
            for k in 1..r.len() {
                let d = uv[r[k]].x - uv[r[k - 1]].x;
                uv[r[k]].x -= (d / period).round() * period;
            }
            // (A seam's nodes are a little either side of it: the nearest whole period.)
            let min = r.iter().fold(f64::MAX, |m, &k| m.min(uv[k].x));
            let shift = ((min - u0) / period).round() * period;
            for &k in r {
                uv[k].x -= shift;
            }
        }
    }
    // Each loop closes on the parameter plane (none round a closed surface's period).
    for r in &rings {
        // (A pole's u is its edge's, not its point's.)
        if pole_u[r[0]].is_some() {
            continue;
        }
        let last = uv[r[r.len() - 1]];
        let (mut q, _) = s.param_of(points[r[0]], Some(last));
        if periodic {
            q.x -= ((q.x - last.x) / (u1 - u0)).round() * (u1 - u0);
        }
        if (q - uv[r[0]]).length() > 1e-6 * (1.0 + q.length()) {
            return Err("a loop round the surface");
        }
        if periodic && r.iter().any(|&k| uv[k].x > u1 + 1e-4 * (u1 - u0) || uv[k].x < u0 - 1e-4 * (u1 - u0)) {
            return Err("a loop across the seam");
        }
    }
    // Boundary nodes closer than the triangulation tells apart (an edge split up to a slight crease in steps of
    // nanometres: a swept hose's seam) stand for one: the one at the least point, so a seam's two uses keep the
    // same.
    let diag = points.iter().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(a, b), q| (a.min(*q), b.max(*q)));
    let near = 1e-6 * (diag.1 - diag.0).length();
    let least = |a: DVec3, b: DVec3| a.to_array().partial_cmp(&b.to_array()) == Some(std::cmp::Ordering::Less);
    for r in rings.iter_mut() {
        let n = r.len();
        // Start after a gap, so no cluster wraps round the ring's start.
        let Some(start) = (0..n).find(|&k| (points[r[k]] - points[r[(k + n - 1) % n]]).length() >= near) else { continue };
        let walk: Vec<usize> = (0..n).map(|k| r[(start + k) % n]).collect();
        let mut kept: Vec<usize> = Vec::with_capacity(n);
        let mut cluster: Vec<usize> = Vec::new();
        for (k, &i) in walk.iter().enumerate() {
            cluster.push(i);
            let next = walk.get(k + 1).copied();
            if next.is_none_or(|j| (points[j] - points[i]).length() >= near || pole_u[i].is_some() || pole_u[j].is_some()) {
                kept.push(*cluster.iter().reduce(|a, b| if least(points[*b], points[*a]) { b } else { a }).expect("one"));
                cluster.clear();
            }
        }
        if kept.len() >= 3 {
            *r = kept;
        }
    }
    let (mut lo, mut hi) = uv.iter().fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(a, b), q| (a.min(*q), b.max(*q)));
    // A seam's nodes a little past it don't widen the grid: a step of 1e-7 between them and the first knot made the
    // other way's ten times that (a hose wall's 30000 rows).
    if periodic {
        (lo.x, hi.x) = (lo.x.max(u0), hi.x.min(u1));
    }
    if !(hi.x > lo.x && hi.y > lo.y) {
        return Err("a flat parameter box");
    }
    let nb_points = points.len();
    // Scale: the mean first derivatives over the box.
    let samples = 5;
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
    // The steps the surface allows (scaled) along u and v, on a raster over the box (four cells a knot span, up to
    // 512 a way and 262144 in all): each cell's from the bending at its and its neighbours' corners, then graded, each
    // growing at most half the distance from a tighter one. Steps by the region, not the face's tightest spot for all
    // of it (a blade's edge set a tensor grid's step across the whole face: cf-setback-fillet 454k triangles against
    // BRepMesh's 549).
    let (w, h) = ((hi.x - lo.x) * su, (hi.y - lo.y) * sv);
    let cells_along = |along_u: bool| {
        let (a, b) = if along_u { (lo.x, hi.x) } else { (lo.y, hi.y) };
        let knots = s.breaks(along_u).iter().filter(|&&k| k > a && k < b).count();
        (4 * (knots + 1)).clamp(8, 512)
    };
    let (mut nu, mut nv) = (cells_along(true), cells_along(false));
    const MAX_RASTER: usize = 1 << 18;
    if nu * nv > MAX_RASTER {
        let f = ((nu * nv) as f64 / MAX_RASTER as f64).sqrt();
        (nu, nv) = (((nu as f64 / f) as usize).max(8), ((nv as f64 / f) as usize).max(8));
    }
    let corner = |k: usize| DVec2::new(lo.x + (hi.x - lo.x) * (k % (nu + 1)) as f64 / nu as f64, lo.y + (hi.y - lo.y) * (k / (nu + 1)) as f64 / nv as f64);
    let mut allowed = vec![(f64::MAX, f64::MAX); (nu + 1) * (nv + 1)];
    let mut normal = vec![DVec3::ZERO; (nu + 1) * (nv + 1)];
    for (k, d) in in_parallel((nu + 1) * (nv + 1), 1024, |k| s.derivs(corner(k).x, corner(k).y)) {
        let step = |first: f64, second: f64| {
            if second.is_nan() || second <= 0.0 {
                return f64::MAX;
            }
            let mut h = (8.0 * defl.linear / second).sqrt();
            // (Not at an apex or pole, where the rows meet whatever the step.)
            if first > 1e-9 * (1.0 + d[0].abs().max_element()) {
                h = h.min(defl.angular * first / second);
            }
            h
        };
        // The surface's bending: the second derivatives across it (along it they only speed the parameters up: a
        // draft's B-spline wall, flat, took 3,792 triangles against BRepMesh's 32); at a pole, all of them.
        // (None where the parameters all but collapse, a tip or pole: its direction there is noise.)
        let cross = d[1].cross(d[2]);
        let n = if cross.length() > 1e-6 * d[1].length() * d[2].length() { cross.normalize() } else { DVec3::ZERO };
        normal[k] = n;
        let across = |x: DVec3| if n == DVec3::ZERO { x.length() } else { x.dot(n).abs() };
        let (mut hu, mut hv) = (step(d[1].length(), across(d[3])), step(d[2].length(), across(d[5])));
        // A twist (a saddle, flat both ways): cells square on the surface, their sides' product within it.
        let twist = across(d[4]);
        if twist > 0.0 && d[1].length() > 0.0 && d[2].length() > 0.0 {
            let c = 4.0 * defl.linear / twist;
            hu = hu.min((c * d[2].length() / d[1].length()).sqrt());
            hv = hv.min((c * d[1].length() / d[2].length()).sqrt());
        }
        allowed[k] = (hu * su, hv * sv);
    }
    let least = (w.max(h) * 1e-4).max(1e-12);
    let (cw, ch) = (w / nu as f64, h / nv as f64);
    // How far the normal turns across each raster cell, each way (a twist turns it too, where the second derivatives
    // along the rows don't see it): the steps that turn it by the angular deflection.
    let turned: Vec<(f64, f64)> = (0..nu * nv)
        .map(|c| {
            let (i, j) = (c % nu, c / nu);
            let at = |di: usize, dj: usize| normal[(j + dj) * (nu + 1) + i + di];
            let angle = |a: DVec3, b: DVec3| if a == DVec3::ZERO || b == DVec3::ZERO { 0.0 } else { a.angle_between(b) };
            let (tu, tv) = (angle(at(0, 0), at(1, 0)).max(angle(at(0, 1), at(1, 1))), angle(at(0, 0), at(0, 1)).max(angle(at(1, 0), at(1, 1))));
            let step = |size: f64, turn: f64| if turn > 0.0 { size * defl.angular / turn } else { f64::MAX };
            (step(cw, tu), step(ch, tv))
        })
        .collect();
    let mut field: Vec<(f64, f64)> = (0..nu * nv)
        .map(|c| {
            // (Its own corners and cells and its neighbours': a peak between samples.)
            let (i, j) = (c % nu, c / nu);
            let mut x_y = (w, h);
            for jj in j.saturating_sub(1)..=(j + 2).min(nv) {
                for ii in i.saturating_sub(1)..=(i + 2).min(nu) {
                    let a = allowed[jj * (nu + 1) + ii];
                    x_y = (x_y.0.min(a.0), x_y.1.min(a.1));
                    if ii < nu && jj < nv {
                        let t = turned[jj * nu + ii];
                        x_y = (x_y.0.min(t.0), x_y.1.min(t.1));
                    }
                }
            }
            (x_y.0.max(least), x_y.1.max(least))
        })
        .collect();
    // The face's typical cell shape (a long bend's: a sliver along its flat length, as BRepMesh's), the plane it is
    // triangulated on stretched so that one is square; a cell no more than ten times longer either way than that (a
    // boundary node amid cells far longer fanned triangles across several: a wavy tube's seam, 11° of turn).
    let mut shapes: Vec<f64> = field.iter().map(|f| f.1 / f.0).collect();
    let mid = shapes.len() / 2;
    let typical = *shapes.select_nth_unstable_by(mid, f64::total_cmp).1;
    for f in &mut field {
        *f = (f.0.min(10.0 * f.1 / typical), f.1.min(10.0 * typical * f.0));
    }
    let flat = |q: DVec2| DVec2::new(q.x * su, q.y * sv / typical);
    for _ in 0..4 {
        let mut changed = false;
        for pass in 0..4 {
            for jj in 0..nv {
                for ii in 0..nu {
                    let (i, j) = (if pass % 2 == 0 { ii } else { nu - 1 - ii }, if pass < 2 { jj } else { nv - 1 - jj });
                    let mut f = field[j * nu + i];
                    for (di, dj, d) in [(-1i64, 0i64, cw), (1, 0, cw), (0, -1, ch), (0, 1, ch)] {
                        let (a, b) = (i as i64 + di, j as i64 + dj);
                        if a < 0 || b < 0 || a >= nu as i64 || b >= nv as i64 {
                            continue;
                        }
                        let g = field[b as usize * nu + a as usize];
                        f = (f.0.min(g.0 + 0.5 * d), f.1.min(g.1 + 0.5 * d));
                    }
                    if f != field[j * nu + i] {
                        field[j * nu + i] = f;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    // The cells: the box cut into as many as each part's steps ask (its raster cells' least), until none asks more.
    // Past the limit, no more cuts: a mesh coarser than the deflections ask rather than BRepMesh's (half an hour on a
    // 512 × 512 pole displacement map).
    const MAX_CELLS: usize = 400_000;
    // (Over the limit, every step longer by the same factor: coarser evenly, not cut short somewhere.)
    let need: f64 = field.iter().map(|f| (cw / f.0) * (ch / f.1)).sum();
    if need > 0.4 * MAX_CELLS as f64 {
        let k = (need / (0.4 * MAX_CELLS as f64)).sqrt();
        for f in &mut field {
            *f = (f.0 * k, f.1 * k);
        }
    }
    let raster = |a: f64, b: f64, lo: f64, hi: f64, n: usize| {
        let t = |x: f64| (x - lo) / (hi - lo) * n as f64;
        let first = (t(a).floor().max(0.0) as usize).min(n - 1);
        let last = ((t(b).ceil() as usize).saturating_sub(1)).clamp(first, n - 1);
        first..=last
    };
    let asks = |c: &[f64; 4]| {
        let mut f = (f64::MAX, f64::MAX);
        for j in raster(c[2], c[3], lo.y, hi.y, nv) {
            for i in raster(c[0], c[1], lo.x, hi.x, nu) {
                let g = field[j * nu + i];
                f = (f.0.min(g.0), f.1.min(g.1));
            }
        }
        f
    };
    let mut leaves: Vec<[f64; 4]> = Vec::new();
    let mut queue = std::collections::VecDeque::from([[lo.x, hi.x, lo.y, hi.y]]);
    while let Some(c) = queue.pop_front() {
        if leaves.len() % 4096 == 4095 && crate::cancelled() {
            return Err("cancelled");
        }
        let (fx, fy) = asks(&c);
        let cuts = |size: f64, step: f64| if size > step * 1.0001 { (size / step).ceil().min(16.0) as usize } else { 1 };
        let (cu, cv) = (cuts((c[1] - c[0]) * su, fx), cuts((c[3] - c[2]) * sv, fy));
        if (cu == 1 && cv == 1) || leaves.len() + queue.len() + cu * cv > MAX_CELLS {
            leaves.push(c);
            continue;
        }
        let at = |a: f64, b: f64, n: usize, k: usize| if k == n { b } else { a + (b - a) * k as f64 / n as f64 };
        for j in 0..cv {
            for i in 0..cu {
                queue.push_back([at(c[0], c[1], cu, i), at(c[0], c[1], cu, i + 1), at(c[2], c[3], cv, j), at(c[2], c[3], cv, j + 1)]);
            }
        }
    }
    // The cells' corners inside the box, each with its least cell's smaller side (on that plane). One corner reached by two
    // cells' cuts differs in its last digits: snapped to a grid four times the triangulation's (2^22 across), so it
    // is one node (two that close fail the triangulation).
    let snap = w.max(h / typical) / f64::from(1u32 << 20);
    let mut corners: std::collections::HashMap<(i64, i64), f64> = std::collections::HashMap::new();
    for c in &leaves {
        let side = ((c[1] - c[0]) * su).min((c[3] - c[2]) * sv / typical);
        for (u, v) in [(c[0], c[2]), (c[1], c[2]), (c[0], c[3]), (c[1], c[3])] {
            if u <= lo.x || u >= hi.x || v <= lo.y || v >= hi.y || u <= u0 || u >= u1 || v <= v0 || v >= v1 {
                continue;
            }
            let q = flat(DVec2::new(u, v));
            let e = corners.entry(((q.x / snap).round() as i64, (q.y / snap).round() as i64)).or_insert(side);
            *e = e.min(side);
        }
    }
    let mut candidates: Vec<(DVec2, f64)> = corners.into_iter().map(|((x, y), side)| (DVec2::new(x as f64 * snap / su, y as f64 * snap * typical / sv), side)).collect();
    // By rows (each row's boundary crossings once), in a fixed order whatever the hashing.
    candidates.sort_by(|a, b| a.0.y.total_cmp(&b.0.y).then(a.0.x.total_cmp(&b.0.x)));
    // Interior nodes: inside the boundary (even-odd), at least a third of their cells' side from it. The boundary's
    // segments in cells (a typical node's side) to look for one close by: not every segment for every node.
    let segs: Vec<(DVec2, DVec2)> = rings.iter().flat_map(|r| (0..r.len()).map(|k| (flat(uv[r[k]]), flat(uv[r[(k + 1) % r.len()]])))).collect();
    let mut sides: Vec<f64> = candidates.iter().map(|c| c.1).collect();
    let cell = if sides.is_empty() {
        0.0
    } else {
        let mid = sides.len() / 2;
        *sides.select_nth_unstable_by(mid, f64::total_cmp).1 / 3.0
    };
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
        let reach = 2 + (r / cell).ceil().min(64.0) as i64;
        (cx - reach..=cx + reach).any(|x| {
            (cy - reach..=cy + reach).any(|y| {
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
    if dbg {
        eprintln!("mesh grid: {} boundary nodes ({}), {} cells ({} x {} raster), {} corners, {} boundary segments {:?}", nb_points, if from_curves_used { "their curves on the face" } else { "projected" }, leaves.len(), nu, nv, candidates.len(), segs.len(), dbg_t.elapsed());
    }
    let mut interior = Vec::new();
    let mut crossings: Vec<f64> = Vec::new();
    let mut row = f64::NAN;
    for (k, &(q, side)) in candidates.iter().enumerate() {
        if k % 65536 == 65535 && crate::cancelled() {
            return Err("cancelled");
        }
        let sq = flat(q);
        if sq.y != row {
            row = sq.y;
            crossings.clear();
            crossings.extend(segs.iter().filter(|(a, b)| (a.y > row) != (b.y > row)).map(|(a, b)| a.x + (row - a.y) * (b.x - a.x) / (b.y - a.y)));
            crossings.sort_by(f64::total_cmp);
        }
        // Inside: an odd number of crossings past it.
        let past = crossings.len() - crossings.partition_point(|&x| x <= sq.x);
        if past % 2 == 1 && !close(sq, side / 3.0) {
            interior.push(q);
        }
    }
    let nb = uv.len();
    if dbg {
        eprintln!("mesh grid: {} interior nodes {:?}", interior.len(), dbg_t.elapsed());
    }
    let all: Vec<cdt::Pt> = uv.iter().chain(&interior).map(|q| flat(*q)).map(|q| cdt::Pt { x: q.x, y: q.y }).collect();
    let inner: Vec<usize> = (nb..nb + interior.len()).collect();
    let mut triangles = cdt::triangulate_with(&all, &rings, &inner).ok_or("the triangulation failed")?;
    // (A triangle with two of a pole's nodes is a point and a segment: none.)
    if has_pole {
        let at_pole = |k: u32| pole_u.get(k as usize).is_some_and(|u| u.is_some());
        triangles.retain(|t| t.iter().filter(|&&k| at_pole(k)).count() < 2);
    }
    if dbg {
        eprintln!("mesh grid: triangulated {:?}", dbg_t.elapsed());
    }
    // Each node's point and derivatives once (a big face's on several threads); normals ∂u × ∂v (at a degenerate
    // point, its triangles').
    let nodes: Vec<DVec2> = uv.iter().chain(&interior).copied().collect();
    let mut d1 = vec![[DVec3::ZERO; 3]; nodes.len()];
    for (k, d) in in_parallel(nodes.len(), 8192, |k| s.d1(nodes[k].x, nodes[k].y)) {
        d1[k] = d;
    }
    points.extend(d1[nb..].iter().map(|d| d[0]));
    let mut normals: Vec<DVec3> = d1.iter().map(|d| d[1].cross(d[2]).normalize_or_zero()).collect();
    // (A pole's ∂u is all but nothing there, ∂u × ∂v turned any way: the surface's normal a hair inside the face
    // from it, which tends to the pole's.)
    let vmid = 0.5 * (lo.y + hi.y);
    for (k, u) in pole_u.iter().enumerate() {
        if u.is_some() {
            let q = nodes[k];
            let d = s.d1(q.x, q.y + (vmid - q.y).signum() * 1e-6 * (hi.y - lo.y));
            normals[k] = d[1].cross(d[2]).normalize_or_zero();
        }
    }
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
