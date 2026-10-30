//! `@opEdgeChange` (@internal, std geomOperations.fs): move sheet edges
//! within their faces. std sheet metal pulls a hem's edge back by the hem's
//! extent with it (Hem alignment OUTER, live dom/fs-smprobe3-7.txt). An
//! option's `offset` moves its edge's line in the face plane, outwards
//! when positive; the neighbouring sides keep their lines, so their ends
//! follow. Only the faces with a moved edge change: a wall meeting the
//! moved corner keeps its outline (live qa5 Hem on G34: the front flange
//! still runs to x = 60, 9378.1835 mm³). Faces keep their names (and their
//! edges, named from them, theirs).
//! `transformList` and `replaceFace` options are not supported yet.

use super::query::evaluate_refs;
use super::{context_arg, convert::*, run_op};
use crate::regen::eval::{FResult, FeatureError, err};
use glam::DVec3;
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::Value;
use noshape_kernel as k;
use noshape_kernel::Shape;
use std::collections::BTreeMap;

pub const NATIVES: &[(&str, NativeFn)] = &[("opEdgeChange", op_edge_change)];

const TOL: f64 = 1e-7;

fn op_edge_change(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let def = def_arg(i, &a, 2)?;
    let mut options = Vec::new();
    for o in def.get("edgeChangeOptions").as_array().map(|a| a.iter().cloned().collect::<Vec<_>>()).unwrap_or_default() {
        if !matches!(o.get("transformList"), Value::Undefined) || !matches!(o.get("replaceFace"), Value::Undefined) {
            return Err(i.error("opEdgeChange: only offset edge changes are supported"));
        }
        let edges = evaluate_refs(i, &cx, o.get("edge"))?;
        options.push((edges, length_of(o.get("offset")).unwrap_or(0.0)));
    }
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        let mut by_body: BTreeMap<usize, BTreeMap<usize, f64>> = BTreeMap::new();
        for (edges, offset) in &options {
            if super::trace_ops() {
                eprintln!("[fs op] edgeChange {:?} -> {:?} offset {offset}", edges, ev.resolve_edges(edges));
            }
            for (b, e) in ev.resolve_edges(edges) {
                by_body.entry(b).or_default().insert(e, *offset);
            }
        }
        for (b, offsets) in by_body {
            let body = ev.ctx.bodies[b].clone();
            let (shape, order, edge_from) = change_edges(&body.shape, &offsets)?;
            let names = order.iter().map(|j| body.faces[*j].clone()).collect();
            ev.replace_body(b, shape, names);
            keep_edge_names(ev, b, &body.edges, &edge_from);
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

/// The changed body's edges keep the names of the edges they were, moved or
/// not (std tracks a moved edge by name: a partial flange's piece of an edge
/// that sheds two step edges shifted the boundary edges' ordinals); new
/// edges are named apart from them.
fn keep_edge_names(ev: &mut crate::regen::eval::EvalCtx, b: usize, old: &[crate::naming::Name], edge_from: &[Option<usize>]) {
    use crate::reference::TopoName;
    let nb = ev.ctx.bodies[b].clone();
    if nb.edges.len() != edge_from.len() {
        return;
    }
    let mut used = std::collections::HashSet::new();
    let mut edges = nb.edges.clone();
    let mut fresh = 0u16;
    for (i, from) in edge_from.iter().enumerate() {
        match from.and_then(|j| old.get(j)) {
            Some(n) if used.insert(n.clone()) => edges[i] = n.clone(),
            _ => {
                if let TopoName::Boundary { face, .. } = &*edges[i] {
                    edges[i] = std::sync::Arc::new(TopoName::Boundary { face: face.clone(), ordinal: u16::MAX - fresh });
                    fresh += 1;
                }
            }
        }
    }
    let names = crate::naming::ShapeNames { faces: nb.faces.clone(), edges, vertices: nb.vertices.clone() };
    ev.ctx.bodies[b] = std::sync::Arc::new(crate::regen::context::Body::named(nb.id.clone(), nb.name.clone(), nb.kind, nb.shape.clone(), names, nb.color, ev.feature));
}

fn fe(e: impl ToString) -> FeatureError {
    FeatureError::new(e.to_string())
}

/// A planar face's oriented normal.
fn normal(f: &Shape) -> FResult<DVec3> {
    let [u0, u1, v0, v1] = f.uv_bounds().map_err(fe)?;
    Ok(f.normal_at_uv((u0 + u1) * 0.5, (v0 + v1) * 0.5).map_err(fe)?.normalize())
}

/// A face loop's corners, side offsets and side edges (see [`face_loop`]).
type FaceLoop = (Vec<DVec3>, Vec<Option<f64>>, Vec<Option<usize>>);

/// A face's outer loop: corners, the offset of each side (`pts[k]` to
/// `pts[k + 1]`) if it moves, and the index of each side's edge in `all`.
fn face_loop(f: &Shape, moved: &[(Shape, f64)], all: &[Shape]) -> FResult<FaceLoop> {
    let wires = f.wires();
    if wires.len() != 1 {
        return err("Edges of faces with holes cannot be changed yet");
    }
    let mut segs = Vec::new();
    for e in wires[0].edges() {
        let info = e.edge_info().map_err(fe)?;
        if !matches!(info.curve, k::Curve::Line { .. }) {
            return err("Only straight edges can be changed yet");
        }
        segs.push((info.start, info.end, moved.iter().find(|(m, _)| m.is_same(&e)).map(|x| x.1), all.iter().position(|x| x.is_same(&e))));
    }
    let (first, mut at, offset, from) = segs.swap_remove(0);
    let (mut pts, mut sides, mut froms) = (vec![first], vec![offset], vec![from]);
    while !segs.is_empty() {
        let Some(k) = segs.iter().position(|(a, b, _, _)| a.distance(at) < TOL || b.distance(at) < TOL) else { return err("A face boundary is not closed") };
        let (a, b, offset, from) = segs.swap_remove(k);
        pts.push(at);
        sides.push(offset);
        froms.push(from);
        at = if a.distance(at) < TOL { b } else { a };
    }
    Ok((pts, sides, froms))
}

/// The sheet with the edges moved, for each of its faces the index of the
/// face it was, and for each of its edges the edge it was (on that edge's
/// line, moved with it).
fn change_edges(shape: &Shape, offsets: &BTreeMap<usize, f64>) -> FResult<(Shape, Vec<usize>, Vec<Option<usize>>)> {
    let edges = shape.edges();
    let moved: Vec<(Shape, f64)> = offsets.iter().filter_map(|(e, d)| edges.get(*e).map(|s| (s.clone(), *d))).collect();
    let faces = shape.faces();
    let mut loops = Vec::new();
    for f in &faces {
        if f.face_plane().map_err(fe)?.is_none() {
            return err("Only edges of planar faces can be changed yet");
        }
        let (pts, sides, froms) = face_loop(f, &moved, &edges)?;
        loops.push((normal(f)?, pts, sides, froms));
    }
    // Each old edge's segment where it goes (moved by its offset).
    let mut segments: Vec<(usize, DVec3, DVec3)> = Vec::new();
    // New corners where a moved side meets its neighbours, per face; a side
    // moved off a collinear neighbour (a partial flange's piece of an edge,
    // live request 1599 (a)) steps across to it: two corners.
    let mut corners: Vec<Vec<(DVec3, Vec<DVec3>)>> = Vec::new();
    for (n, pts, sides, froms) in &loops {
        let mut own = Vec::new();
        let m = pts.len();
        let area: DVec3 = (0..m).map(|k| pts[k].cross(pts[(k + 1) % m])).sum();
        let ccw = area.dot(*n) > 0.0;
        let line = |k: usize| {
            let (p, q) = (pts[k], pts[(k + 1) % m]);
            let d = (q - p).normalize();
            let out = if ccw { d.cross(*n) } else { n.cross(d) };
            (p + out * sides[k].unwrap_or(0.0), d)
        };
        for k in 0..m {
            if let Some(j) = froms[k] {
                let (p, d) = line(k);
                segments.push((j, p, p + d * pts[k].distance(pts[(k + 1) % m])));
            }
        }
        for k in 0..m {
            let prev = (k + m - 1) % m;
            if sides[prev].is_none() && sides[k].is_none() {
                continue;
            }
            let ((p0, d0), (p1, d1)) = (line(prev), line(k));
            let c = d0.cross(d1);
            if c.length_squared() < 1e-20 {
                if (sides[prev].unwrap_or(0.0) - sides[k].unwrap_or(0.0)).abs() > TOL {
                    let at = |q: DVec3, d: DVec3| q + d * (pts[k] - q).dot(d);
                    own.push((pts[k], vec![at(p0, d0), at(p1, d1)]));
                }
                continue;
            }
            let t = (p1 - p0).cross(d1).dot(c) / c.length_squared();
            own.push((pts[k], vec![p0 + d0 * t]));
        }
        corners.push(own);
    }
    let mut built = Vec::new();
    for (((n, pts, _, _), f), corners) in loops.iter().zip(&faces).zip(&corners) {
        let new: Vec<DVec3> = pts.iter().flat_map(|p| corners.iter().find(|(o, _)| o.distance(*p) < TOL).map_or(vec![*p], |x| x.1.clone())).collect();
        if new.len() == pts.len() && new.iter().zip(pts).all(|(a, b)| a.distance(*b) < TOL) {
            built.push(f.clone());
            continue;
        }
        let wire = k::make_polyline(&new, true).map_err(fe)?;
        let mut face = k::make_face_from_wire(&wire, true).map_err(fe)?;
        if normal(&face)?.dot(*n) < 0.0 {
            face = face.reversed().map_err(fe)?;
        }
        built.push(face);
    }
    // A new edge is the old edge it lies along (the one whose segment holds
    // its middle, else the nearest of those on its line).
    let edge_from = |result: &Shape| -> Vec<Option<usize>> {
        result
            .edges()
            .iter()
            .map(|e| {
                let (a, b) = e.endpoints().ok()?;
                let on_line = |p: DVec3, q: DVec3, x: DVec3| ((x - p).cross((q - p).normalize())).length() < 1e-6;
                let mid = (a + b) * 0.5;
                let along = |p: DVec3, q: DVec3| {
                    let t = (mid - p).dot(q - p) / (q - p).length_squared().max(1e-300);
                    (t.clamp(0.0, 1.0) - t).abs()
                };
                segments.iter().filter(|(_, p, q)| on_line(*p, *q, a) && on_line(*p, *q, b)).min_by(|x, y| along(x.1, x.2).total_cmp(&along(y.1, y.2))).map(|x| x.0)
            })
            .collect()
    };
    if built.len() == 1 {
        let face = built.remove(0);
        let from = edge_from(&face);
        return Ok((face, vec![0], from));
    }
    let sewn = k::sew(&built, TOL, false).map_err(fe)?.shape;
    // Sewing may reorder the faces: match them by their boxes.
    let key = |f: &Shape| f.bounding_box(false).map(|b| (b.center(), b.diagonal())).ok();
    let keys: Vec<_> = built.iter().map(key).collect();
    let order = sewn
        .faces()
        .iter()
        .map(|f| {
            let kf = key(f)?;
            keys.iter().position(|k| k.is_some_and(|k| k.0.distance(kf.0) < 1e-9 && (k.1 - kf.1).abs() < 1e-9))
        })
        .collect::<Option<Vec<usize>>>()
        .ok_or_else(|| fe("The changed sheet's faces could not be matched"))?;
    let from = edge_from(&sewn);
    Ok((sewn, order, from))
}
