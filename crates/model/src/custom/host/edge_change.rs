//! `@opEdgeChange` (@internal, std geomOperations.fs): move sheet edges
//! within their faces. std sheet metal pulls a hem's edge back by the hem's
//! extent with it (Hem alignment OUTER, live dom/fs-smprobe3-7.txt). An
//! option's `offset` moves its edge's line in the face plane, outwards
//! when positive; the neighbouring sides keep their lines, so their ends
//! follow, in every face of the sheet sharing a moved corner. Faces keep
//! their names (and their edges, named from them, theirs).
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
            for (b, e) in ev.resolve_edges(edges) {
                by_body.entry(b).or_default().insert(e, *offset);
            }
        }
        for (b, offsets) in by_body {
            let body = ev.ctx.bodies[b].clone();
            let (shape, order) = change_edges(&body.shape, &offsets)?;
            let names = order.iter().map(|j| body.faces[*j].clone()).collect();
            ev.replace_body(b, shape, names);
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

fn fe(e: impl ToString) -> FeatureError {
    FeatureError::new(e.to_string())
}

/// A planar face's oriented normal.
fn normal(f: &Shape) -> FResult<DVec3> {
    let [u0, u1, v0, v1] = f.uv_bounds().map_err(fe)?;
    Ok(f.normal_at_uv((u0 + u1) * 0.5, (v0 + v1) * 0.5).map_err(fe)?.normalize())
}

/// A face's outer loop: corners, and the offset of each side (`pts[k]` to
/// `pts[k + 1]`) if it moves.
fn face_loop(f: &Shape, moved: &[(Shape, f64)]) -> FResult<(Vec<DVec3>, Vec<Option<f64>>)> {
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
        segs.push((info.start, info.end, moved.iter().find(|(m, _)| m.is_same(&e)).map(|x| x.1)));
    }
    let (first, mut at, offset) = segs.swap_remove(0);
    let (mut pts, mut sides) = (vec![first], vec![offset]);
    while !segs.is_empty() {
        let Some(k) = segs.iter().position(|(a, b, _)| a.distance(at) < TOL || b.distance(at) < TOL) else { return err("A face boundary is not closed") };
        let (a, b, offset) = segs.swap_remove(k);
        pts.push(at);
        sides.push(offset);
        at = if a.distance(at) < TOL { b } else { a };
    }
    Ok((pts, sides))
}

/// The sheet with the edges moved, and for each of its faces the index of
/// the face it was.
fn change_edges(shape: &Shape, offsets: &BTreeMap<usize, f64>) -> FResult<(Shape, Vec<usize>)> {
    let edges = shape.edges();
    let moved: Vec<(Shape, f64)> = offsets.iter().filter_map(|(e, d)| edges.get(*e).map(|s| (s.clone(), *d))).collect();
    let faces = shape.faces();
    let mut loops = Vec::new();
    for f in &faces {
        if f.face_plane().map_err(fe)?.is_none() {
            return err("Only edges of planar faces can be changed yet");
        }
        let (pts, sides) = face_loop(f, &moved)?;
        loops.push((normal(f)?, pts, sides));
    }
    // New corners where a moved side meets its neighbours.
    let mut corners: Vec<(DVec3, DVec3)> = Vec::new();
    for (n, pts, sides) in &loops {
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
            let prev = (k + m - 1) % m;
            if sides[prev].is_none() && sides[k].is_none() {
                continue;
            }
            let ((p0, d0), (p1, d1)) = (line(prev), line(k));
            let c = d0.cross(d1);
            if c.length_squared() < 1e-20 {
                continue;
            }
            let t = (p1 - p0).cross(d1).dot(c) / c.length_squared();
            corners.push((pts[k], p0 + d0 * t));
        }
    }
    let mut built = Vec::new();
    for ((n, pts, _), f) in loops.iter().zip(&faces) {
        let new: Vec<DVec3> = pts.iter().map(|p| corners.iter().find(|(o, _)| o.distance(*p) < TOL).map_or(*p, |x| x.1)).collect();
        if new.iter().zip(pts).all(|(a, b)| a.distance(*b) < TOL) {
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
    if built.len() == 1 {
        return Ok((built.remove(0), vec![0]));
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
    Ok((sewn, order))
}
