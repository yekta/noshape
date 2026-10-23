//! Std sheet metal loft's natives (sheetMetalLoft.fs): `@evTessellatedLoftMatches`
//! pairs points of the two profiles (each profile faceted by the chordal
//! tolerance, corners matched by the best rotation / direction), and
//! `@opTessellatedLoft` makes the faceted sheet between the matched points.
//! The faceting is the native sheet metal loft's (sheetmetal_loft.rs).

use super::query::{Ent, QueryCx, evaluate_refs, query_of_refs};
use super::{CtxState, FsContext, context_arg, convert::*, run_op};
use crate::display::BodyKind;
use crate::naming::Name;
use crate::reference::{Reference, Role, TopoName};
use crate::regen::eval::{FeatureError, NamedShape, err};
use crate::regen::features::bshared::order_chain;
use glam::DVec3;
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::{Map, Value};
use noshape_kernel as k;
use noshape_kernel::Shape;
use std::sync::Arc;

pub const NATIVES: &[(&str, NativeFn)] = &[("evTessellatedLoftMatches", ev_tessellated_loft_matches), ("opTessellatedLoft", op_tessellated_loft)];

/// A point of a faceted profile: on `edge` at normalized `t` (0..1).
#[derive(Clone)]
struct ProfilePoint {
    edge: Reference,
    t: f64,
    pos: DVec3,
}

/// A profile's edges in chain order, faceted by `chordal` (lines: their ends;
/// arcs: by the chord angle; other curves: by length), corners on straight
/// runs dropped. Returns the points and whether the profile is closed.
fn profile_points(st: &CtxState, refs: &[Reference], chordal: f64) -> Result<(Vec<ProfilePoint>, bool), String> {
    let q = QueryCx::new(st);
    let mut edges: Vec<(Reference, Shape)> = Vec::new();
    for r in refs {
        for e in q.from_ref(r) {
            if matches!(e, Ent::Edge(..) | Ent::SketchEdge(..))
                && let Some(s) = q.shape(&e)
                && !edges.iter().any(|(_, x)| x.is_same(&s))
            {
                edges.push((q.to_ref(&e), s));
            }
        }
    }
    if edges.is_empty() {
        return Err("Profiles must be edges".into());
    }
    let shapes: Vec<Shape> = edges.iter().map(|e| e.1.clone()).collect();
    let scale = shapes.iter().filter_map(|e| e.bounding_box(false).ok()).map(|b| b.diagonal()).fold(0.0f64, f64::max).max(1e-6);
    let chain = order_chain(&shapes, scale * 1e-7).map_err(|e| e.message)?;
    let mut pts: Vec<ProfilePoint> = Vec::new();
    for (s, rev) in &chain {
        let Some(j) = shapes.iter().position(|x| x.is_same(s)) else { continue };
        let info = s.edge_info().map_err(|e| e.to_string())?;
        let n = match info.curve {
            k::Curve::Line { .. } => 1,
            k::Curve::Circle { radius, .. } => {
                let step = 2.0 * (1.0 - (chordal / radius).min(1.0)).acos();
                let sweep = info.length / radius;
                ((sweep / step.max(1e-3)).ceil() as usize).clamp(2, 256)
            }
            _ => ((info.length / (chordal * 50.0).max(1e-6)).ceil() as usize).clamp(4, 256),
        };
        for k in 0..=n {
            if k == 0 && !pts.is_empty() {
                continue;
            }
            let u = if *rev { 1.0 - k as f64 / n as f64 } else { k as f64 / n as f64 };
            let pos = s.point_at(info.first_param + (info.last_param - info.first_param) * u).map_err(|e| e.to_string())?;
            pts.push(ProfilePoint { edge: edges[j].0.clone(), t: u, pos });
        }
    }
    let closed = pts.len() > 2 && (pts[0].pos - pts[pts.len() - 1].pos).length() < scale * 1e-7;
    if closed {
        pts.pop();
    }
    // Interior points of straight runs carry no corner.
    let n = pts.len();
    let mut clean = Vec::new();
    for i in 0..n {
        if !closed && (i == 0 || i == n - 1) {
            clean.push(pts[i].clone());
            continue;
        }
        let (a, b, p) = (pts[(i + n - 1) % n].pos, pts[(i + 1) % n].pos, pts[i].pos);
        if (p - a).cross(b - p).length() > 1e-12 * (p - a).length() * (b - p).length() + 1e-18 {
            clean.push(pts[i].clone());
        }
    }
    if clean.len() < 2 {
        return Err("A profile has too few points".into());
    }
    Ok((clean, closed))
}

/// The two profiles' points paired: the second rotated (closed) and/or
/// reversed to lie closest to the first.
fn matched(def: &Value, i: &mut Interp, cx: &FsContext) -> R<(Vec<(ProfilePoint, ProfilePoint)>, bool)> {
    let subs = def.get("profileSubqueries").as_array().map(|a| a.iter().cloned().collect::<Vec<_>>()).unwrap_or_default();
    if subs.len() != 2 {
        return Err(i.error("A tessellated loft needs two profiles"));
    }
    let chordal = def_length_opt(def, "chordalTolerance").unwrap_or(0.005).max(1e-6);
    let r1 = evaluate_refs(i, cx, &subs[0])?;
    let r2 = evaluate_refs(i, cx, &subs[1])?;
    let st = cx.state.lock();
    let (p1, c1) = profile_points(&st, &r1, chordal).map_err(|m| i.error(m))?;
    let (p2, c2) = profile_points(&st, &r2, chordal).map_err(|m| i.error(m))?;
    drop(st);
    if c1 != c2 {
        return Err(i.error("Both profiles must be open, or both closed"));
    }
    if p1.len() != p2.len() {
        return Err(i.error(format!("The profiles have {} and {} corners; the tessellated loft needs the same number in both", p1.len(), p2.len())));
    }
    let n = p1.len();
    let cost = |q: &[ProfilePoint]| p1.iter().zip(q).map(|(a, b)| (a.pos - b.pos).length_squared()).sum::<f64>();
    let mut cands = vec![p2.clone(), p2.iter().rev().cloned().collect::<Vec<_>>()];
    if c1 {
        let base = cands.clone();
        cands.clear();
        for b in base {
            for r in 0..n {
                let mut v = b.clone();
                v.rotate_left(r);
                cands.push(v);
            }
        }
    }
    let best = cands.into_iter().min_by(|a, b| cost(a).total_cmp(&cost(b))).unwrap_or(p2);
    Ok((p1.into_iter().zip(best).collect(), c1))
}

fn ev_tessellated_loft_matches(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let (pairs, closed) = matched(&def, i, &cx)?;
    let item = |p: &ProfilePoint| {
        let mut m = Map::new();
        m.insert(Value::str("edge"), query_of_refs(&cx, std::slice::from_ref(&p.edge)));
        m.insert(Value::str("parameter"), Value::num(p.t));
        m.insert(Value::str("position"), point_value(p.pos));
        Value::map(m)
    };
    let matches: Vec<Value> = pairs
        .iter()
        .map(|(x, y)| {
            let mut m = Map::new();
            m.insert(Value::str("match"), Value::array(vec![item(x), item(y)]));
            m.insert(Value::str("removeRedundantEdge"), Value::bool(false));
            // Computed here, not a user connection.
            m.insert(Value::str("hideMatchFromUI"), Value::bool(false));
            Value::map(m)
        })
        .collect();
    Ok(Value::map_from([("matches", Value::array(matches)), ("isClosed", Value::bool(closed))]))
}

/// The point of a connection on profile `k`: its `k`th edge at its `k`th parameter.
fn connection_point(st: &CtxState, edge: &Reference, t: f64) -> Option<DVec3> {
    let q = QueryCx::new(st);
    let e = q.from_ref(edge).into_iter().next()?;
    let s = q.shape(&e)?;
    let info = s.edge_info().ok()?;
    s.point_at(info.first_param + (info.last_param - info.first_param) * t).ok()
}

fn op_tessellated_loft(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let def = def_arg(i, &a, 2)?;
    // The matched points: from the connections (evTessellatedLoftMatches'), else matched here.
    let conns = def.get("connections").as_array().map(|a| a.iter().cloned().collect::<Vec<_>>()).unwrap_or_default();
    let mut rows: Vec<[DVec3; 2]> = Vec::new();
    for c in &conns {
        let edges = c.get("connectionEdges").as_array().map(|a| a.iter().cloned().collect::<Vec<_>>()).unwrap_or_default();
        let params: Vec<f64> = c.get("connectionEdgeParameters").as_array().map(|a| a.iter().filter_map(|v| v.as_number()).collect()).unwrap_or_default();
        if edges.len() != 2 || params.len() != 2 {
            return Err(i.error("A tessellated loft connection needs one edge per profile"));
        }
        let e0 = evaluate_refs(i, &cx, &edges[0])?;
        let e1 = evaluate_refs(i, &cx, &edges[1])?;
        let st = cx.state.lock();
        let (Some(p0), Some(p1)) = (e0.first().and_then(|r| connection_point(&st, r, params[0])), e1.first().and_then(|r| connection_point(&st, r, params[1]))) else {
            return Err(i.error("A tessellated loft connection could not be resolved"));
        };
        rows.push([p0, p1]);
    }
    let closed = if rows.is_empty() {
        let (pairs, closed) = matched(&def, i, &cx)?;
        rows = pairs.iter().map(|(x, y)| [x.pos, y.pos]).collect();
        closed
    } else {
        // Closed when the profiles are (their first edge's start meets the last's end).
        rows.len() > 2 && {
            let subs = def.get("profileSubqueries").as_array().map(|a| a.iter().cloned().collect::<Vec<_>>()).unwrap_or_default();
            let refs = match subs.first() {
                Some(s) => evaluate_refs(i, &cx, s)?,
                None => Vec::new(),
            };
            let st = cx.state.lock();
            profile_points(&st, &refs, 1.0).map(|(_, c)| c).unwrap_or(false)
        }
    };
    run_op(i, &cx, &id, "LOFT_FAILED", |ev| {
        let n = rows.len();
        if n < 2 {
            return err("A tessellated loft needs at least two matched points");
        }
        let facets = if closed { n } else { n - 1 };
        let mut faces = Vec::new();
        for f in 0..facets {
            let (a, b) = (rows[f], rows[(f + 1) % n]);
            let quad = [a[0], b[0], b[1], a[1]];
            let wire = k::make_polyline(&quad, true).map_err(|e| FeatureError::new(e.to_string()))?;
            match k::make_face_from_wire(&wire, true) {
                Ok(face) => faces.push(face),
                // Not planar: two triangles.
                Err(_) => {
                    for tri in [[a[0], b[0], b[1]], [a[0], b[1], a[1]]] {
                        let w = k::make_polyline(&tri, true).map_err(|e| FeatureError::new(e.to_string()))?;
                        faces.push(k::make_face_from_wire(&w, true).map_err(|e| FeatureError::new(e.to_string()))?);
                    }
                }
            }
        }
        let sewn = if faces.len() == 1 { faces[0].clone() } else { k::sew(&faces, 1e-7, false).map_err(|e| FeatureError::new(e.to_string()))?.shape };
        let names: Vec<Name> = (0..sewn.faces().len()).map(|k| Arc::new(TopoName::face(ev.feature, Role::New(k as u32)))).collect();
        ev.apply_operation(vec![NamedShape::new(sewn, names)], "NEW", None, BodyKind::Surface)
    })?;
    Ok(Value::Undefined)
}
