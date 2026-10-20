//! `@evOffsetDetection` (std evaluate.fs, used by sheetMetalRecognize):
//! groups of faces that are offsets of each other through the material.
//!
//! Live (dom/fs-offset-detect.txt): one thickness per call, the smallest
//! face-pair offset of the first body; every body then contributes the
//! pairs at that offset, connected pairs forming one group (an L bracket's
//! two legs and bend are one group of three pairs). A 1 mm and a 3 mm plate
//! give only the 1 mm plate; plate + tube + bracket + plates + wedge give
//! the 2 mm plate and tube. `side0[i]` / `side1[i]` are the two faces of
//! pair `i`; `offsetLow` / `offsetHigh` the smallest and largest offset.

use super::query::{Ent, QueryCx, with_ents};
use super::{context_arg, convert::*};
use crate::reference::Reference;
use glam::DVec3;
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::Value;
use noshape_kernel as k;
use noshape_kernel::Shape;

pub const NATIVES: &[(&str, NativeFn)] = &[("evOffsetDetection", ev_offset_detection)];

/// Default offset tolerance (meters).
const TOL: f64 = 1e-7;

/// Points inside a face with their outward normals.
fn samples(f: &Shape) -> Vec<(DVec3, DVec3)> {
    let Ok([u0, u1, v0, v1]) = f.uv_bounds() else { return Vec::new() };
    let mut out = Vec::new();
    for (a, b) in [(0.5, 0.5), (0.25, 0.25), (0.75, 0.75), (0.25, 0.75), (0.75, 0.25), (0.5, 0.2), (0.2, 0.5)] {
        let (u, v) = (u0 + (u1 - u0) * a, v0 + (v1 - v0) * b);
        let (Ok(p), Ok(n)) = (f.point_at_uv(u, v), f.normal_at_uv(u, v)) else { continue };
        // Trimmed faces: keep points inside the face.
        if f.project_point(p).is_ok_and(|pr| pr.point.distance(p) < 1e-9) {
            out.push((p, n.normalize_or_zero()));
        }
        if out.len() >= 3 {
            break;
        }
    }
    out
}

/// Surfaces that can be offsets of each other (analytic types must match).
fn compatible(a: &Option<k::Surface>, b: &Option<k::Surface>) -> bool {
    use k::Surface as S;
    let analytic = |s: &S| matches!(s, S::Plane(_) | S::Cylinder { .. } | S::Sphere { .. } | S::Cone { .. } | S::Torus { .. });
    match (a, b) {
        (Some(S::Plane(p)), Some(S::Plane(q))) => p.unit_normal().cross(q.unit_normal()).length() < 1e-9,
        (Some(S::Cylinder { origin: o1, axis: a1, .. }), Some(S::Cylinder { origin: o2, axis: a2, .. })) => a1.normalize().cross(a2.normalize()).length() < 1e-9 && (*o2 - *o1).cross(a1.normalize()).length() < 1e-9,
        (Some(S::Sphere { center: c1, .. }), Some(S::Sphere { center: c2, .. })) => c1.distance(*c2) < 1e-9,
        (Some(x), Some(y)) if analytic(x) || analytic(y) => std::mem::discriminant(x) == std::mem::discriminant(y),
        _ => true,
    }
}

/// The constant offset from face `a` to face `b` through the material, if `b`
/// is an offset of `a`.
fn offset_between(sa: &[(DVec3, DVec3)], b: &Shape, tol: f64) -> Option<f64> {
    let mut d0: Option<f64> = None;
    for (p, n) in sa {
        let v = k::make_vertex(*p).ok()?;
        let r = k::distance(&v, b).ok()?;
        let d = r.distance;
        if d < tol {
            return None;
        }
        // The partner point lies straight behind `p` and faces back.
        if r.point_b.distance(*p - *n * d) > tol.max(d * 1e-7) {
            return None;
        }
        let nb = b.normal_at_point(r.point_b).ok()?;
        if nb.dot(*n) > -1.0 + 1e-6 {
            return None;
        }
        match d0 {
            Some(x) if (x - d).abs() > tol => return None,
            Some(_) => {}
            None => d0 = Some(d),
        }
    }
    d0
}

/// An offset face pair `(a, b, offset)` (face indices of one body).
type Pair = (usize, usize, f64);

/// Offset face pairs `(a, b, offset)` of one body, `a` before `b` in face order.
fn body_pairs(q: &QueryCx, b: usize, tol: f64) -> Vec<Pair> {
    let faces: Vec<Shape> = q.st.ctx.bodies[b].shape.faces();
    let surfaces: Vec<Option<k::Surface>> = faces.iter().map(|f| f.face_surface().ok()).collect();
    let samp: Vec<Vec<(DVec3, DVec3)>> = faces.iter().map(samples).collect();
    let mut out = Vec::new();
    for i in 0..faces.len() {
        for j in i + 1..faces.len() {
            if samp[i].is_empty() || samp[j].is_empty() || !compatible(&surfaces[i], &surfaces[j]) {
                continue;
            }
            // Both ways: the smaller face may not see all of the larger.
            let d = offset_between(&samp[i], &faces[j], tol).or_else(|| offset_between(&samp[j], &faces[i], tol));
            if let Some(d) = d {
                out.push((i, j, d));
            }
        }
    }
    out
}

/// Connected groups of pairs (pairs touch when their faces share an edge),
/// oriented so `side0` faces are adjacent to `side0` faces.
fn groups(q: &QueryCx, b: usize, pairs: &[Pair]) -> Vec<Vec<Pair>> {
    let edge_faces = q.st.ctx.bodies[b].shape.edge_faces().unwrap_or_default();
    let adjacent = |x: usize, y: usize| edge_faces.iter().any(|fs| fs.contains(&(x as u32)) && fs.contains(&(y as u32)));
    let mut used = vec![false; pairs.len()];
    let mut out = Vec::new();
    for seed in 0..pairs.len() {
        if used[seed] {
            continue;
        }
        used[seed] = true;
        let mut group = vec![pairs[seed]];
        let mut n = 0;
        while n < group.len() {
            let (s0, s1, _) = group[n];
            for (k, p) in pairs.iter().enumerate() {
                if used[k] {
                    continue;
                }
                let (a, c, d) = *p;
                let oriented = if adjacent(s0, a) || adjacent(s1, c) {
                    Some((a, c, d))
                } else if adjacent(s0, c) || adjacent(s1, a) {
                    Some((c, a, d))
                } else {
                    None
                };
                if let Some(o) = oriented {
                    used[k] = true;
                    group.push(o);
                }
            }
            n += 1;
        }
        out.push(group);
    }
    out
}

fn ev_offset_detection(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let tol = length_of(def.get("offsetTolerance")).unwrap_or(TOL).max(TOL);
    let found: Vec<(Vec<Reference>, Vec<Reference>, f64, f64)> = with_ents(i, &cx, def.get("bodies"), |_, q, ents| {
        let bodies: Vec<usize> = ents.iter().filter_map(|e| if let Ent::Body(b) = e { Some(*b) } else { None }).filter(|b| !q.st.ctx.bodies[*b].shape.solids().is_empty()).collect();
        let per_body: Vec<(usize, Vec<Pair>)> = bodies.iter().map(|b| (*b, body_pairs(q, *b, TOL))).collect();
        // The thickness: the first body's smallest offset.
        let Some(thickness) = per_body.iter().find(|(_, p)| !p.is_empty()).map(|(_, p)| p.iter().map(|x| x.2).fold(f64::INFINITY, f64::min)) else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        for (b, pairs) in &per_body {
            let at: Vec<Pair> = pairs.iter().copied().filter(|p| (p.2 - thickness).abs() <= tol).collect();
            for g in groups(q, *b, &at) {
                let face = |f: usize| q.to_ref(&Ent::Face(*b, f));
                let low = g.iter().map(|p| p.2).fold(f64::INFINITY, f64::min);
                let high = g.iter().map(|p| p.2).fold(f64::NEG_INFINITY, f64::max);
                out.push((g.iter().map(|p| face(p.0)).collect(), g.iter().map(|p| face(p.1)).collect(), low, high));
            }
        }
        Ok(out)
    })?;
    let mut st = cx.state.lock();
    let ids = |st: &mut super::CtxState, refs: &[Reference]| Value::array(refs.iter().map(|r| Value::str(st.transient(r))).collect());
    let groups = found
        .iter()
        .map(|(s0, s1, low, high)| {
            let side0 = ids(&mut st, s0);
            let side1 = ids(&mut st, s1);
            Value::map_from([("side0", side0), ("side1", side1), ("offsetLow", Value::num(*low)), ("offsetHigh", Value::num(*high))])
        })
        .collect();
    Ok(Value::array(groups))
}
