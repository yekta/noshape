//! Rounds of a wall that thins to nothing: a convex and a concave picked edge of one planar wall leave a vertex the same
//! way, the wall a sliver between them (asm-scissor-lift 9cbb0e's Fillet 18: the wall between a plate's top and a round
//! tangent to it). Live runs the concave rounds on to the tip; where the wall is lower than the two balls the convex
//! round's ball rolls off the wall onto the concave round. Built in stages: the concave rounds on the wall raised past
//! the plate's top, the top cut back, then the rest.

use super::detail::blend_role;
use crate::naming::Name;
use crate::reference::{Role, TopoName};
use crate::regen::eval::{names_for, EvalCtx, FResult, NamedShape};
use glam::DVec3;
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;
use std::sync::Arc;

struct Cusp {
    /// The tip, the convex edge's direction away from it, the plate's and the wall's outward normals.
    v: DVec3,
    t: DVec3,
    np: DVec3,
    nw: DVec3,
    wall: usize,
    /// The box's length along the convex edge.
    len: f64,
}

fn mid(e: &Shape) -> Option<DVec3> {
    let i = e.edge_info().ok()?;
    e.point_at(0.5 * (i.first_param + i.last_param)).ok()
}

/// The edge's direction away from its end at `v` (None where it doesn't end there).
fn away(e: &Shape, v: DVec3, tol: f64) -> Option<DVec3> {
    let i = e.edge_info().ok()?;
    [i.first_param, i.last_param].into_iter().find_map(|t| {
        let p = e.point_at(t).ok()?;
        if p.distance(v) > tol {
            return None;
        }
        let d = e.derivatives_at(t).ok()?[1].normalize_or_zero();
        Some(if t == i.first_param { d } else { -d })
    })
}

/// Convex (Some(true)), concave (Some(false)) or smooth (None): a point off the edge along the difference of its faces'
/// normals is outside a convex edge's material, inside a concave one's.
fn convexity(shape: &Shape, faces: &[Shape], fs: &[u32], e: &Shape, eps: f64) -> Option<bool> {
    let [a, b] = fs else { return None };
    let p = mid(e)?;
    let (na, nb) = (faces[*a as usize].normal_at_point(p).ok()?, faces[*b as usize].normal_at_point(p).ok()?);
    if na.dot(nb) > 1.0 - 1e-9 {
        return None;
    }
    match shape.classify_point(p + (na - nb).normalize() * eps, eps * 1e-3).ok()? {
        k::PointClass::Outside => Some(true),
        k::PointClass::Inside => Some(false),
        _ => None,
    }
}

fn plane_normal(face: &Shape, at: DVec3) -> Option<DVec3> {
    face.face_plane().ok()??;
    face.normal_at_point(at).ok()
}

fn cusps(shape: &Shape, edges: &[usize], r: f64, eps: f64, tol: f64) -> Vec<Cusp> {
    let (all, faces) = (shape.edges(), shape.faces());
    let Ok(ef) = shape.edge_faces() else { return Vec::new() };
    let kind: Vec<Option<bool>> = edges.iter().map(|&e| convexity(shape, &faces, &ef[e], &all[e], eps)).collect();
    let mut out: Vec<Cusp> = Vec::new();
    for (i, &a) in edges.iter().enumerate() {
        for (j, &b) in edges.iter().enumerate() {
            if kind[i] != Some(true) || kind[j] != Some(false) {
                continue;
            }
            let Some(&wall) = ef[a].iter().find(|f| ef[b].contains(f)) else { continue };
            let Some(&plate) = ef[a].iter().find(|&&f| f != wall) else { continue };
            let Ok((s, e)) = all[a].endpoints() else { continue };
            for v in [s, e] {
                let (Some(t), Some(u)) = (away(&all[a], v, tol), away(&all[b], v, tol)) else { continue };
                let (Some(np), Some(nw)) = (plane_normal(&faces[plate as usize], v), plane_normal(&faces[wall as usize], v)) else { continue };
                if t.dot(u) < 1.0 - 1e-6 || out.iter().any(|c| c.v.distance(v) < tol) {
                    continue;
                }
                let len = all[a].length().unwrap_or(0.0).min(15.0 * r);
                out.push(Cusp { v, t, np, nw, wall: wall as usize, len });
            }
        }
    }
    out
}

/// A box from `o`: `len` along the cusp's edge, `wid` along `across`, `depth` along the plate's outward normal.
fn slab(c: &Cusp, o: DVec3, across: DVec3, wid: f64, depth: f64) -> k::Result<Shape> {
    let frame = k::Plane { origin: o, x_dir: c.t, normal: c.np };
    let o = if c.np.cross(c.t).dot(across) > 0.0 { o } else { o + across * wid };
    k::make_box(&k::Plane { origin: o, ..frame }, c.len, wid, depth)
}

/// The image in `shape` of each of `of` (same mid-point, or a piece of a line on it).
fn images(shape: &Shape, of: &[Shape], tol: f64) -> Vec<usize> {
    let all = shape.edges();
    let on = |o: &Shape, e: &Shape| -> bool {
        let Some(p) = mid(e) else { return false };
        if mid(o).is_some_and(|q| q.distance(p) < tol) {
            return true;
        }
        let (Ok(k::Curve::Line { .. }), Ok((a, b)), Ok((c, d))) = (o.edge_curve(), o.endpoints(), e.endpoints()) else { return false };
        let dist = |q: DVec3| {
            let ab = b - a;
            let s = ((q - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            (a + ab * s).distance(q)
        };
        dist(c) < tol && dist(d) < tol && dist(p) < tol
    };
    (0..all.len()).filter(|&i| of.iter().any(|o| on(o, &all[i]))).collect()
}

fn unified(ev: &EvalCtx, shape: Shape, names: Vec<Name>) -> FResult<(Shape, Vec<Name>)> {
    let u = k::unify_same_domain(&shape)?;
    let names = ev.name_result(&[&names_for(&shape, names)], &u.history, &u.shape, &BTreeMap::new(), None);
    Ok((u.shape, names))
}

/// [`cusp`]'s round of `edges` at radius `r`: whether it applies and was built.
pub fn cusp_wall_round(ev: &mut EvalCtx, bi: usize, edges: &[usize], r: f64) -> FResult<bool> {
    let body = ev.ctx.bodies[bi].clone();
    let shape = &body.shape;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let (tol, eps) = (scale * 1e-7, r * 1e-3);
    // Only where the plate's top cut back over the wall and round is air (the wall's other end at a round rising off the
    // plate isn't a cusp: the cut would take that round).
    let cut_box = |c: &Cusp| slab(c, c.v + c.nw * 1.25 * r, -c.nw, 3.25 * r, 1.25 * r);
    let empty = |c: &Cusp| {
        cut_box(c).and_then(|b| k::boolean(k::BooleanOp::Common, std::slice::from_ref(shape), &[b], &k::BooleanOptions::default())).is_ok_and(|x| x.shape.volume().is_ok_and(|v| v < 1e-3 * r.powi(3)))
    };
    let cs: Vec<Cusp> = cusps(shape, edges, r, eps, tol).into_iter().filter(empty).collect();
    if cs.is_empty() {
        return Ok(false);
    }
    let debug = std::env::var_os("NOSHAPE_BLEND_DEBUG").is_some();
    let (all, faces, ef) = (shape.edges(), shape.faces(), shape.edge_faces()?);
    let walls: Vec<usize> = cs.iter().map(|c| c.wall).collect();
    let (first, rest): (Vec<usize>, Vec<usize>) = edges.iter().partition(|&&e| ef[e].iter().any(|f| walls.contains(&(*f as usize))) && convexity(shape, &faces, &ef[e], &all[e], eps) == Some(false));
    let names = body.shape_names();
    let edge_name = |e: usize| names.edges.get(e).cloned();
    let Some(made) = edges.first().and_then(|&e| edge_name(e)) else { return Ok(false) };
    let tool_name = Arc::new(TopoName::face(ev.feature, Role::Generated(made.clone())));
    let tool_names = |s: &Shape| vec![tool_name.clone(); s.count(ShapeType::Face)];
    // 1: the wall raised past the plate's top, the concave rounds on it.
    let raise: Vec<Shape> = cs.iter().map(|c| slab(c, c.v, -c.nw, 2.0 * r, 1.25 * r)).collect::<k::Result<_>>()?;
    let v0 = shape.volume()?;
    let boxes: f64 = raise.iter().map(|b| b.volume().unwrap_or(0.0)).sum();
    let tools: Vec<NamedShape> = raise.iter().map(|b| NamedShape::new(b.clone(), tool_names(b))).collect();
    let (s1, n1) = super::sheetmetal_util::bool_named(ev, k::BooleanOp::Fuse, &NamedShape { shape: shape.clone(), names: names.clone() }, &tools)?;
    let (s1, n1) = unified(ev, s1, n1)?;
    if (s1.volume()? - v0 - boxes).abs() > 1e-6 * v0 {
        if debug {
            eprintln!("cusp round: the raised wall's box isn't in the air");
        }
        return Ok(false);
    }
    let firsts: Vec<Shape> = first.iter().map(|&e| all[e].clone()).collect();
    let on1: Vec<Shape> = images(&s1, &firsts, tol).into_iter().map(|i| s1.edges()[i].clone()).collect();
    let f1 = match k::fillet_checked(&s1, &on1, &[r]) {
        Ok(f) => f,
        Err(e) => {
            if debug {
                eprintln!("cusp round: concave rounds on the raised wall: {e}");
            }
            return Ok(false);
        }
    };
    let n1 = ev.name_result(&[&names_for(&s1, n1)], &f1.history, &f1.shape, &BTreeMap::new(), Some(&blend_role));
    // 2: the plate's top cut back over the wall and the concave round's width.
    let cut: Vec<Shape> = cs.iter().map(cut_box).collect::<k::Result<_>>()?;
    let tools: Vec<NamedShape> = cut.iter().map(|b| NamedShape::new(b.clone(), tool_names(b))).collect();
    let (s2, n2) = super::sheetmetal_util::bool_named(ev, k::BooleanOp::Cut, &NamedShape::new(f1.shape.clone(), n1), &tools)?;
    let (s2, n2) = unified(ev, s2, n2)?;
    if debug {
        eprintln!("cusp round: {} cusps, {} concave rounds first: {} faces, valid {}", cs.len(), on1.len(), s2.count(ShapeType::Face), s2.is_valid());
    }
    // 3: the rest, and the plate's creases with the concave rounds where the wall was lower than the ball.
    let rests: Vec<Shape> = rest.iter().map(|&e| all[e].clone()).collect();
    let mut on3 = images(&s2, &rests, tol);
    let (all2, faces2, ef2) = (s2.edges(), s2.faces(), s2.edge_faces()?);
    for (i, e) in all2.iter().enumerate() {
        let Some(p) = mid(e) else { continue };
        let crease = cs.iter().any(|c| {
            p.distance(c.v) < c.len
                && ef2[i].iter().any(|&f| plane_normal(&faces2[f as usize], p).is_some_and(|n| n.dot(c.np) > 1.0 - 1e-9 && (p - c.v).dot(c.np).abs() < tol * 10.0))
                && ef2[i].iter().any(|&f| faces2[f as usize].face_plane().ok().flatten().is_none())
        });
        if crease && !on3.contains(&i) && convexity(&s2, &faces2, &ef2[i], e, eps) == Some(true) {
            on3.push(i);
        }
    }
    on3.sort_unstable();
    let shapes3: Vec<Shape> = on3.iter().map(|&i| all2[i].clone()).collect();
    match k::fillet_checked(&s2, &shapes3, &[r]) {
        Ok(f3) => {
            let n3 = ev.name_result(&[&names_for(&s2, n2)], &f3.history, &f3.shape, &BTreeMap::new(), Some(&blend_role));
            ev.replace_body(bi, f3.shape, n3);
            return Ok(true);
        }
        Err(e) if debug => eprintln!("cusp round: the rest by OCCT: {e}"),
        Err(_) => {}
    }
    ev.replace_body(bi, s2, n2);
    let balls = super::blend_section::rolling_ball_blend(ev, bi, &on3, r, None).or_else(|_| super::blend_section::with_concave_balls(|| super::blend_section::rolling_ball_blend(ev, bi, &on3, r, None)));
    if let Err(e) = balls {
        if debug {
            eprintln!("cusp round: the rest by rolling balls: {}", e.message);
        }
        ev.ctx.bodies[bi] = body;
        return Ok(false);
    }
    Ok(true)
}
