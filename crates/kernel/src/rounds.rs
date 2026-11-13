//! Rounds taken off a solid whose kept faces next to them are planes (a rounded box, shelled or split: cylinder rounds,
//! sphere and torus corners), without OCCT's Defeaturing: each kept face beside a round is made again on its own plane,
//! its corners where the planes of the faces round it meet, a round's straight edge standing for the round's other
//! support; its curved edges (a round's end, a corner's) add nothing. The other faces stay; all are sewn again.
//! cf-part-draft's Part Draft 2 (Reapply fillet): ~190 ms of Defeaturing per half.

use crate::{Curve, History, HistoryRecord, OpResult, Plane, Relation, Shape, ShapeType, Surface};
use glam::DVec3;

const TOL: f64 = 1e-7;

/// The plane a polygon's side lies on: the face it comes from, a point of it and its unit normal.
type Side = (usize, (DVec3, DVec3));

/// `del` (faces of `solid`) taken away as [`crate::delete_faces`] does, when each is a cylinder, sphere or torus and
/// every kept face beside one is a plane whose new outline the planes round it give; `None` otherwise.
pub fn delete_planar_rounds(solid: &Shape, del: &[Shape]) -> Option<OpResult> {
    if solid.shape_type() != ShapeType::Solid || del.is_empty() {
        return None;
    }
    let faces = solid.faces();
    let mut gone = vec![false; faces.len()];
    for f in del {
        gone[solid.index_of(f, ShapeType::Face)?] = true;
    }
    let surfaces: Vec<Surface> = faces.iter().map(|f| f.face_surface().ok()).collect::<Option<_>>()?;
    if gone.iter().zip(&surfaces).any(|(g, s)| *g && !matches!(s, Surface::Cylinder { .. } | Surface::Sphere { .. } | Surface::Torus { .. })) {
        return None;
    }
    // Cylinders about parallel axes only: a prism's rounds, which the kernel's own prism case edits in place.
    let axes: Vec<DVec3> = (0..faces.len()).filter(|&f| gone[f]).filter_map(|f| if let Surface::Cylinder { axis, .. } = surfaces[f] { Some(axis.normalize()) } else { None }).collect();
    if axes.len() == del.len() && axes.iter().all(|a| a.cross(axes[0]).length() < 1e-9) {
        return None;
    }
    let plane = |f: usize| match &surfaces[f] {
        Surface::Plane(p) => Some((p.origin, p.normal.normalize())),
        _ => None,
    };
    let edges = solid.edges();
    let ef = solid.edge_faces().ok()?;
    let lines: Vec<Option<(DVec3, DVec3)>> = edges.iter().map(|e| matches!(e.edge_info().ok()?.curve, Curve::Line { .. }).then(|| e.endpoints().ok()).flatten()).collect();
    let other = |ei: usize, f: usize| -> Option<usize> {
        match ef.get(ei)?.as_slice() {
            [a, b] if *a as usize == f && *b as usize != f => Some(*b as usize),
            [a, b] if *b as usize == f && *a as usize != f => Some(*a as usize),
            _ => None,
        }
    };
    // A cylinder round's other support: the kept face across its other straight edges, tangent to it.
    let support = |d: usize, from: usize| -> Option<usize> {
        let Surface::Cylinder { origin, axis, radius, .. } = surfaces[d] else { return None };
        let axis = axis.normalize();
        let mut found = None;
        for (ei, fs) in ef.iter().enumerate() {
            if !fs.contains(&(d as u32)) {
                continue;
            }
            let Some((a, b)) = lines[ei] else { continue };
            let Some(o) = other(ei, d) else { continue };
            if o == from {
                continue;
            }
            let (_, n) = plane(o)?;
            if gone[o] || found.is_some_and(|x| x != o) {
                return None;
            }
            // Along the axis, the plane tangent there (a cut along it is no support).
            let m = (a + b) * 0.5;
            let radial = (m - origin) - axis * (m - origin).dot(axis);
            if (b - a).normalize_or_zero().cross(axis).length() > 1e-9 || (radial.length() - radius).abs() > TOL.max(1e-9 * radius) || n.dot(radial.normalize()).abs() < 1.0 - 1e-9 {
                return None;
            }
            found = Some(o);
        }
        found
    };
    // A new corner is a vertex already there or by a round (within four radii of its box): one made far off (planes
    // near parallel, or meeting past other faces) is left to OCCT.
    let reach = 4.0 * (0..faces.len()).filter(|&f| gone[f]).map(|f| match surfaces[f] {
        Surface::Cylinder { radius, .. } | Surface::Sphere { radius, .. } => radius,
        Surface::Torus { minor_radius, .. } => minor_radius,
        _ => 0.0,
    }).fold(0.0, f64::max);
    let boxes: Vec<crate::BoundingBox> = (0..faces.len()).filter(|&f| gone[f]).map(|f| faces[f].bounding_box(false).ok()).collect::<Option<_>>()?;
    let vertices: Vec<DVec3> = solid.vertices().iter().map(|v| v.vertex_point().ok()).collect::<Option<_>>()?;
    let allowed = |p: DVec3| {
        vertices.iter().any(|v| v.distance(p) <= 1e-6) || boxes.iter().any(|b| (b.min - DVec3::splat(reach)).cmple(p).all() && p.cmple(b.max + DVec3::splat(reach)).all())
    };
    let touches = |f: usize| ef.iter().any(|fs| fs.contains(&(f as u32)) && fs.iter().any(|&o| gone[o as usize]));
    let kept: Vec<usize> = (0..faces.len()).filter(|&f| !gone[f]).collect();
    let mut rebuilt: Vec<Shape> = Vec::with_capacity(kept.len());
    let mut changed = vec![false; faces.len()];
    for &f in &kept {
        if !touches(f) {
            rebuilt.push(faces[f].clone());
            continue;
        }
        let (origin, normal) = plane(f)?;
        changed[f] = true;
        let mut wires: Vec<(f64, Shape)> = Vec::new();
        for w in faces[f].wires() {
            let ids: Vec<usize> = w.edges().iter().map(|e| solid.index_of(e, ShapeType::Edge)).collect::<Option<_>>()?;
            let size = w.bounding_box(false).ok()?.diagonal();
            if !ids.iter().any(|&ei| other(ei, f).is_some_and(|o| gone[o])) {
                wires.push((size, w));
                continue;
            }
            let outline = new_outline(&ids, &edges, origin, normal, &|ei| {
                let o = other(ei, f)?;
                match (gone[o], lines[ei]) {
                    (false, _) => plane(o).map(|p| Some((o, p))),
                    (true, Some(_)) => support(o, f).and_then(|s| plane(s).map(|p| Some((s, p)))),
                    (true, None) => Some(None),
                }
            })?;
            if !outline.iter().all(|&p| allowed(p)) {
                return None;
            }
            wires.push((size, crate::make_polyline(&outline, true).ok()?));
        }
        let outer = (0..wires.len()).max_by(|&a, &b| wires[a].0.total_cmp(&wires[b].0))?;
        let holes: Vec<Shape> = wires.iter().enumerate().filter(|(i, _)| *i != outer).map(|(_, w)| w.1.clone()).collect();
        rebuilt.push(crate::make_face(&Plane::from_normal(origin, normal), &wires[outer].1, &holes).ok()?);
    }
    let sewn = crate::sew(&rebuilt, 1e-6, true).ok()?;
    if sewn.shape.shape_type() != ShapeType::Solid || sewn.shape.faces().len() != kept.len() || !sewn.shape.is_valid() {
        return None;
    }
    let mut records = Vec::with_capacity(faces.len());
    for (j, &f) in kept.iter().enumerate() {
        let [out] = sewn.history.modified(j as u32, ShapeType::Face, 0)[..] else { return None };
        let relation = if changed[f] { Relation::Modified } else { Relation::Kept };
        records.push(HistoryRecord { arg: 0, input_kind: ShapeType::Face, input_index: f as u32, relation, output_kind: ShapeType::Face, output_index: out });
    }
    for f in (0..faces.len()).filter(|&f| gone[f]) {
        records.push(HistoryRecord { arg: 0, input_kind: ShapeType::Face, input_index: f as u32, relation: Relation::Deleted, output_kind: ShapeType::Face, output_index: 0 });
    }
    Some(OpResult { shape: sewn.shape, history: History { records } })
}

/// A planar face's wire (`ids`, edges of the solid) as a polygon: its edges in order, each giving the plane its new
/// side lies on (`side_of`: `None` to give up, `Some(None)` for none), the corners where consecutive planes meet.
/// `None` where the planes don't meet in a point or a side would run backwards (a neighbour cut back).
fn new_outline(ids: &[usize], edges: &[Shape], origin: DVec3, normal: DVec3, side_of: &dyn Fn(usize) -> Option<Option<Side>>) -> Option<Vec<DVec3>> {
    // The edges chained end to end.
    let mut ends: Vec<(usize, DVec3, DVec3)> = ids.iter().map(|&ei| edges[ei].endpoints().ok().map(|(a, b)| (ei, a, b))).collect::<Option<_>>()?;
    let scale = ends.iter().map(|e| e.1.length().max(e.2.length())).fold(1.0, f64::max);
    let near = |p: DVec3, q: DVec3| p.distance(q) <= 1e-6 * scale;
    let mut chain = vec![ends.swap_remove(0)];
    while !ends.is_empty() {
        let tail = chain.last()?.2;
        let k = ends.iter().position(|e| near(e.1, tail) || near(e.2, tail))?;
        let (ei, a, b) = ends.swap_remove(k);
        chain.push(if near(a, tail) { (ei, a, b) } else { (ei, b, a) });
    }
    if !near(chain.last()?.2, chain[0].1) {
        return None;
    }
    // Each run of edges along one plane: the plane and the direction the run goes.
    let mut runs: Vec<(usize, (DVec3, DVec3), DVec3)> = Vec::new();
    for &(ei, a, b) in &chain {
        let Some((s, p)) = side_of(ei)? else { continue };
        match runs.last_mut() {
            Some(r) if r.0 == s => r.2 += b - a,
            _ => runs.push((s, p, b - a)),
        }
    }
    if runs.len() > 1 && runs[0].0 == runs.last()?.0 {
        let last = runs.pop()?;
        runs[0].2 += last.2;
    }
    if runs.len() < 3 {
        return None;
    }
    let n = runs.len();
    let mut corners = Vec::with_capacity(n);
    for j in 0..n {
        let (p, q) = (runs[j].1, runs[(j + 1) % n].1);
        corners.push(meet((origin, normal), p, q)?);
    }
    // Run j goes from corner j-1 to corner j, the way its edges went.
    for j in 0..n {
        let side = corners[j] - corners[(j + n - 1) % n];
        if side.length() <= TOL || side.dot(runs[j].2) <= 0.0 {
            return None;
        }
    }
    Some(corners)
}

/// The point on three planes (a point and a unit normal each); `None` where two are near parallel.
fn meet(a: (DVec3, DVec3), b: (DVec3, DVec3), c: (DVec3, DVec3)) -> Option<DVec3> {
    let det = a.1.dot(b.1.cross(c.1));
    if det.abs() < 1e-9 {
        return None;
    }
    let (da, db, dc) = (a.1.dot(a.0), b.1.dot(b.0), c.1.dot(c.0));
    Some((b.1.cross(c.1) * da + c.1.cross(a.1) * db + a.1.cross(b.1) * dc) / det)
}
