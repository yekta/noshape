//! `@sheetMetalApplyInFlat` (std sheetMetalBuiltIns.fs, used by Corner
//! break: sheetMetalCornerBreak.fs → SMEdgeBlendImpl): a fillet or chamfer
//! applied in the flat, i.e. to the definition sheet's corner behind each
//! picked part vertex or edge (they share its association attribute). The
//! corner's two straight sides are cut back and joined by a tangent arc
//! (fillet, `radius`) or a line (equal-offsets chamfer, `width`).
//! Corners of one planar definition face are supported (a wall's free
//! corner); corners next to bends are not yet.

use super::query::{Ent, QueryCx, evaluate_refs};
use super::sheetmetal::{associations, sm_attribute};
use super::{context_arg, convert::*, run_op};
use crate::naming::Name;
use crate::reference::BodyId;
use crate::regen::eval::{FResult, FeatureError, err};
use glam::DVec3;
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::Value;
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};

pub const NATIVES: &[(&str, NativeFn)] = &[("sheetMetalApplyInFlat", sheet_metal_apply_in_flat)];

const TOL: f64 = 1e-7;

fn fe(e: impl ToString) -> FeatureError {
    FeatureError::new(e.to_string())
}

/// How a corner is broken.
#[derive(Clone, Copy)]
enum Break {
    Fillet(f64),
    Chamfer(f64),
}

fn sheet_metal_apply_in_flat(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let def = def_arg(i, &a, 2)?;
    let picks = evaluate_refs(i, &cx, def.get("entities"))?;
    let how = if def_enum(&def, "crossSection").as_deref() == Some("CHAMFER") {
        Break::Chamfer(def_length_opt(&def, "width").ok_or_else(|| i.error("sheetMetalApplyInFlat: a chamfer needs width"))?)
    } else {
        Break::Fillet(def_length_opt(&def, "radius").ok_or_else(|| i.error("sheetMetalApplyInFlat: a fillet needs radius"))?)
    };
    // The definition corners behind the picks: (definition body, vertex point).
    let corners: Vec<(BodyId, DVec3)> = {
        let st = cx.state.lock();
        let q = QueryCx::new(&st);
        let mut out = Vec::new();
        for r in &picks {
            let mine = associations(&st, r);
            if mine.is_empty() {
                continue;
            }
            for (b, body) in st.ctx.bodies.iter().enumerate() {
                if sm_attribute(&st, &q.to_ref(&Ent::Body(b)), "MODEL").is_none() {
                    continue;
                }
                for v in 0..body.vertices.len() {
                    if associations(&st, &q.to_ref(&Ent::Vertex(b, v))).iter().any(|x| mine.contains(x))
                        && let Ok(p) = body.shape.sub_shape(ShapeType::Vertex, v).and_then(|s| s.vertex_point())
                        && !out.iter().any(|(id, q): &(BodyId, DVec3)| id == &body.id && q.distance(p) < TOL)
                    {
                        out.push((body.id.clone(), p));
                    }
                }
            }
        }
        out
    };
    if corners.is_empty() {
        return Err(i.error("sheetMetalApplyInFlat: no sheet metal corner selected"));
    }
    run_op(i, &cx, &id, "SHEET_METAL_CORNER_BREAK_FAILED", |ev| {
        for (body_id, v) in &corners {
            let Some(bi) = ev.ctx.bodies.iter().position(|b| &b.id == body_id) else { continue };
            let body = ev.ctx.bodies[bi].clone();
            let faces = body.shape.faces();
            let mut changed = false;
            let mut built = Vec::new();
            for f in &faces {
                match break_corner(f, *v, how)? {
                    Some(nf) => {
                        built.push(nf);
                        changed = true;
                    }
                    None => built.push(f.clone()),
                }
            }
            if !changed {
                return err("Corner break is only supported at a wall's free corner yet");
            }
            let (shape, names) = resewn(&built, &body.faces)?;
            ev.replace_body(bi, shape, names);
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

/// Face `f` with its corner at `v` broken, if `v` is a corner of `f`
/// between two straight sides.
fn break_corner(f: &Shape, v: DVec3, how: Break) -> FResult<Option<Shape>> {
    let Some(_) = f.face_plane().map_err(fe)? else { return Ok(None) };
    let wires = f.wires();
    let [wire] = wires.as_slice() else { return Ok(None) };
    // The outer loop's corners, chained; only straight sides.
    let mut segs: Vec<(DVec3, DVec3)> = Vec::new();
    for e in wire.edges() {
        let info = e.edge_info().map_err(fe)?;
        if !matches!(info.curve, k::Curve::Line { .. }) {
            return Ok(None);
        }
        segs.push((info.start, info.end));
    }
    let (first, mut at) = segs.swap_remove(0);
    let mut pts = vec![first];
    while !segs.is_empty() {
        let Some(k) = segs.iter().position(|(a, b)| a.distance(at) < TOL || b.distance(at) < TOL) else { return Ok(None) };
        let (a, b) = segs.swap_remove(k);
        pts.push(at);
        at = if a.distance(at) < TOL { b } else { a };
    }
    let m = pts.len();
    let Some(k) = pts.iter().position(|p| p.distance(v) < TOL) else { return Ok(None) };
    let (prev, next) = (pts[(k + m - 1) % m], pts[(k + 1) % m]);
    let (d1, d2) = ((prev - v).normalize(), (next - v).normalize());
    let theta = d1.dot(d2).clamp(-1.0, 1.0).acos();
    if theta < 1e-6 || theta > std::f64::consts::PI - 1e-6 {
        return Ok(None);
    }
    let reach = match how {
        Break::Fillet(r) => r / (theta * 0.5).tan(),
        Break::Chamfer(w) => w,
    };
    if reach >= prev.distance(v) - TOL || reach >= next.distance(v) - TOL {
        return err("The corner break is larger than the corner's sides");
    }
    let (p1, p2) = (v + d1 * reach, v + d2 * reach);
    // The new loop: corner k replaced by p1 – (arc or line) – p2.
    let mut edges = Vec::new();
    for j in 0..m {
        let (a, b) = (pts[j], pts[(j + 1) % m]);
        let a = if j == k { p2 } else { a };
        let b = if (j + 1) % m == k { p1 } else { b };
        edges.push(k::make_line(a, b).map_err(fe)?);
        if (j + 1) % m == k {
            edges.push(match how {
                Break::Fillet(r) => {
                    let bis = (d1 + d2).normalize();
                    let centre = v + bis * (r / (theta * 0.5).sin());
                    k::make_arc_3pts(p1, centre - bis * r, p2).map_err(fe)?
                }
                Break::Chamfer(_) => k::make_line(p1, p2).map_err(fe)?,
            });
        }
    }
    let wire = k::make_wire(&edges).map_err(fe)?;
    let mut face = k::make_face_from_wire(&wire, true).map_err(fe)?;
    let normal = |s: &Shape| -> FResult<DVec3> {
        let [u0, u1, v0, v1] = s.uv_bounds().map_err(fe)?;
        s.normal_at_uv((u0 + u1) * 0.5, (v0 + v1) * 0.5).map_err(fe)
    };
    if normal(&face)?.dot(normal(f)?) < 0.0 {
        face = face.reversed().map_err(fe)?;
    }
    Ok(Some(face))
}

/// The faces sewn back into one sheet; each keeps its old face's name.
fn resewn(built: &[Shape], names: &[Name]) -> FResult<(Shape, Vec<Name>)> {
    if built.len() == 1 {
        return Ok((built[0].clone(), names.to_vec()));
    }
    let sewn = k::sew(built, TOL, false).map_err(fe)?.shape;
    let key = |f: &Shape| f.bounding_box(false).map(|b| (b.center(), b.diagonal())).ok();
    let keys: Vec<_> = built.iter().map(key).collect();
    let order = sewn
        .faces()
        .iter()
        .map(|f| {
            let kf = key(f)?;
            let j = keys.iter().position(|k| k.is_some_and(|k| k.0.distance(kf.0) < 1e-9 && (k.1 - kf.1).abs() < 1e-9))?;
            names.get(j).cloned()
        })
        .collect::<Option<Vec<Name>>>()
        .ok_or_else(|| fe("The broken sheet's faces could not be matched"))?;
    Ok((sewn, order))
}
