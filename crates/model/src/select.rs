//! Selection helpers behind Onshape's "Select ▸" context submenu and the
//! Create selection dialog: grow a seed face or edge into related entities
//! (tangent-connected faces, the edge loop of a face, tangent chains,
//! parallel edges, equal radius, all faces of the part).

use crate::reference::Reference;
use crate::regen::context::{Body, Context};
use glam::DVec3;
use noshape_kernel::{Curve, ShapeType, Surface};
use std::collections::{BTreeSet, VecDeque};

/// What to select relative to the seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Criterion {
    /// Faces connected to the seed face through tangent (smooth) edges.
    TangentFaces,
    /// The edges bounding the seed face.
    FaceLoop,
    /// Edges connected to the seed edge with tangent continuity.
    TangentEdges,
    /// Linear edges parallel to the seed edge.
    ParallelEdges,
    /// Circular edges / cylindrical faces with the seed's radius.
    EqualRadius,
    /// Every face of the seed's part.
    PartFaces,
}

impl Criterion {
    pub const FACE: [Criterion; 4] = [Criterion::TangentFaces, Criterion::FaceLoop, Criterion::EqualRadius, Criterion::PartFaces];
    pub const EDGE: [Criterion; 3] = [Criterion::TangentEdges, Criterion::ParallelEdges, Criterion::EqualRadius];

    pub fn label(self) -> &'static str {
        match self {
            Criterion::TangentFaces => "Tangent connected faces",
            Criterion::FaceLoop => "Loop/chain connected edges",
            Criterion::TangentEdges => "Tangent connected edges",
            Criterion::ParallelEdges => "Parallel edges",
            Criterion::EqualRadius => "Equal radius",
            Criterion::PartFaces => "All faces of part",
        }
    }
}

const TANGENT_COS: f64 = 0.9998; // ~1.1°

fn body_of<'a>(ctx: &'a Context, r: &Reference) -> Option<(&'a Body, ShapeType, usize)> {
    ctx.bodies.iter().find_map(|b| match r {
        Reference::Face(n) => b.find_faces(n).first().map(|i| (&**b, ShapeType::Face, *i)),
        Reference::Edge(n) => b.find_edges(n).first().map(|i| (&**b, ShapeType::Edge, *i)),
        _ => None,
    })
}

fn face_ref(b: &Body, i: usize) -> Option<Reference> {
    b.faces.get(i).map(|n| Reference::Face((**n).clone()))
}

fn edge_ref(b: &Body, i: usize) -> Option<Reference> {
    b.edges.get(i).map(|n| Reference::Edge((**n).clone()))
}

fn edge_mid(b: &Body, e: usize) -> Option<DVec3> {
    let s = b.shape.sub_shape(ShapeType::Edge, e).ok()?;
    let info = s.edge_info().ok()?;
    s.point_at((info.first_param + info.last_param) * 0.5).ok()
}

/// Radius of a circular edge or cylindrical face.
fn radius(b: &Body, kind: ShapeType, i: usize) -> Option<f64> {
    let s = b.shape.sub_shape(kind, i).ok()?;
    match kind {
        ShapeType::Edge => match s.edge_curve().ok()? {
            Curve::Circle { radius, .. } => Some(radius),
            _ => None,
        },
        _ => match s.face_surface().ok()? {
            Surface::Cylinder { radius, .. } | Surface::Sphere { radius, .. } => Some(radius),
            _ => None,
        },
    }
}

fn line_dir(b: &Body, e: usize) -> Option<DVec3> {
    match b.shape.sub_shape(ShapeType::Edge, e).ok()?.edge_curve().ok()? {
        Curve::Line { direction, .. } => Some(direction.normalize_or_zero()),
        _ => None,
    }
}

/// Whether the two faces of an edge meet smoothly.
fn smooth_edge(b: &Body, e: usize, fa: usize, fb: usize) -> bool {
    let Some(m) = edge_mid(b, e) else { return false };
    let n = |f: usize| b.shape.sub_shape(ShapeType::Face, f).ok().and_then(|s| s.normal_at_point(m).ok());
    match (n(fa), n(fb)) {
        (Some(a), Some(c)) => a.normalize_or_zero().dot(c.normalize_or_zero()).abs() > TANGENT_COS,
        _ => false,
    }
}

/// Entities related to `seed` by `criterion` (including the seed).
pub fn select_related(ctx: &Context, seed: &Reference, criterion: Criterion) -> Vec<Reference> {
    let Some((b, kind, i)) = body_of(ctx, seed) else { return Vec::new() };
    let edge_faces = b.shape.edge_faces().unwrap_or_default();
    let mut out: BTreeSet<usize> = BTreeSet::new();
    let faces = |set: BTreeSet<usize>| set.into_iter().filter_map(|f| face_ref(b, f)).collect::<Vec<_>>();
    let edges = |set: BTreeSet<usize>| set.into_iter().filter_map(|e| edge_ref(b, e)).collect::<Vec<_>>();
    match (criterion, kind) {
        (Criterion::TangentFaces, ShapeType::Face) => {
            let mut queue = VecDeque::from([i]);
            out.insert(i);
            while let Some(f) = queue.pop_front() {
                for (e, fs) in edge_faces.iter().enumerate() {
                    if fs.len() != 2 || !fs.contains(&(f as u32)) {
                        continue;
                    }
                    let other = if fs[0] as usize == f { fs[1] as usize } else { fs[0] as usize };
                    if !out.contains(&other) && smooth_edge(b, e, f, other) {
                        out.insert(other);
                        queue.push_back(other);
                    }
                }
            }
            faces(out)
        }
        (Criterion::FaceLoop, ShapeType::Face) => edges(edge_faces.iter().enumerate().filter(|(_, fs)| fs.contains(&(i as u32))).map(|(e, _)| e).collect()),
        (Criterion::PartFaces, _) => faces((0..b.faces.len()).collect()),
        (Criterion::EqualRadius, k) => {
            let Some(r) = radius(b, k, i) else { return vec![seed.clone()] };
            let n = if k == ShapeType::Edge { b.edges.len() } else { b.faces.len() };
            let same: BTreeSet<usize> = (0..n).filter(|j| radius(b, k, *j).is_some_and(|q| (q - r).abs() <= 1e-7_f64.max(r * 1e-6))).collect();
            if k == ShapeType::Edge { edges(same) } else { faces(same) }
        }
        (Criterion::ParallelEdges, ShapeType::Edge) => {
            let Some(d) = line_dir(b, i) else { return vec![seed.clone()] };
            edges((0..b.edges.len()).filter(|j| line_dir(b, *j).is_some_and(|q| q.dot(d).abs() > TANGENT_COS)).collect())
        }
        (Criterion::TangentEdges, ShapeType::Edge) => {
            let vertex_edges = b.shape.vertex_edges().unwrap_or_default();
            let ends = |e: usize| -> Vec<(usize, DVec3)> {
                let Ok(s) = b.shape.sub_shape(ShapeType::Edge, e) else { return Vec::new() };
                let Ok(info) = s.edge_info() else { return Vec::new() };
                let t0 = s.tangent_at(info.first_param).unwrap_or(DVec3::ZERO);
                let t1 = s.tangent_at(info.last_param).unwrap_or(DVec3::ZERO);
                let mut v = Vec::new();
                for (vi, es) in vertex_edges.iter().enumerate() {
                    if !es.contains(&(e as u32)) {
                        continue;
                    }
                    let Ok(p) = b.shape.sub_shape(ShapeType::Vertex, vi).and_then(|x| x.vertex_point()) else { continue };
                    let t = if p.distance(info.start) < p.distance(info.end) { t0 } else { t1 };
                    v.push((vi, t.normalize_or_zero()));
                }
                v
            };
            let mut queue = VecDeque::from([i]);
            out.insert(i);
            while let Some(e) = queue.pop_front() {
                for (vi, t) in ends(e) {
                    for &other in vertex_edges.get(vi).map(|v| v.as_slice()).unwrap_or(&[]) {
                        let other = other as usize;
                        if out.contains(&other) {
                            continue;
                        }
                        if ends(other).iter().any(|(v2, t2)| *v2 == vi && t2.dot(t).abs() > TANGENT_COS) {
                            out.insert(other);
                            queue.push_back(other);
                        }
                    }
                }
            }
            edges(out)
        }
        _ => vec![seed.clone()],
    }
}

/// Midpoint of an edge and the direction into the material (minus the
/// average of its two faces' normals), for fillet/chamfer drag handles.
pub fn edge_inward(ctx: &Context, edge: &Reference) -> Option<(DVec3, DVec3)> {
    let (b, kind, e) = body_of(ctx, edge)?;
    if kind != ShapeType::Edge {
        return None;
    }
    let m = edge_mid(b, e)?;
    let faces = b.shape.edge_faces().ok()?.get(e)?.clone();
    let mut n = DVec3::ZERO;
    for f in faces {
        if let Ok(s) = b.shape.sub_shape(ShapeType::Face, f as usize)
            && let Ok(v) = s.normal_at_point(m)
        {
            n += v.normalize_or_zero();
        }
    }
    let dir = -n.normalize_or_zero();
    (dir.length() > 0.5).then_some((m, dir))
}

/// Centroid of a face and its outward normal there.
pub fn face_anchor(ctx: &Context, face: &Reference) -> Option<(DVec3, DVec3)> {
    let (b, kind, f) = body_of(ctx, face)?;
    if kind != ShapeType::Face {
        return None;
    }
    let s = b.shape.sub_shape(ShapeType::Face, f).ok()?;
    let c = s.centroid().ok()?;
    let p = s.project_point(c).map(|x| x.point).unwrap_or(c);
    let n = s.normal_at_point(p).ok()?.normalize_or_zero();
    Some((p, n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regen::{RegenOptions, Regenerator};

    fn plate() -> std::sync::Arc<Context> {
        let ps = crate::demo::plate_studio();
        let mut r = Regenerator::new();
        let res = r.regenerate(&ps, &Default::default(), &noshape_units::UnitPrefs::default(), &RegenOptions::default());
        res.context
    }

    #[test]
    fn grows_seeds_into_related_entities() {
        let ctx = plate();
        let b = &ctx.bodies[0];
        // The planar top face: its loop has several edges; part faces = all.
        let top = (0..b.faces.len())
            .find(|i| b.shape.sub_shape(ShapeType::Face, *i).ok().and_then(|f| f.face_plane().ok().flatten()).is_some_and(|p| p.unit_normal().z > 0.99))
            .expect("top face");
        let seed = face_ref(b, top).unwrap();
        let lp = select_related(&ctx, &seed, Criterion::FaceLoop);
        assert!(lp.len() >= 4, "{}", lp.len());
        assert_eq!(select_related(&ctx, &seed, Criterion::PartFaces).len(), b.faces.len());
        // A straight edge of the plate has parallel siblings.
        let line = (0..b.edges.len()).find(|e| line_dir(b, *e).is_some()).expect("linear edge");
        let par = select_related(&ctx, &edge_ref(b, line).unwrap(), Criterion::ParallelEdges);
        assert!(par.len() >= 2, "{}", par.len());
        // A circular hole edge: equal-radius siblings include it.
        if let Some(c) = (0..b.edges.len()).find(|e| radius(b, ShapeType::Edge, *e).is_some()) {
            let eq = select_related(&ctx, &edge_ref(b, c).unwrap(), Criterion::EqualRadius);
            assert!(eq.contains(&edge_ref(b, c).unwrap()));
        }
    }
}
