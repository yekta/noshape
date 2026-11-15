//! An Offset chamfer of a chain of edges of one plane face whose other faces run straight along the plane's normal (a
//! tube's end): the tool the part loses, as Parasolid's chamfer there (c2-impeller-loft's Chamfer Stevenrohr Halterung,
//! live q2985c: a 53.6 mm arc and two 0.31 mm spline edges meeting it at 8.9°, 3.5 mm at 20°, which OCCT's chamfer
//! can't build). Each edge's chamfer is ruled from the edge, `d` down its side, to its offset `e` in along the plane;
//! where the chain stops at an edge of the plane face the chamfer runs on until the surface of the face across it.

use crate::boolean::{self, BooleanOp, BooleanOptions};
use crate::construct::{make_cylinder, make_line, make_wire};
use crate::error::Result;
use crate::geom::{Plane, Surface};
use crate::ops::{LoftOptions, loft};
use crate::shape::{Shape, ShapeType};
use glam::DVec3;

/// The tool of [`prism_rim_chamfer`], and a point on each edge's chamfer face (in the edges' order).
pub struct RimChamfer {
    pub tool: Shape,
    pub on_chamfer: Vec<DVec3>,
}

/// The tool cutting an Offset chamfer of `edges` (indices in `shape`'s edge map), `d` down the side faces and `e` in
/// along the plane face they share; None where it doesn't apply: no one plane face, a side not running straight along
/// its normal for `d`, a closed edge, an open chain whose ends stop at faces other than one cylinder or plane.
pub fn prism_rim_chamfer(shape: &Shape, edges: &[usize], d: f64, e: f64) -> Result<Option<RimChamfer>> {
    if edges.is_empty() || !(d > 0.0) || !(e > 0.0) {
        return Ok(None);
    }
    let faces = shape.faces();
    let all = shape.edges();
    let ef = shape.edge_faces()?;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let tol = 1e-7 * scale;
    // The plane face all the edges are on.
    let mut common = ef.get(edges[0]).cloned().unwrap_or_default();
    for &x in edges {
        let Some(fs) = ef.get(x).filter(|f| f.len() == 2 && f[0] != f[1]) else { return Ok(None) };
        common.retain(|f| fs.contains(f));
    }
    let [pf] = common[..] else { return Ok(None) };
    let plane_face = &faces[pf as usize];
    if plane_face.face_plane()?.is_none() {
        return Ok(None);
    }
    let chain: Vec<&Shape> = edges.iter().map(|&x| &all[x]).collect();
    let mid = |s: &Shape| -> Result<DVec3> {
        let i = s.edge_info()?;
        s.point_at(0.5 * (i.first_param + i.last_param))
    };
    let plane_point = mid(chain[0])?;
    let n = plane_face.normal_at_point(plane_point)?.normalize();
    // Each side runs straight along the normal for d: its points d down from the edge's lie on it.
    for (k, &x) in edges.iter().enumerate() {
        let side = &faces[ef[x].iter().copied().find(|&f| f != pf).unwrap_or(pf) as usize];
        let i = chain[k].edge_info()?;
        for j in 0..=4 {
            let p = chain[k].point_at(i.first_param + (i.last_param - i.first_param) * j as f64 / 4.0)?;
            let q = p - n * d;
            if side.normal_at_point(p)?.dot(n).abs() > 1e-6 || side.project_point(q)?.point.distance(q) > tol || !side.contains_point(q, tol)? {
                return Ok(None);
            }
        }
    }
    // Where an open chain stops: the face across the plane face's next edge at each end, one surface for both.
    let ends = chain_ends(shape, edges, tol)?;
    let stop = match ends.as_slice() {
        [] => None,
        [a, b] => {
            let across = |v: DVec3| -> Result<Option<u32>> {
                let mut found = None;
                for (x, fs) in ef.iter().enumerate() {
                    if edges.contains(&x) || !fs.contains(&pf) || fs.len() != 2 {
                        continue;
                    }
                    let i = all[x].edge_info()?;
                    if i.start.distance(v) <= tol || i.end.distance(v) <= tol {
                        if found.is_some() {
                            return Ok(None);
                        }
                        found = fs.iter().copied().find(|&f| f != pf);
                    }
                }
                Ok(found)
            };
            let (Some(fa), Some(fb)) = (across(*a)?, across(*b)?) else { return Ok(None) };
            if fa != fb {
                return Ok(None);
            }
            let Some(region) = stop_region(&faces[fa as usize], *a, scale)? else { return Ok(None) };
            Some(region)
        }
        _ => return Ok(None),
    };
    // Each edge's wedge: ruled from a section just above the plane (its offset e (1 + h / d) in) to one just below d
    // down (its offset e h / d out), each the offset closed by one further out (out of the part). At the chain's ends
    // the offsets run on along their tangents (to the stop); between two edges the inward offsets cross (the chain
    // turns into the part), so none.
    let h = 0.05 * d;
    let reach = 4.0 * (d + e);
    // (Just out of the part along the edge (a curve tighter than a larger offset turns over); at a run-on end out
    // and away, for the stop to trim.)
    let out = 0.2 * e;
    let at_end = |p: DVec3| ends.iter().any(|q| q.distance(p) <= tol);
    let mut wedges = Vec::new();
    let mut on_chamfer = Vec::new();
    for c in &chain {
        let ci = c.edge_info()?;
        // (A whole circle's rim: a ring of a section, not this one's band.)
        if ci.closed {
            return Ok(None);
        }
        let (f0, f1) = (ci.first_param, ci.last_param);
        // Along the curve's own direction (its parameter's), whatever the edge's orientation.
        let tangent = |t: f64| -> Result<DVec3> { Ok(c.derivatives_at(t)?[1].normalize()) };
        let (s0, s1) = (c.point_at(f0)?, c.point_at(f1)?);
        let (run0, run1) = (if at_end(s0) { reach } else { 0.0 }, if at_end(s1) { reach } else { 0.0 });
        // At a run-on end the curve itself goes on first (its side face's: c2-impeller-loft's strips are pieces of the
        // spline Part 3's sides go on along), d + e of it, then its tangent.
        let along = |t: f64| -> Result<f64> { Ok((d + e) / c.derivatives_at(t)?[1].length().max(1e-300)) };
        let (g0, g1) = (if run0 > 0.0 { f0 - along(f0)? } else { f0 }, if run1 > 0.0 { f1 + along(f1)? } else { f1 });
        // In along the plane: the side of the edge the plane face is on.
        let fm = 0.5 * (f0 + f1);
        let inward = if plane_face.contains_point(c.point_at(fm)? + n.cross(tangent(fm)?).normalize() * (1e-3 * e), 1e-4 * e)? { 1.0 } else { -1.0 };
        // The edge `down` below the plane, offset `by` in along it (out where negative): circles as circles, lines as
        // lines, other curves through their offset points and tangents.
        let offset = |down: f64, by: f64| -> Result<Shape> {
            let at = |t: f64| -> Result<DVec3> { Ok(c.point_at(t)? - n * down + n.cross(tangent(t)?).normalize() * (inward * by)) };
            match ci.curve {
                crate::geom::Curve::Circle { .. } => crate::construct::make_arc_3pts(at(g0)?, at(0.5 * (g0 + g1))?, at(g1)?),
                crate::geom::Curve::Line { .. } => make_line(at(g0)?, at(g1)?),
                _ => {
                    let k = 48;
                    let ts: Vec<f64> = (0..=k).map(|j| g0 + (g1 - g0) * j as f64 / k as f64).collect();
                    let pts = ts.iter().map(|&t| at(t)).collect::<Result<Vec<_>>>()?;
                    let tans = ts.iter().map(|&t| tangent(t)).collect::<Result<Vec<_>>>()?;
                    crate::construct::make_interpolated_spline(&pts, &tans, false)
                }
            }
        };
        let (t0, t1) = (tangent(g0)?, tangent(g1)?);
        let section = |down: f64, by: f64| -> Result<Shape> {
            let (inner, outer) = (offset(down, by)?, offset(down, -out)?);
            let (ii, oi) = (inner.edge_info()?, outer.edge_info()?);
            let (p0, p1) = (inner.point_at(ii.first_param)?, inner.point_at(ii.last_param)?);
            let (q0, q1) = (outer.point_at(oi.first_param)?, outer.point_at(oi.last_param)?);
            let (m0, m1) = ((q0 - p0).normalize_or_zero(), (q1 - p1).normalize_or_zero());
            let (a0, a1, b0, b1) = (p0 - t0 * run0, p1 + t1 * run1, q0 + (m0 - t0) * run0, q1 + (m1 + t1) * run1);
            let mut edges = Vec::new();
            if run0 > 0.0 {
                edges.push(make_line(a0, p0)?);
            }
            edges.push(inner.clone());
            if run1 > 0.0 {
                edges.push(make_line(p1, a1)?);
            }
            edges.push(make_line(a1, b1)?);
            if run1 > 0.0 {
                edges.push(make_line(b1, q1)?);
            }
            edges.push(outer);
            if run0 > 0.0 {
                edges.push(make_line(q0, b0)?);
            }
            edges.push(make_line(b0, a0)?);
            make_wire(&edges)
        };
        let (top, bottom) = (section(-h, e * (1.0 + h / d))?, section(d + h, -e * h / d)?);
        let m = n.cross(tangent(fm)?).normalize() * inward;
        let (pt, pb) = (c.point_at(fm)? + n * h + m * (e * (1.0 + h / d)), c.point_at(fm)? - n * (d + h) - m * (e * h / d));
        let w = loft(&[top, bottom], &LoftOptions { solid: true, ruled: true, ..Default::default() })?.shape;
        if w.count(ShapeType::Solid) != 1 || !w.is_valid() {
            return Ok(None);
        }
        wedges.push(w);
        // (The two sections' middles pair on the ruled face: d / 2 down there.)
        let f = (0.5 * d + h) / (d + 2.0 * h);
        on_chamfer.push(pb + (pt - pb) * f);
    }
    let opts = BooleanOptions::default();
    let mut tool = boolean::boolean(BooleanOp::Fuse, &wedges[..1], &wedges[1..], &opts)?.shape;
    // Between the plane and d down only (the sections' margins keep the wedges' faces apart, they cut nothing).
    let x = n.any_orthonormal_vector();
    let slab = crate::construct::make_box(&Plane::new(plane_point - n * d - x * scale - n.cross(x) * scale, n, x), 2.0 * scale, 2.0 * scale, d)?;
    tool = boolean::boolean(BooleanOp::Common, &[tool], &[slab], &opts)?.shape;
    // At the stop: only what's on the side of that face's surface its material isn't.
    if let Some(stop) = stop {
        tool = boolean::boolean(BooleanOp::Common, &[tool], &[stop], &opts)?.shape;
    }
    if tool.count(ShapeType::Solid) == 0 {
        return Ok(None);
    }
    Ok(Some(RimChamfer { tool, on_chamfer }))
}

/// The ends of the chain `edges` (points of one edge only), none for a closed one.
fn chain_ends(shape: &Shape, edges: &[usize], tol: f64) -> Result<Vec<DVec3>> {
    let all = shape.edges();
    let mut pts: Vec<(DVec3, usize)> = Vec::new();
    for &x in edges {
        let i = all[x].edge_info()?;
        for p in [i.start, i.end] {
            match pts.iter_mut().find(|(q, _)| q.distance(p) <= tol) {
                Some(found) => found.1 += 1,
                None => pts.push((p, 1)),
            }
        }
    }
    Ok(pts.into_iter().filter(|(_, k)| *k == 1).map(|(p, _)| p).collect())
}

/// The side of face `f`'s surface its material isn't on, as a solid around `near`, `scale` across: a cylinder's inside
/// (on the face's own frame) or a plane's outer side; None for other surfaces or a cylinder with its material inside.
fn stop_region(f: &Shape, near: DVec3, scale: f64) -> Result<Option<Shape>> {
    let outward = f.normal_at_point(near)?.normalize();
    match f.face_surface()? {
        Surface::Cylinder { origin, axis, x_dir, radius } => {
            let a = axis.normalize();
            let off = |p: DVec3| {
                let w = p - origin;
                (w - a * w.dot(a)).length() - radius
            };
            // (The outward normal toward the axis: the material outside, the void in.)
            if off(near + outward * (1e-3 * radius)) >= off(near) {
                return Ok(None);
            }
            let way = if (near - origin).dot(a) >= 0.0 { a } else { -a };
            Ok(Some(make_cylinder(&Plane::new(origin, way, x_dir), radius, (near - origin).dot(way) + scale, std::f64::consts::TAU)?))
        }
        Surface::Plane(p) => {
            let x = p.x_dir.normalize();
            let y = outward.cross(x);
            let base = near - x * scale - y * scale;
            Ok(Some(crate::construct::make_box(&Plane::new(base, outward, x), 2.0 * scale, 2.0 * scale, scale)?))
        }
        _ => Ok(None),
    }
}
