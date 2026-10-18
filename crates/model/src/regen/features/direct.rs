//! Direct editing: Delete face (heal / cap / leave open), Move face (offset,
//! translate along planar face normals), Replace face (unsupported).

use super::helpers_a::*;
use crate::display::BodyKind;
use crate::naming::{Name, ShapeNames};
use crate::reference::{Role, TopoName};
use crate::regen::context::Body;
use crate::regen::eval::{EvalCtx, FResult, err};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

// ------------------------------------------------------------------ delete face

/// Fillet faces adjacent to the selection (non-planar faces created as blends).
fn adjacent_fillets(body: &Body, faces: &BTreeSet<usize>) -> Vec<usize> {
    let mut out = Vec::new();
    for &f in faces {
        for a in body.shape.adjacent_faces(f).unwrap_or_default() {
            if faces.contains(&a) || out.contains(&a) {
                continue;
            }
            let is_blend = matches!(body.faces[a].root(), TopoName::Face { role: Role::Blend(_), .. });
            let curved = body.shape.sub_shape(ShapeType::Face, a).ok().and_then(|s| s.face_surface().ok()).is_some_and(|s| !matches!(s, k::Surface::Plane(_)));
            if is_blend && curved {
                out.push(a);
            }
        }
    }
    out
}

pub fn delete_face(ev: &mut EvalCtx) -> FResult<()> {
    let groups = by_body(ev.resolve_faces(ev.query("deleteFaces")));
    if groups.is_empty() {
        return err("Select faces to delete");
    }
    let heal = ev.enum_("healType").to_string();
    for (bi, faces) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let mut set: BTreeSet<usize> = faces.into_iter().collect();
        if ev.bool("includeFillet") {
            set.extend(adjacent_fillets(&body, &set));
        }
        let all = body.shape.faces();
        match heal.as_str() {
            "HEAL" => {
                let del: Vec<Shape> = set.iter().map(|f| all[*f].clone()).collect();
                let res = k::delete_faces(&body.shape, &del).map_err(kerr("Delete face failed (the neighboring faces could not be extended to close the gap)"))?;
                let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), None);
                ev.replace_body(bi, res.shape, names);
            }
            "CAP" | "VOID" => {
                let keep: Vec<usize> = (0..all.len()).filter(|f| !set.contains(f)).collect();
                if keep.is_empty() {
                    return err("Cannot delete every face of a part");
                }
                let mut inputs: Vec<Shape> = keep.iter().map(|f| all[*f].clone()).collect();
                let mut arg_names: Vec<ShapeNames> = keep.iter().map(|f| ShapeNames { faces: vec![body.faces[*f].clone()], ..Default::default() }).collect();
                let cap = heal == "CAP";
                if cap {
                    let ef = body.shape.edge_faces()?;
                    let sel: BTreeSet<u32> = set.iter().map(|f| *f as u32).collect();
                    let boundary: Vec<Shape> = ef
                        .iter()
                        .enumerate()
                        .filter(|(_, fs)| fs.iter().any(|f| sel.contains(f)) && fs.iter().any(|f| !sel.contains(f)))
                        .filter_map(|(ei, _)| body.shape.sub_shape(ShapeType::Edge, ei).ok())
                        .collect();
                    let wires = k::edges_to_wires(&Shape::compound(&boundary), 1e-6)?;
                    for (c, face) in cap_faces(&wires)?.into_iter().enumerate() {
                        inputs.push(face);
                        arg_names.push(ShapeNames { faces: vec![Arc::new(TopoName::face(ev.feature, Role::New(c as u32)))], ..Default::default() });
                    }
                }
                let res = k::sew(&inputs, 1e-6, cap).map_err(kerr("Delete face failed"))?;
                let (shape, kind) = if cap {
                    match res.shape.solids().into_iter().next() {
                        Some(s) => (s, body.kind),
                        None => return err("Delete face: the capped faces don't form a closed part"),
                    }
                } else {
                    (res.shape.clone(), BodyKind::Surface)
                };
                let refs: Vec<&ShapeNames> = arg_names.iter().collect();
                let raw = ev.name_result(&refs, &res.history, &res.shape, &BTreeMap::new(), None);
                let names: Vec<Name> = shape.faces().iter().map(|f| res.shape.index_of(f, ShapeType::Face).and_then(|i| raw.get(i).cloned()).unwrap_or_else(|| Arc::new(TopoName::face(ev.feature, Role::New(999))))).collect();
                let names = dedup_names(ev, names);
                let nb = Body::new(body.id.clone(), body.name.clone(), kind, shape, names, body.color, ev.feature);
                ev.ctx.bodies[bi] = Arc::new(nb);
            }
            other => return err(format!("Unknown delete face type {other}")),
        }
    }
    Ok(())
}

/// Faces closing boundary loops: coplanar nested loops become one face with
/// holes; non-planar loops get a filling face.
fn cap_faces(wires: &[Shape]) -> FResult<Vec<Shape>> {
    let mut planar: Vec<(k::Plane, f64, Shape)> = Vec::new();
    let mut out = Vec::new();
    for w in wires {
        let f = k::make_face_from_wire(w, false).map_err(|_| fe("Delete face: the opening could not be capped"))?;
        match f.face_plane()? {
            Some(pl) => planar.push((pl, f.area().unwrap_or(0.0), w.clone())),
            None => out.push(f),
        }
    }
    planar.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut used = vec![false; planar.len()];
    for i in 0..planar.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let (pl, _, outer) = &planar[i];
        let n = pl.unit_normal();
        let mut holes = Vec::new();
        for j in (i + 1)..planar.len() {
            let (pj, _, wj) = &planar[j];
            if !used[j] && pj.unit_normal().dot(n).abs() > 1.0 - 1e-9 && pl.signed_distance(pj.origin).abs() < 1e-7 {
                used[j] = true;
                holes.push(wj.clone());
            }
        }
        out.push(k::make_face(pl, outer, &holes).map_err(|_| fe("Delete face: the opening could not be capped"))?);
    }
    Ok(out)
}

// ------------------------------------------------------------------ move face

pub fn move_face(ev: &mut EvalCtx) -> FResult<()> {
    if ev.is("outputType", "CREATE") {
        return err("Move face \"Create\" output is not supported yet");
    }
    let ty = ev.enum_("moveFaceType").to_string();
    if ty == "ROTATE" {
        return err("Rotating faces is not supported yet");
    }
    if ev.is("limitType", "UP_TO_ENTITY") {
        return err("Move face \"Up to entity\" is not supported yet");
    }
    let groups = by_body(ev.resolve_faces(ev.query("moveFaces")));
    if groups.is_empty() {
        return err("Select faces to move");
    }
    let sign = if ev.bool("oppositeDirection") { -1.0 } else { 1.0 };
    let translate_dir = if ty == "TRANSLATE" {
        let Some(r) = ev.query("direction").first() else { return err("Select a direction") };
        Some(ev.resolve_direction(r).ok_or_else(|| fe("The direction could not be resolved"))?.normalize())
    } else {
        None
    };
    let dist = if ty == "TRANSLATE" { ev.length("translationDistance")? } else { ev.length("offsetDistance")? } * sign;
    for (bi, faces) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let shapes: Vec<Shape> = faces.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
        let res = match translate_dir {
            None => k::offset_faces(&body.shape, &shapes, dist).map_err(kerr("Move face failed"))?,
            Some(d) => {
                // Translation of planar faces along their normal = offset.
                let mut each = Vec::new();
                for s in &shapes {
                    let Some(pl) = s.face_plane()? else { return err("Translating non-planar faces is not supported yet") };
                    let c = pl.unit_normal().dot(d);
                    if (c.abs() - 1.0).abs() > 1e-6 {
                        return err("Faces can only be translated along their normal so far");
                    }
                    each.push(dist * c);
                }
                k::offset_faces_each(&body.shape, &shapes, &each).map_err(kerr("Move face failed"))?
            }
        };
        let offset_role = |n: &Name| Role::Offset(n.clone());
        let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), Some(&offset_role));
        ev.replace_body(bi, res.shape, names);
    }
    Ok(())
}

// ------------------------------------------------------------------ replace face

pub fn replace_face(_ev: &mut EvalCtx) -> FResult<()> {
    err("Replace face is not supported by the geometry kernel yet")
}
