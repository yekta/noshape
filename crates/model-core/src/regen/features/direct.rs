//! Direct editing: Delete face (heal / cap / leave open), Move face (offset,
//! translate, rotate, up to entity, create surfaces), Replace face.

use super::helpers_a::*;
use crate::display::BodyKind;
use crate::naming::{Name, ShapeNames};
use crate::reference::{Reference, Role, TopoName};
use crate::regen::context::Body;
use crate::regen::eval::{EvalCtx, FResult, err};
use glam::{DMat4, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

// ------------------------------------------------------------------ delete face

/// Fillet faces next to the selection, recognised from the geometry (live
/// L11: also on imported parts): a cylinder, torus, sphere or B-spline
/// (rolling-ball) face that is tangent to a deleted face along their edge and
/// to another face along another edge. Fillets tangent to fillets found this
/// way follow (a fillet chain). Faces our Fillet feature made count too.
fn adjacent_fillets(body: &Body, faces: &BTreeSet<usize>) -> Vec<usize> {
    let all = body.shape.faces();
    let Ok(ef) = body.shape.edge_faces() else { return Vec::new() };
    let edges = body.shape.edges();
    let blend_like = |f: usize| {
        let named = matches!(body.faces[f].root(), TopoName::Face { role: Role::Blend(_), .. });
        let shape = all[f].face_surface().ok();
        named || matches!(shape, Some(k::Surface::Cylinder { .. } | k::Surface::Torus { .. } | k::Surface::Sphere { .. } | k::Surface::BSpline { .. }))
    };
    // Whether faces a and b meet tangentially along edge e.
    let tangent = |e: usize, a: usize, b: usize| -> bool {
        let Some(m) = edges.get(e).and_then(|x| x.edge_info().ok().and_then(|i| x.point_at((i.first_param + i.last_param) * 0.5).ok())) else { return false };
        match (face_normal_near(&all[a], m), face_normal_near(&all[b], m)) {
            (Some((_, na)), Some((_, nb))) => na.normalize().dot(nb.normalize()) > 1.0 - 1e-6,
            _ => false,
        }
    };
    // (neighbour, edge) pairs of a face.
    let neighbours = |f: usize| -> Vec<(usize, usize)> {
        ef.iter()
            .enumerate()
            .filter_map(|(e, fs)| match fs[..] {
                [a, b] if a as usize == f && b as usize != f => Some((b as usize, e)),
                [a, b] if b as usize == f && a as usize != f => Some((a as usize, e)),
                _ => None,
            })
            .collect()
    };
    let mut seeds: BTreeSet<usize> = faces.clone();
    let mut out: Vec<usize> = Vec::new();
    loop {
        let mut found = Vec::new();
        for &s in &seeds {
            for (f, e) in neighbours(s) {
                if seeds.contains(&f) || out.contains(&f) || found.contains(&f) || !blend_like(f) || !tangent(e, s, f) {
                    continue;
                }
                // Tangent to something else too (a fillet joins two faces).
                if neighbours(f).into_iter().any(|(g, e2)| g != s && e2 != e && tangent(e2, f, g)) {
                    found.push(f);
                }
            }
        }
        if found.is_empty() {
            break;
        }
        out.extend(found.iter().copied());
        seeds.extend(found);
    }
    out
}

/// Onshape's DIRECT_EDIT_DELETE_FAILED (live, behaviours "Delete face").
const DELETE_FAILED: &str = "Could not delete selected faces.";

/// The kernel's reason isn't shown (opDeleteFace).
fn failed<E>(_: E) -> crate::regen::eval::FeatureError {
    fe(DELETE_FAILED)
}

pub fn delete_face(ev: &mut EvalCtx) -> FResult<()> {
    let groups = by_body(ev.resolve_faces(ev.query("deleteFaces")));
    if groups.is_empty() {
        return crate::regen::eval::err_at(&["deleteFaces"], "Select faces to delete.");
    }
    let heal = ev.enum_("healType").to_string();
    for (bi, faces) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let mut set: BTreeSet<usize> = faces.into_iter().collect();
        let fillets = adjacent_fillets(&body, &set);
        if ev.bool("includeFillet") {
            set.extend(fillets);
        } else if heal == "HEAL" && !fillets.is_empty() {
            // Live L11: a fillet left around the deleted faces can't be
            // healed (Parasolid); OCCT would stretch it to close the gap.
            return err(DELETE_FAILED);
        }
        let all = body.shape.faces();
        // Live: deleting every face removes the part, with no message.
        if set.len() >= all.len() {
            let id = body.id.clone();
            ev.ctx.bodies.retain(|b| b.id != id);
            continue;
        }
        match heal.as_str() {
            "HEAL" => {
                let del: Vec<Shape> = set.iter().map(|f| all[*f].clone()).collect();
                let res = k::delete_faces(&body.shape, &del).map_err(failed)?;
                // Live 1668 (d): the top and bottom of a block can't be
                // healed; OCCT hands the block back unchanged. A face left in place (OCCT out of its work bound:
                // c2-impeller-loft's Delete face 2) fails too, never a part with it as if deleted.
                let left = set.iter().any(|&f| !res.history.modified(0, ShapeType::Face, f as u32).is_empty());
                if left || res.shape.faces().len() >= all.len() || !res.shape.is_valid() {
                    return err(DELETE_FAILED);
                }
                let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), None);
                ev.replace_body(bi, res.shape, names);
            }
            "CAP" | "VOID" => {
                let keep: Vec<usize> = (0..all.len()).filter(|f| !set.contains(f)).collect();
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
                    let wires = k::edges_to_wires(&Shape::compound(&boundary), 1e-6).map_err(failed)?;
                    for (c, face) in cap_faces(&wires)?.into_iter().enumerate() {
                        inputs.push(face);
                        arg_names.push(ShapeNames { faces: vec![Arc::new(TopoName::face(ev.feature, Role::New(c as u32)))], ..Default::default() });
                    }
                }
                let res = k::sew(&inputs, 1e-6, cap).map_err(failed)?;
                let (shape, kind) = if cap {
                    match res.shape.solids().into_iter().next() {
                        Some(s) => (s, body.kind),
                        None => return err(DELETE_FAILED),
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
        let f = k::make_face_from_wire(w, false).map_err(|_| fe(DELETE_FAILED))?;
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
        out.push(k::make_face(pl, outer, &holes).map_err(|_| fe(DELETE_FAILED))?);
    }
    Ok(out)
}

// ------------------------------------------------------------------ move face

/// Point of a limit entity and, for planar entities, its normal.
/// Onshape marks `limitEntity` for both limit fields (moveFace.fs).
fn limit_point(ev: &EvalCtx, id: &str) -> FResult<(DVec3, Option<DVec3>)> {
    let unresolved = || crate::regen::eval::err_at(&["limitEntity"], "Cannot resolve entities.");
    let Some(r) = ev.query(id).first() else { return unresolved() };
    if let Some(f) = ev.resolve_plane(r)
        && matches!(r, Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. } | Reference::Face(_))
    {
        return Ok((f.origin, Some(f.normal)));
    }
    if let Some(p) = ev.resolve_vertex_point(r) {
        return Ok((p, None));
    }
    if let Some(&(bi, ei)) = ev.resolve_edges(std::slice::from_ref(r)).first() {
        let (a, b) = ev.edge_shape(bi, ei)?.endpoints()?;
        return Ok(((a + b) * 0.5, None));
    }
    if let Some(&(bi, fi)) = ev.resolve_faces(std::slice::from_ref(r)).first() {
        return Ok((ev.face_shape(bi, fi)?.centroid()?, None));
    }
    unresolved()
}

/// Move faces by a rigid transform: remove them (healing the gap with the
/// neighbours), then re-apply the region they bounded at the new place.
pub fn move_rigid(ev: &mut EvalCtx, bi: usize, faces: &[usize], m: &DMat4) -> FResult<()> {
    let body = ev.ctx.bodies[bi].clone();
    let shapes: Vec<Shape> = faces.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
    let healed = k::delete_faces(&body.shape, &shapes).map_err(kerr("Could not transform selected faces as requested."))?;
    let h_names = ev.name_result(&[&body.shape_names()], &healed.history, &healed.shape, &BTreeMap::new(), None);
    let healed_n = crate::regen::eval::names_for(&healed.shape, h_names);
    let (v0, v1) = (body.shape.volume()?, healed.shape.volume()?);
    let pocket = v1 > v0;
    // The region the faces bound (material removed by a pocket, or a boss).
    let region = if pocket { k::cut(&healed.shape, &body.shape)? } else { k::cut(&body.shape, &healed.shape)? };
    if region.shape.count(ShapeType::Solid) == 0 {
        return err("Could not transform selected faces as requested.");
    }
    let r_names = ev.name_result(&[&ShapeNames::default(), &body.shape_names()], &region.history, &region.shape, &BTreeMap::new(), None);
    let r_named = crate::regen::eval::names_for(&region.shape, r_names);
    let moved = k::transform(&region.shape, &k::Transform::Matrix(*m), true)?;
    let m_names = ev.name_result(&[&r_named], &moved.history, &moved.shape, &BTreeMap::new(), None);
    let m_named = crate::regen::eval::names_for(&moved.shape, m_names);
    let op = if pocket { k::BooleanOp::Cut } else { k::BooleanOp::Fuse };
    let res = k::boolean(op, std::slice::from_ref(&healed.shape), std::slice::from_ref(&moved.shape), &k::BooleanOptions::default())?;
    let names = ev.name_result(&[&healed_n, &m_named], &res.history, &res.shape, &BTreeMap::new(), None);
    let un = k::unify_same_domain(&res.shape)?;
    let rn = crate::regen::eval::names_for(&res.shape, names);
    let names = ev.name_result(&[&rn], &un.history, &un.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, un.shape, names);
    Ok(())
}

/// Move faces by a rigid transform in place, their neighbours re-trimmed to them (no new topology): false where that
/// can't be done (the caller then cuts and adds the region, [`move_rigid`]).
fn tweak(ev: &mut EvalCtx, bi: usize, faces: &[usize], m: &DMat4) -> bool {
    let body = ev.ctx.bodies[bi].clone();
    let shapes: Vec<Shape> = faces.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
    let res = match k::tweak_faces(&body.shape, &shapes, &k::Transform::Matrix(*m)) {
        Ok(r) if r.shape.count(ShapeType::Solid) == 1 && r.shape.is_valid() => r,
        other => {
            log::debug!("move face: tweak declined ({}), cutting and adding the region", other.map_or_else(|e| e.to_string(), |_| "invalid".into()));
            return false;
        }
    };
    let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, res.shape, names);
    true
}

/// Planes offset as translations along their outward normals, the planes facing one way at a time, their neighbours
/// re-trimmed in place: each step's way, points on its faces, and result. None unless every face is a
/// plane and every step makes one valid solid (asm-scissor-lift Studio b58244's Move face 6: two notches' floors
/// tilted apart; OCCT's offset was invalid and the booleans' slabs fell short of the tilted walls).
fn tweak_planes_in_turn(shape: &Shape, faces: &[usize], dist: f64) -> Option<Vec<(DVec3, Vec<DVec3>, k::OpResult)>> {
    let all = shape.faces();
    // Each face's outward normal and a point inside it (to find it again after the steps before its own).
    let mut groups: Vec<(DVec3, Vec<DVec3>)> = Vec::new();
    for &fi in faces {
        let f = all.get(fi)?;
        f.face_plane().ok()??;
        let p = inner_point(f)?;
        let n = f.normal_at_point(p).ok()?;
        match groups.iter_mut().find(|(m, _)| m.dot(n) > 1.0 - 1e-12) {
            Some(g) => g.1.push(p),
            None => groups.push((n, vec![p])),
        }
    }
    // Planes facing several ways only among planes (a curved neighbour's crossings left to OCCT's offset).
    if groups.len() > 1 {
        let ef = shape.edge_faces().ok()?;
        let beside = ef.iter().filter(|fs| fs.iter().any(|f| faces.contains(&(*f as usize)))).flatten();
        if beside.clone().any(|&f| !all.get(f as usize).is_some_and(|x| x.is_planar())) {
            return None;
        }
    }
    let mut cur = shape.clone();
    let mut steps = Vec::new();
    for (n, points) in groups {
        let now = cur.faces();
        let moved = planes_at(&cur, n, &points)?;
        let shapes: Vec<Shape> = moved.iter().map(|&i| now[i].clone()).collect();
        let r = k::tweak_faces(&cur, &shapes, &k::Transform::Matrix(DMat4::from_translation(n * dist))).ok().filter(|r| r.shape.count(ShapeType::Solid) == 1 && r.shape.is_valid())?;
        cur = r.shape.clone();
        steps.push((n, points, r));
    }
    Some(steps)
}

/// [`tweak_planes_in_turn`] as one operation, its steps' histories followed through.
fn tweak_planes(shape: &Shape, faces: &[Shape], dist: f64) -> Option<k::OpResult> {
    let idx: Vec<usize> = faces.iter().map(|f| shape.index_of(f, ShapeType::Face)).collect::<Option<_>>()?;
    let mut steps = tweak_planes_in_turn(shape, &idx, dist)?.into_iter().map(|(_, _, r)| r);
    let first = steps.next()?;
    Some(steps.fold(first, |acc, r| k::OpResult { history: acc.history.then(&r.history), shape: r.shape }))
}

/// The planar faces of `shape` through `points`, facing `n`.
fn planes_at(shape: &Shape, n: DVec3, points: &[DVec3]) -> Option<Vec<usize>> {
    let all = shape.faces();
    points.iter().map(|p| all.iter().position(|f| f.is_planar() && f.normal_at_point(*p).is_ok_and(|m| m.dot(n) > 1.0 - 1e-12) && f.contains_point(*p, 1e-9).unwrap_or(false))).collect()
}

/// Whether a face in `faces` of `shape` or one sharing an edge with them is a freeform surface (not a plane, cylinder,
/// cone, sphere or torus).
fn touches_freeform(shape: &Shape, faces: &[usize]) -> bool {
    let all = shape.faces();
    let Ok(ef) = shape.edge_faces() else { return false };
    let mut near: std::collections::BTreeSet<usize> = faces.iter().copied().collect();
    for fs in &ef {
        if fs.iter().any(|f| faces.contains(&(*f as usize))) {
            near.extend(fs.iter().map(|f| *f as usize));
        }
    }
    near.iter().any(|&f| all.get(f).is_some_and(|x| !matches!(x.face_surface(), Ok(k::Surface::Plane(_) | k::Surface::Cylinder { .. } | k::Surface::Cone { .. } | k::Surface::Sphere { .. } | k::Surface::Torus { .. }))))
}

/// A moved face beside an unmoved one on its own surface (std's thread undercut: the 3 mm end of a split shaft):
/// OCCT's offset moves both, live steps between them.
fn beside_its_own_surface(shape: &Shape, faces: &[usize]) -> bool {
    let all = shape.faces();
    let Ok(ef) = shape.edge_faces() else { return false };
    let same = |a: usize, b: usize| match (all.get(a).map(|f| f.face_surface()), all.get(b).map(|f| f.face_surface())) {
        (Some(Ok(k::Surface::Cylinder { origin: o1, axis: a1, radius: r1, .. })), Some(Ok(k::Surface::Cylinder { origin: o2, axis: a2, radius: r2, .. }))) => {
            let a1 = a1.normalize();
            (r1 - r2).abs() < 1e-9 && a1.cross(a2.normalize()).length() < 1e-9 && (o2 - o1).reject_from_normalized(a1).length() < 1e-9
        }
        _ => false,
    };
    ef.iter().any(|fs| match fs.as_slice() {
        &[a, b] => faces.contains(&(a as usize)) != faces.contains(&(b as usize)) && same(a as usize, b as usize),
        _ => false,
    })
}

pub fn move_face(ev: &mut EvalCtx) -> FResult<()> {
    let ty = ev.enum_("moveFaceType").to_string();
    // moveFace.fs: the faces are checked before the direction or axis.
    let create = ev.is("outputType", "CREATE");
    let groups = by_body(ev.resolve_faces(ev.query("moveFaces")));
    if !create && groups.is_empty() {
        return crate::regen::eval::err_at(&["moveFaces"], "Select faces to move.");
    }
    let sign = if ev.bool("oppositeDirection") { -1.0 } else { 1.0 };
    let up_to = ev.is("limitType", "UP_TO_ENTITY") && ty != "ROTATE";
    let extra = if up_to && ev.bool("hasOffset") { ev.length("offset")? * if ev.bool("oppositeOffsetDirection") { -1.0 } else { 1.0 } } else { 0.0 };
    let translate_dir = if ty == "TRANSLATE" {
        let no_direction = || crate::regen::eval::err_at(&["direction"], "Cannot determine translation direction.");
        let Some(r) = ev.query("direction").first() else { return no_direction() };
        let Some(d) = ev.resolve_direction(r) else { return no_direction() };
        Some(d.normalize())
    } else {
        None
    };
    let rotation = if ty == "ROTATE" {
        // Live L5: "Cannot determine rotation axis." on Axis.
        let no_axis = || crate::regen::eval::err_at(&["axis"], "Cannot determine rotation axis.");
        let Some(r) = ev.query("axis").first() else { return no_axis() };
        let Some((o, a)) = ev.resolve_axis(r) else { return no_axis() };
        Some((o, a.normalize(), ev.angle("angle")? * sign))
    } else {
        None
    };
    if create {
        return move_face_create(ev, &ty, translate_dir, rotation, sign);
    }
    // Onshape's texts: the kernel's reason isn't shown.
    let failed_msg = if ty == "OFFSET" { "Could not offset selected faces as requested." } else { "Could not transform selected faces as requested." };
    let moved: Vec<usize> = groups.keys().copied().collect();
    for (bi, faces) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let shapes: Vec<Shape> = faces.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
        if let Some((o, a, ang)) = rotation {
            if ang.abs() < 1e-12 {
                continue;
            }
            let m = DMat4::from_translation(o) * DMat4::from_axis_angle(a, ang) * DMat4::from_translation(-o);
            if shapes.iter().all(|f| f.is_planar()) {
                // Tilt planar faces: replace each with its rotated plane.
                let extent = ev.model_extent();
                for (f, &fi) in shapes.iter().zip(&faces) {
                    let Some(pl) = f.face_plane()? else { continue };
                    let rp = k::Plane::new(m.transform_point3(pl.origin), m.transform_vector3(pl.unit_normal()), m.transform_vector3(pl.unit_x()));
                    let template = k::make_plane_face(&rp, -extent, extent, -extent, extent)?;
                    let name = body.faces[fi].clone();
                    replace_with(ev, bi, &name, &template, 1.0).map_err(|_| fe(failed_msg))?;
                }
            } else if !tweak(ev, bi, &faces, &m) {
                move_rigid(ev, bi, &faces, &m).map_err(|_| fe(failed_msg))?;
            }
            continue;
        }
        // Live dl-bracket-1125: a mesh part is one face, which Offset offsets whole (its convex edges rounded) and
        // Translate moves whole (mesh-offset-1125).
        if body.shape.is_mesh() && !up_to {
            let dist = ev.length(if translate_dir.is_some() { "translationDistance" } else { "offsetDistance" })? * sign;
            if dist.abs() < 1e-12 {
                continue;
            }
            let shape = match translate_dir {
                Some(d) => body.shape.translated(d * dist),
                None => body.shape.offset_mesh(dist),
            }
            .map_err(|_| fe(failed_msg))?;
            let feature = ev.feature;
            let names = shape.faces().iter().map(|_| Arc::new(TopoName::face(feature, Role::Offset(body.faces[faces[0]].clone())))).collect();
            ev.replace_body(bi, shape, names);
            continue;
        }
        // Distance (blind or up to an entity).
        let c0 = shapes.first().ok_or_else(|| fe("Select faces to move."))?.centroid()?;
        let dist = if up_to {
            let id = if ty == "TRANSLATE" { "limitEntity" } else { "limitQuery" };
            let (p, pn) = limit_point(ev, id)?;
            let face_n = shapes[0].face_plane()?.map(|pl| pl.unit_normal());
            let parallel = |n: DVec3, m: DVec3| (n.dot(m).abs() - 1.0).abs() < 1e-9;
            let d = match (translate_dir, pn) {
                // Translate up to a plane: the first face must be planar and
                // parallel to it (getTranslationTransformToEntity).
                (Some(along), Some(n)) => {
                    let Some(fnorm) = face_n else { return crate::regen::eval::err_at(&["moveFaces"], "When moving up to a plane, first selected face must be planar.") };
                    if !parallel(fnorm, n) {
                        return err("Limit face must be parallel to face being moved.");
                    }
                    if along.dot(n).abs() < 1e-9 {
                        return crate::regen::eval::err_at(&["moveFaces"], "First face cannot be moved to limit entity.");
                    }
                    (p - c0).dot(n) / along.dot(n)
                }
                (Some(along), None) => (p - c0).dot(along),
                // Offset up to an entity: along the face normal; a plane that
                // isn't parallel meets the extended face, so the distance is 0.
                (None, Some(n)) => {
                    let along = face_n.map_or_else(|| shapes[0].normal_at_point(c0), Ok)?;
                    if parallel(along, n) { (p - c0).dot(along) } else { 0.0 }
                }
                (None, None) => (p - c0).dot(shapes[0].normal_at_point(c0)?),
            };
            d + extra
        } else if ty == "TRANSLATE" {
            ev.length("translationDistance")? * sign
        } else {
            ev.length("offsetDistance")? * sign
        };
        // A 0 distance succeeds and changes nothing (V484_MOVE_FACE_0_DISTANCE).
        if dist.abs() < 1e-12 {
            continue;
        }
        // moveFace.fs "Reapply fillet" (default on): the fillets beside the
        // moved faces are removed and rolled again at their radii.
        if ev.params.get("reFillet").is_none_or(|_| ev.bool("reFillet")) {
            let blends = super::detail_a::blends_to_reapply(&body.shape, &body.faces, &faces, false)?;
            if !blends.is_empty() {
                let td = translate_dir;
                super::detail_a::reblend(ev, bi, &faces, &blends, failed_msg, |shape, fs| match td {
                    // OCCT's offset, else (invalid: b58244's Move face 6, two notches' floors tilted apart, their rounds
                    // removed) the planes moved one way at a time.
                    None => match k::offset_faces(shape, fs, dist) {
                        Ok(r) if r.shape.is_valid() && r.shape.count(ShapeType::Solid) == 1 => Ok(r),
                        other => tweak_planes(shape, fs, dist).map_or(other, Ok),
                    },
                    Some(d) => {
                        let each: Vec<f64> = fs.iter().map(|s| s.face_plane().ok().flatten().map_or(0.0, |pl| dist * pl.unit_normal().dot(d))).collect();
                        k::offset_faces_each(shape, fs, &each)
                    }
                })?;
                continue;
            }
        }
        // Planes facing one way: their offset is a translation along that way, the neighbours re-trimmed in place
        // (c2-impeller-loft's Flansch (vorne) verbreitern: OCCT's offset 0.8 s and invalid, then the booleans).
        let tweaked = if translate_dir.is_none() { tweak_planes_in_turn(&body.shape, &faces, dist) } else { None };
        if let Some(steps) = tweaked {
            for (n, points, r) in steps {
                let body = ev.ctx.bodies[bi].clone();
                let fs = planes_at(&body.shape, n, &points).ok_or_else(|| fe(failed_msg))?;
                // A moved face the Offset of the one it was, the others keeping their names, as the booleans named them
                // (v0.4.58's c2-impeller-loft Flansch (vorne) verbreitern, OCCT's offset invalid: a fillet's edges); the
                // booleans' piece of the moved face (Move face 2's faces) and OCCT's offset names (every face the Offset of
                // the one it was) still find them.
                let kept = ev.name_result(&[&body.shape_names()], &r.history, &r.shape, &BTreeMap::new(), None);
                let moved: BTreeSet<&Name> = fs.iter().map(|&fi| &body.faces[fi]).collect();
                let feature = ev.feature;
                let offset = |k: &Name| -> Name { Arc::new(TopoName::face(feature, Role::Offset(k.clone()))) };
                let names: Vec<Name> = kept.iter().map(|k| if moved.contains(k) { offset(k) } else { k.clone() }).collect();
                let old = kept.iter().zip(&names).map(|(k, n)| if moved.contains(k) { (Arc::new(TopoName::Split { parent: n.clone(), by: feature, ordinal: 0 }), n.clone()) } else { (offset(k), n.clone()) });
                ev.ctx.bodies.add_aliases(old.collect::<Vec<_>>());
                ev.replace_body(bi, r.shape, names);
            }
            continue;
        }
        // Else, faces on or beside the moved ones that aren't planes, cylinders, cones, spheres or tori (a flange's walls
        // extruded from a loft's): OCCT's offset worked on them for ~0.6 s and left an invalid solid the booleans
        // then made (c2-impeller-loft's Flansch verbreitern): the booleans first, OCCT's offset if they fail.
        if translate_dir.is_none() && (touches_freeform(&body.shape, &faces) || beside_its_own_surface(&body.shape, &faces)) && offset_by_booleans(ev, bi, &faces, dist).is_ok() {
            continue;
        }
        // A cylinder offset to its axis goes whole: OCCT's offset turns it inside out (live mf-cyl-neg: no r 2 boss).
        if translate_dir.is_none() && shapes.iter().any(|f| offset_past_axis(f, dist).unwrap_or(false)) {
            offset_by_booleans(ev, bi, &faces, dist).map_err(|_| fe(failed_msg))?;
            continue;
        }
        let res = match translate_dir {
            None => match k::offset_faces(&body.shape, &shapes, dist) {
                Ok(r) if r.shape.is_valid() && r.shape.count(ShapeType::Solid) > 0 => r,
                // OCCT can't rebuild the topology (the face passes through a hole,
                // a hole closes or splits the part): booleans.
                other => {
                    if let Err(e) = &other {
                        log::debug!("move face: offset failed ({e}), trying booleans");
                    }
                    offset_by_booleans(ev, bi, &faces, dist).map_err(|e| {
                        log::debug!("move face: booleans failed ({})", e.message);
                        fe(failed_msg)
                    })?;
                    continue;
                }
            },
            Some(d) => {
                // Planar faces moved along their normal: offset (neighbours follow).
                let mut each = Vec::new();
                let mut rigid = false;
                for s in &shapes {
                    match s.face_plane()? {
                        Some(pl) if (pl.unit_normal().dot(d).abs() - 1.0).abs() < 1e-6 => each.push(dist * pl.unit_normal().dot(d)),
                        _ => rigid = true,
                    }
                }
                if rigid {
                    move_rigid(ev, bi, &faces, &DMat4::from_translation(d * dist)).map_err(|_| fe(failed_msg))?;
                    continue;
                }
                match k::offset_faces_each(&body.shape, &shapes, &each) {
                    Ok(r) if r.shape.is_valid() && r.shape.count(ShapeType::Solid) > 0 => r,
                    // As Offset: booleans when every face moves the same way
                    // (else a face moved through the far side came out invalid).
                    _ if each.iter().all(|e| (e - each[0]).abs() < 1e-12) => {
                        offset_by_booleans(ev, bi, &faces, each[0]).map_err(|_| fe(failed_msg))?;
                        continue;
                    }
                    _ => return err(failed_msg),
                }
            }
        };
        let offset_role = |n: &Name| Role::Offset(n.clone());
        let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), Some(&offset_role));
        ev.replace_body(bi, res.shape, names);
    }
    // A part moved to nothing (live mf-side-through: a side offset the part's length in) fails. (Its volume by the
    // plain estimate first: the exact one took 135 ms of a threaded screw's Move face.)
    let nothing = |s: &Shape| k::volume_estimate(s).is_ok_and(|v| v <= 1e-12) && s.volume().is_ok_and(|v| v <= 1e-15);
    if moved.iter().any(|&bi| ev.ctx.bodies.get(bi).is_some_and(|b| b.kind == BodyKind::Solid && nothing(&b.shape))) {
        return err(failed_msg);
    }
    Ok(())
}

/// Whether a round cylinder face offset by `d` along its outward normal reaches its axis: a hole closing, a boss going
/// (live mf-cyl-r0 / mf-cyl-neg: the r 4 boss offset 4 or 6 mm in is removed).
fn offset_past_axis(f: &Shape, d: f64) -> FResult<bool> {
    let k::Surface::Cylinder { origin, axis, radius, .. } = f.face_surface()? else { return Ok(false) };
    // A point on the face (a full cylinder's centroid is on its axis).
    let c = f.project_point(f.centroid()? + axis.any_orthonormal_vector() * radius)?.point;
    let toward_axis = f.normal_at_point(c)?.dot((origin + axis * (c - origin).dot(axis)) - c) > 0.0;
    let inward = if toward_axis { d } else { -d };
    Ok(inward >= radius * (1.0 - 1e-9) && f.edges().iter().any(|e| e.is_closed().unwrap_or(false)))
}

/// The region a face sweeps when offset by `d` along its outward normal (d < 0:
/// into the part). A cylinder face offset past its axis (a hole closing)
/// sweeps the whole cylinder.
fn face_slab(f: &Shape, d: f64) -> FResult<Shape> {
    if let k::Surface::Cylinder { origin, axis, radius, .. } = f.face_surface()?
        && offset_past_axis(f, d)?
    {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for v in f.vertices() {
            let t = (v.vertex_point()? - origin).dot(axis);
            lo = lo.min(t);
            hi = hi.max(t);
        }
        return Ok(k::make_cylinder(&k::Plane::from_normal(origin + axis * lo, axis), radius, hi - lo, std::f64::consts::TAU)?);
    }
    // Thicken, then make sure the slab is on the requested side: probed off a point on the face (a flange's face has
    // its centroid in its bore: asm-scissor-lift 404ddb's Move face 1).
    let p = match inner_point(f) {
        Some(p) => p,
        None => f.project_point(f.centroid()?)?.point,
    };
    let n = f.normal_at_point(p)?;
    let probe = p + n * (d * 0.5);
    // A plane's slab is its prism: walls on the surfaces of the walls it extends (exact cylinders for its holes), so
    // they merge with them (404ddb's Move face 1: live's 14 faces, not 22).
    if f.is_planar()
        && let Ok(r) = k::extrude(f, n * d)
        && r.shape.is_valid()
        && r.shape.classify_point(probe, d.abs() * 1e-6)? == k::PointClass::Inside
    {
        return Ok(r.shape);
    }
    for (t1, t2) in [(d.max(0.0), (-d).max(0.0)), ((-d).max(0.0), d.max(0.0))] {
        let Ok(r) = k::thicken(f, t1, t2) else { continue };
        if !r.shape.is_valid() {
            continue;
        }
        if r.shape.classify_point(probe, d.abs() * 1e-6)? == k::PointClass::Inside {
            return Ok(r.shape);
        }
    }
    err("Could not offset selected faces as requested.")
}

/// Move face (offset) by booleans, where OCCT's offset fails: each face's slab
/// is added (outward) or cut (inward), the faces on the far side of the slabs
/// are the moved faces. A part that falls apart keeps its largest piece
/// (Onshape keeps one part: a hole grown to touch both sides leaves one half).
fn offset_by_booleans(ev: &mut EvalCtx, bi: usize, faces: &[usize], dist: f64) -> FResult<()> {
    use super::sheetmetal_util::{bool_named, body_named, tool_named};
    let body = ev.ctx.bodies[bi].clone();
    let feature = ev.feature;
    let mut tools = Vec::new();
    for &fi in faces {
        let f = body.shape.sub_shape(ShapeType::Face, fi)?;
        let parent = body.faces[fi].clone();
        tools.push(tool_named(face_slab(&f, dist)?, |_| Arc::new(TopoName::face(feature, Role::Offset(parent.clone())))));
    }
    let op = if dist > 0.0 { k::BooleanOp::Fuse } else { k::BooleanOp::Cut };
    let (shape, names) = bool_named(ev, op, &body_named(&body), &tools)?;
    let un = k::unify_same_domain(&shape)?;
    let names = ev.name_result(&[&crate::regen::eval::names_for(&shape, names)], &un.history, &un.shape, &BTreeMap::new(), None);
    let volume = |s: &Shape| k::volume_properties(s).map(|m| m.mass).unwrap_or(0.0);
    // The face moved through the far side: the cut leaves an empty solid. (One piece: its volume isn't needed, a big
    // part's took 0.8 s.)
    let mut pieces: Vec<_> = crate::regen::eval::split_solids(&un.shape, &names, Some(ev.feature)).into_iter().filter(|p| !p.shape.faces().is_empty()).collect();
    if pieces.len() > 1 {
        pieces.retain(|p| volume(&p.shape) > 0.0);
    }
    if pieces.is_empty() {
        return err("Could not offset selected faces as requested.");
    }
    if pieces.len() == 1 {
        let piece = pieces.swap_remove(0);
        ev.replace_body(bi, piece.shape, piece.names.faces);
        return Ok(());
    }
    let key = |p: &crate::regen::eval::NamedShape| -> (f64, [f64; 3]) {
        let v = volume(&p.shape);
        let c = p.shape.bounding_box(false).map(|b| b.center()).unwrap_or_default();
        (v, [c.x, c.y, c.z])
    };
    // Largest; equal volumes: the one nearest the origin corner (lowest x, y, z).
    let best = (0..pieces.len())
        .max_by(|&a, &b| {
            let (ka, kb) = (key(&pieces[a]), key(&pieces[b]));
            let tol = 1e-9 * ka.0.abs().max(kb.0.abs());
            if (ka.0 - kb.0).abs() > tol { ka.0.total_cmp(&kb.0) } else { kb.1.partial_cmp(&ka.1).unwrap_or(std::cmp::Ordering::Equal) }
        })
        .unwrap_or(0);
    let piece = pieces.swap_remove(best);
    ev.replace_body(bi, piece.shape, piece.names.faces);
    Ok(())
}

/// Move face "Create": new surfaces from moved copies of faces / regions.
fn move_face_create(ev: &mut EvalCtx, ty: &str, dir: Option<DVec3>, rotation: Option<(DVec3, DVec3, f64)>, sign: f64) -> FResult<()> {
    let refs = ev.query("surfacesAndFaces").to_vec();
    let mut sources: Vec<(Shape, Name)> = Vec::new();
    for r in &refs {
        match r {
            Reference::Face(n) => {
                for (bi, fi) in ev.resolve_faces(std::slice::from_ref(r)) {
                    sources.push((ev.face_shape(bi, fi)?, Arc::new(n.clone())));
                }
            }
            Reference::Body(_) => {
                for bi in ev.resolve_bodies(std::slice::from_ref(r)) {
                    let b = &ev.ctx.bodies[bi];
                    for (fi, f) in b.shape.faces().into_iter().enumerate() {
                        sources.push((f, b.faces[fi].clone()));
                    }
                }
            }
            _ => {
                for p in ev.profiles(std::slice::from_ref(r))? {
                    let key = Arc::new(TopoName::face(ev.feature, Role::Profile(p.key.clone())));
                    sources.push((p.shape, key));
                }
            }
        }
    }
    if sources.is_empty() {
        return crate::regen::eval::err_at(&["surfacesAndFaces"], "Select faces, surfaces, or sketch regions to offset from.");
    }
    let mut faces = Vec::new();
    let mut names = Vec::new();
    for (f, n) in &sources {
        let out = match (ty, dir, rotation) {
            ("ROTATE", _, Some((o, a, ang))) => f.transformed(&k::Transform::Rotate { origin: o, axis: a, angle: ang })?,
            ("TRANSLATE", Some(d), _) => f.translated(d * ev.length("translationDistance")? * sign)?,
            _ => {
                let d = ev.length("offsetDistance")? * sign;
                if d.abs() < 1e-12 { f.clone() } else { k::offset_shape(f, d, k::JoinType::Arc).map_err(kerr("Could not offset selected faces as requested."))?.shape }
            }
        };
        for g in out.faces() {
            faces.push(g);
            names.push(Arc::new(TopoName::face(ev.feature, Role::Offset(n.clone()))));
        }
    }
    let names = dedup_names(ev, names);
    let shape = if faces.len() == 1 { faces[0].clone() } else { Shape::compound(&faces) };
    ev.apply_operation(vec![crate::regen::eval::NamedShape::new(shape, names)], "NEW", None, BodyKind::Surface)
}

// ------------------------------------------------------------------ replace face

/// Replace faces with the surface of a template face (or plane): the region
/// between each face and the template is added (template outside the part)
/// or removed (inside), so the neighbours run up to the template.
/// Onshape's DIRECT_EDIT_REPLACE_FACE_FAILED (opReplaceFace).
const REPLACE_FAILED: &str = "Could not replace faces as requested.";

pub fn replace_face(ev: &mut EvalCtx) -> FResult<()> {
    let groups = by_body(ev.resolve_faces(ev.query("replaceFaces")));
    if groups.is_empty() {
        return crate::regen::eval::err_at(&["replaceFaces"], "Select faces to replace.");
    }
    // Live L9: an empty or unresolvable Faces to replace with.
    let unresolved = || crate::regen::eval::err_at(&["templateFace"], "Cannot resolve entities.");
    let Some(tr) = ev.query("templateFace").first().cloned() else { return unresolved() };
    let extent = ev.model_extent();
    let mut template = match &tr {
        Reference::Face(_) => {
            // Several faces make one template (opReplaceFace): they must
            // join into one sheet.
            let picked = ev.query("templateFace").to_vec();
            let found = ev.resolve_faces(&picked);
            if found.is_empty() {
                return unresolved();
            }
            let faces: Vec<Shape> = found.iter().map(|&(tb, tf)| ev.face_shape(tb, tf)).collect::<FResult<_>>()?;
            if faces.len() == 1 {
                faces.into_iter().next().unwrap_or_else(Shape::null)
            } else {
                let sewn = k::sew(&faces, 1e-6, false).map_err(|_| fe(REPLACE_FAILED))?.shape;
                if sewn.count(ShapeType::Shell) != 1 || sewn.count(ShapeType::Face) != faces.len() {
                    return err(REPLACE_FAILED);
                }
                sewn
            }
        }
        _ => {
            let Some(p) = ev.resolve_plane(&tr) else { return unresolved() };
            k::make_plane_face(&crate::regen::eval::kernel_plane(&p), -extent, extent, -extent, extent)?
        }
    };
    // Flip alignment reverses the template's sense (live L7/L8): the
    // replaced faces must then face the same way as the template, else it
    // fails; the offset goes along the template's (flipped) normal.
    let sense = if ev.params.get("oppositeSense").is_some() && ev.bool("oppositeSense") { -1.0 } else { 1.0 };
    let template_normal = |p: DVec3| -> Option<DVec3> { nearest_on(&template, p).map(|(_, n)| n) };
    for (bi, faces) in &groups {
        for &f in faces {
            let face = ev.ctx.bodies[*bi].shape.sub_shape(ShapeType::Face, f)?;
            // (A template built over the face's own parameters: its normal where the face's points map, no projection
            // onto it.)
            if let Some(along) = k::mapped_template_sense(&face, &template) {
                if along != (sense > 0.0) {
                    return err(REPLACE_FAILED);
                }
                continue;
            }
            // (A point on the face: a whole cylinder's centroid is on its axis.)
            let c = point_on_face(&face).ok_or_else(|| fe(REPLACE_FAILED))?;
            if let (Ok(n), Some(t)) = (face.normal_at_point(c), template_normal(c))
                && n.dot(t * sense) < 0.0
            {
                return err(REPLACE_FAILED);
            }
        }
    }
    let off = ev.length("offset")? * if ev.bool("oppositeDirection") { -1.0 } else { 1.0 } * sense;
    if off.abs() > 1e-12 {
        template = match template.face_plane()? {
            Some(pl) => template.translated(pl.unit_normal() * off)?,
            None => k::offset_shape(&template, off, k::JoinType::Arc).map_err(|_| fe(REPLACE_FAILED))?.shape,
        };
    }
    // (Within a deadline: a dense template's Booleans can run for minutes. Live's own error then.)
    let (r, late) = k::with_deadline(REPLACE_DEADLINE_S, || -> FResult<()> {
        for (bi, faces) in groups.into_iter().rev() {
            let names: Vec<Name> = faces.iter().map(|&f| ev.ctx.bodies[bi].faces[f].clone()).collect();
            for n in names {
                replace_with(ev, bi, &n, &template, sense)?;
            }
        }
        Ok(())
    });
    r.map_err(|e| {
        log::debug!("replace face: {}{}", e.message, if late { " (past its deadline)" } else { "" });
        fe(REPLACE_FAILED)
    })
}

/// How long a Replace face may take (the user's 512 x 512 displacement map: live 11 s).
const REPLACE_DEADLINE_S: f64 = 90.0;

/// Replace face by the face's own slab: the face thickened past the template's offsets both ways, split by the
/// template (carried past the face's edges), the part less the slab, and the slab's pieces on its side of the template
/// put back. So a curved face (live: a cylinder's side by a bumpy periodic B-spline) and a template crossing the face
/// (a displacement map's black below, white above) are replaced; the neighbours run on along the face's normals.
fn replace_by_slabs(ev: &mut EvalCtx, bi: usize, fi_name: &Name, face: &Shape, template: &Shape, sense: f64, (lo, hi): (f64, f64)) -> FResult<()> {
    let feature = ev.feature;
    let body = ev.ctx.bodies[bi].clone();
    let size = face.bounding_box(false)?.diagonal().max(1e-9);
    let margin = 0.25 * (hi - lo).abs() + 1e-3 * size;
    let (out, inward) = (hi.max(0.0) + margin, (-lo).max(0.0) + margin);
    let ext = if template.shape_type() == ShapeType::Face { k::extend_face(template, 0.25 * size + margin)? } else { template.clone() };
    // The slab: the face thickened both ways (checked: the kernel's sense is the face's orientation).
    let c = super::helpers_a::point_on_face(face).ok_or_else(|| fe(REPLACE_FAILED))?;
    let n = face.normal_at_point(c)?;
    let mut slab = k::thicken(face, out, inward)?.shape;
    if slab.classify_point(c + n * (0.5 * out), 1e-9)? != k::PointClass::Inside {
        slab = k::thicken(face, inward, out)?.shape;
    }
    // Its pieces on the part's side of the template: their face on the template faces along its outward normal.
    // (A dense template in patches: OCCT's edge-face checks project onto a whole face, 7 s each on the user's 512 x 512
    // displacement map; the patches' pieces on one surface are joined by the unify below.)
    let tools = if ext.shape_type() == ShapeType::Face { k::face_patches(&ext, 64)? } else { ext.clone() };
    let pieces = k::split(&[slab.clone()], std::slice::from_ref(&tools), &k::BooleanOptions::default())?.shape.solids();
    let kept: Vec<Shape> = pieces
        .into_iter()
        .filter(|piece| {
            !piece.faces().iter().any(|g| {
                let Some(p) = super::helpers_a::point_on_face(g) else { return false };
                let Some((q, t)) = nearest_on(&ext, p) else { return false };
                q.distance(p) < 1e-7 * size && g.normal_at_point(p).is_ok_and(|m| m.dot(t * sense) < 0.0)
            })
        })
        .collect();
    let tool_names = |s: &Shape, from: u16| -> crate::regen::eval::NamedShape {
        let names = (0..s.count(ShapeType::Face) as u16).map(|i| Arc::new(TopoName::face(feature, Role::Generated(Arc::new(TopoName::Split { parent: fi_name.clone(), by: feature, ordinal: from + i }))))).collect();
        crate::regen::eval::NamedShape::new(s.clone(), names)
    };
    // The part less the slab (no template in it), then the kept pieces on it: they share the slab's inner face only,
    // so glued (a Boolean against the dense template again took 15 GB on the user's 512 x 512 displacement map).
    let slab_names = tool_names(&slab, 0);
    let inner = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&body.shape), std::slice::from_ref(&slab), &k::BooleanOptions::default())?;
    let names = ev.name_result(&[&body.shape_names(), &slab_names.names], &inner.history, &inner.shape, &BTreeMap::new(), None);
    let (shape, names) = if kept.is_empty() {
        (inner.shape, names)
    } else {
        let adds = Shape::compound(&kept);
        let glued = k::BooleanOptions { glue: k::Glue::ShiftChecked, ..Default::default() };
        let fused = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(&inner.shape), std::slice::from_ref(&adds), &glued)?;
        let inner_names = crate::regen::eval::names_for(&inner.shape, names);
        let add_names = tool_names(&adds, slab.count(ShapeType::Face) as u16);
        let names = ev.name_result(&[&inner_names, &add_names.names], &fused.history, &fused.shape, &BTreeMap::new(), None);
        (fused.shape, names)
    };
    let un = k::unify_same_domain(&shape)?;
    let rn = crate::regen::eval::names_for(&shape, names);
    let names = ev.name_result(&[&rn], &un.history, &un.shape, &BTreeMap::new(), None);
    if un.shape.count(ShapeType::Solid) != 1 {
        return err(REPLACE_FAILED);
    }
    // (The slab's seam can stay inside a cap as a slit edge: a face the display mesher won't take. Faces in place.)
    let result = k::drop_open_wires(&un.shape)?;
    // The template's pieces (a piece on each patch) merged into one face, as live's: faces left alone keep their
    // names, a merged one takes its first piece's.
    let (result, names) = match ext.shape_type() {
        ShapeType::Face => {
            let merged = k::merge_faces_on(&result, &ext)?;
            if merged.is_same(&result) {
                (result, names)
            } else {
                let old = result.faces();
                let gone: Vec<usize> = (0..old.len()).filter(|&i| merged.index_of(&old[i], ShapeType::Face).is_none()).collect();
                let mut fresh = gone.iter();
                let renamed: Vec<Name> = merged
                    .faces()
                    .iter()
                    .map(|f| match result.index_of(f, ShapeType::Face) {
                        Some(i) => names[i].clone(),
                        None => fresh.next().map_or_else(|| Arc::new(TopoName::face(feature, Role::New(0))), |&i| names[i].clone()),
                    })
                    .collect();
                (merged, renamed)
            }
        }
        _ => (result, names),
    };
    ev.replace_body(bi, result, names);
    Ok(())
}

/// Replace one face (by name) of body `bi` with the template's surface.
/// The template point nearest `p` and the template's normal there (a face,
/// or a sheet of several).
fn nearest_on(template: &Shape, p: DVec3) -> Option<(DVec3, DVec3)> {
    let faces = if template.shape_type() == ShapeType::Face { vec![template.clone()] } else { template.faces() };
    let (f, q) = faces.iter().filter_map(|f| f.project_point(p).ok().map(|q| (f, q))).min_by(|a, b| a.1.distance.total_cmp(&b.1.distance))?;
    Some((q.point, f.normal_at_point(q.point).ok()?))
}

/// Planar faces of `shape` replaced by the template one at a time: the prism from each up to the template added or
/// cut. The history records only where each face went (its pieces on the template); the other faces are found again
/// by place.
fn replace_by_prism(shape: &Shape, faces: &[Shape], template: &Shape) -> k::Result<k::OpResult> {
    let failed = || k::KernelError::OperationFailed(REPLACE_FAILED.into());
    let mut cur = shape.clone();
    // Each face's images so far (the walls the prism shortens: their old points may lie in what it filled).
    let mut images: Vec<Vec<u32>> = (0..shape.count(ShapeType::Face) as u32).map(|i| vec![i]).collect();
    for face in faces {
        let c = face.centroid()?;
        let n = face.normal_at_point(c)?;
        let (q, _) = nearest_on(template, c).ok_or_else(failed)?;
        let s = (q - c).dot(n);
        if s.abs() < 1e-9 {
            continue;
        }
        // Onto a plane: the face moved there, its neighbours cut again where they meet it (a drafted pocket's walls; a
        // prism square to the face left wedges between them: c2-replace-face-exercise).
        let now = shape.index_of(face, ShapeType::Face).and_then(|fi| match images[fi][..] { [i] => cur.sub_shape(ShapeType::Face, i as usize).ok(), _ => None });
        let tweaked = now.clone().zip(onto_plane(face, c, n, template)).and_then(|(f, m)| k::tweak_faces(&cur, &[f], &k::Transform::Matrix(m)).ok()).filter(|r| r.shape.count(ShapeType::Solid) == 1 && r.shape.is_valid());
        if let Some(r) = tweaked {
            faces_through(&mut images, &r.history, &cur, &r.shape);
            cur = r.shape;
            continue;
        }
        let onto = now.as_ref().and_then(|f| onto_cylinder(&cur, f, c, s, template));
        if let Some([r, u]) = onto {
            faces_through(&mut images, &r.history, &cur, &r.shape);
            faces_through(&mut images, &u.history, &r.shape, &u.shape);
            cur = u.shape;
            continue;
        }
        let prism = k::extrude_up_to(face, if s > 0.0 { n } else { -n }, template, 0.0)?;
        let op = if s > 0.0 { k::BooleanOp::Fuse } else { k::BooleanOp::Cut };
        let res = k::boolean(op, std::slice::from_ref(&cur), std::slice::from_ref(&prism.shape), &k::BooleanOptions::default())?;
        let uni = k::unify_same_domain(&res.shape)?;
        faces_through(&mut images, &res.history, &cur, &res.shape);
        faces_through(&mut images, &uni.history, &res.shape, &uni.shape);
        cur = uni.shape;
    }
    if cur.count(ShapeType::Solid) != 1 {
        return Err(failed());
    }
    let size = cur.bounding_box(false)?.diagonal().max(1e-9);
    let on_template: Vec<u32> = cur
        .faces()
        .iter()
        .enumerate()
        .filter(|(_, g)| point_on_face(g).and_then(|p| nearest_on(template, p).map(|(q, _)| q.distance(p) < 1e-7 * size)).unwrap_or(false))
        .map(|(i, _)| i as u32)
        .collect();
    let replaced: Vec<usize> = faces.iter().filter_map(|f| shape.index_of(f, ShapeType::Face)).collect();
    let record = |fi: usize, o: u32| k::HistoryRecord { arg: 0, input_kind: ShapeType::Face, input_index: fi as u32, relation: k::Relation::Modified, output_kind: ShapeType::Face, output_index: o };
    let records = images
        .iter()
        .enumerate()
        .flat_map(|(fi, imgs)| if replaced.contains(&fi) { on_template.iter().map(|&o| record(fi, o)).collect::<Vec<_>>() } else { imgs.iter().map(|&o| record(fi, o)).collect() })
        .collect();
    Ok(k::OpResult { shape: cur, history: k::History { records } })
}

/// A face of `cur` moved `s` along its normal onto a cylinder template, its walls cut again where they meet it: of the
/// pocket (boss) it ends, the part on the face's side of the cylinder filled (taken off). The Boolean and the unify
/// after it; `None` where the template is no one cylinder or a step fails.
fn onto_cylinder(cur: &Shape, face: &Shape, c: DVec3, s: f64, template: &Shape) -> Option<[k::OpResult; 2]> {
    let t = if template.shape_type() == ShapeType::Face { template.clone() } else { template.faces().into_iter().next().filter(|_| template.count(ShapeType::Face) == 1)? };
    let k::Surface::Cylinder { origin, axis, radius, .. } = t.face_surface().ok()? else { return None };
    let fi = cur.index_of(face, ShapeType::Face)? as u32;
    let ef = cur.edge_faces().ok()?;
    let mut walls: BTreeSet<u32> = ef.iter().filter(|fs| fs.contains(&fi)).flatten().copied().collect();
    walls.insert(fi);
    // The pocket (boss) as a solid of its own: the face, its walls and a cap across their free edges (Delete face's
    // filled part came out as the part's voids: c2-replace-face-exercise's tray, recessed under its floor).
    let free: Vec<Shape> = ef.iter().enumerate().filter(|(_, fs)| fs.iter().filter(|f| walls.contains(f)).count() == 1).map(|(ei, _)| cur.sub_shape(ShapeType::Edge, ei).ok()).collect::<Option<_>>()?;
    let cap = k::make_face_from_wire(&k::make_wire(&free).ok()?, true).ok()?;
    let mut pieces: Vec<Shape> = walls.iter().map(|&i| cur.sub_shape(ShapeType::Face, i as usize).ok()).collect::<Option<_>>()?;
    pieces.push(cap);
    let mut region = k::sew(&pieces, 1e-6, true).ok()?.shape;
    if region.shape_type() != ShapeType::Solid || !region.is_valid() {
        return None;
    }
    if region.volume().ok()? < 0.0 {
        region = region.reversed().ok()?;
    }
    let bb = cur.bounding_box(false).ok()?;
    let size = bb.diagonal().max(1e-9);
    let axis = axis.normalize();
    let mid = origin + axis * ((bb.min + bb.max) * 0.5 - origin).dot(axis);
    let cyl = k::make_cylinder(&k::Plane::from_normal(mid - axis * 4.0 * size, axis), radius, 8.0 * size, std::f64::consts::TAU).ok()?;
    let opts = k::BooleanOptions::default();
    let off = c - origin;
    let side = if (off - axis * off.dot(axis)).length() < radius {
        cyl
    } else {
        let world = k::make_box_corners(bb.min - DVec3::splat(size), bb.max + DVec3::splat(size)).ok()?;
        k::boolean(k::BooleanOp::Cut, &[world], &[cyl], &opts).ok()?.shape
    };
    let piece = k::boolean(k::BooleanOp::Common, &[region], &[side], &opts).ok()?.shape;
    if piece.count(ShapeType::Solid) == 0 || piece.volume().ok()? <= 0.0 {
        return None;
    }
    let op = if s > 0.0 { k::BooleanOp::Fuse } else { k::BooleanOp::Cut };
    let r = k::boolean(op, std::slice::from_ref(cur), &[piece], &opts).ok()?;
    let u = k::unify_same_domain(&r.shape).ok()?;
    (u.shape.count(ShapeType::Solid) == 1 && u.shape.is_valid()).then_some([r, u])
}

/// The rigid motion taking a planar face (centroid `c`, normal `n`) onto the template where the template is one plane
/// over it; `None` otherwise.
fn onto_plane(face: &Shape, c: DVec3, n: DVec3, template: &Shape) -> Option<DMat4> {
    let (q, nt) = nearest_on(template, c)?;
    let m = if nt.dot(n) < 0.0 { -nt } else { nt }.normalize();
    let size = face.bounding_box(false).ok()?.diagonal().max(1e-9);
    let [u0, u1, v0, v1] = face.uv_bounds().ok()?;
    for i in 0..=2 {
        for j in 0..=2 {
            let p = face.point_at_uv(u0 + (u1 - u0) * i as f64 / 2.0, v0 + (v1 - v0) * j as f64 / 2.0).ok()?;
            let (qq, nn) = nearest_on(template, p)?;
            if ((qq - q).dot(m)).abs() > 1e-7 * size || nn.cross(m).length() > 1e-9 {
                return None;
            }
        }
    }
    let r = glam::DQuat::from_rotation_arc(n.normalize(), m);
    Some(DMat4::from_translation(c + m * (q - c).dot(m)) * DMat4::from_quat(r) * DMat4::from_translation(-c))
}

/// Each face's images carried through one step `from` → `to`: its Modified / Kept records, else (no record) the same
/// face in `to`.
fn faces_through(images: &mut [Vec<u32>], h: &k::History, from: &Shape, to: &Shape) {
    for imgs in images.iter_mut() {
        *imgs = imgs
            .iter()
            .flat_map(|&c| {
                let m = h.modified(0, ShapeType::Face, c);
                if !m.is_empty() || h.is_deleted(0, ShapeType::Face, c) {
                    return m;
                }
                from.sub_shape(ShapeType::Face, c as usize).ok().and_then(|f| to.index_of(&f, ShapeType::Face)).map(|i| i as u32).into_iter().collect()
            })
            .collect();
    }
}

fn replace_with(ev: &mut EvalCtx, bi: usize, fi_name: &Name, template: &Shape, sense: f64) -> FResult<()> {
    let feature = ev.feature;
    let body = ev.ctx.bodies[bi].clone();
    let Some(&fi) = body.find_faces(fi_name).first() else { return Ok(()) };
    let face = body.shape.sub_shape(ShapeType::Face, fi)?;
    // A template built over the face's own parameters (a displacement map's): the face's edges carried onto it, no
    // intersection with it (the user's 512 x 512 map); its faces in place, so their names stay.
    let one = if template.shape_type() == ShapeType::Face { Some(template.clone()) } else { template.faces().into_iter().next().filter(|_| template.count(ShapeType::Face) == 1) };
    let mapped = one.filter(|t| matches!(t.face_surface(), Ok(k::Surface::BSpline { .. }))).map(|t| k::replace_face_mapped(&body.shape, &face, &t));
    if let Some(Ok(shape)) = mapped
        && shape.count(ShapeType::Solid) == 1
        && shape.count(ShapeType::Face) == body.shape.count(ShapeType::Face)
    {
        let names = body.faces.clone();
        ev.replace_body(bi, shape, names);
        return Ok(());
    }
    // (Stopped, by a cancel or the deadline: not tried again by slabs.)
    if k::thread_cancelled() {
        return err("cancelled");
    }
    // The template's offsets from the face along its normal, over the face.
    let [u0, u1, v0, v1] = face.uv_bounds()?;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..=8 {
        for j in 0..=8 {
            let p = face.point_at_uv(u0 + (u1 - u0) * i as f64 / 8.0, v0 + (v1 - v0) * j as f64 / 8.0)?;
            let (Ok(n), Some((q, _))) = (face.normal_at_point(p), nearest_on(template, p)) else { continue };
            let d = (q - p).dot(n);
            lo = lo.min(d);
            hi = hi.max(d);
        }
    }
    // A curved face, a template on both sides of it (a displacement map's black and white) or a dense one (its prism
    // up to it ran for minutes): by slabs.
    let dense = template.faces().iter().any(|f| matches!(f.face_surface(), Ok(k::Surface::BSpline { nb_u_poles, nb_v_poles, .. }) if nb_u_poles * nb_v_poles >= 4096));
    if !face.is_planar() || (lo < -1e-9 && hi > 1e-9) || dense {
        return replace_by_slabs(ev, bi, fi_name, &face, template, sense, (lo, hi));
    }
    let c = face.centroid()?;
    let n = face.normal_at_point(c)?;
    let Some((q, _)) = nearest_on(template, c) else { return err(REPLACE_FAILED) };
    let s = (q - c).dot(n);
    if s.abs() < 1e-9 {
        return Ok(());
    }
    // The fillets round the face rolled again against the template (live: c2-replace-face-exercise's pocket floor,
    // its r 12.7 mm fillets to the walls and corners rolled at the new floor).
    let blends = super::detail_a::blends_to_reapply(&body.shape, &body.faces, &[fi], false)?;
    if !blends.is_empty() {
        return super::detail_a::reblend(ev, bi, &[fi], &blends, REPLACE_FAILED, |shape, fs| replace_by_prism(shape, fs, template));
    }
    let dir = if s > 0.0 { n } else { -n };
    let prism = k::extrude_up_to(&face, dir, template, 0.0).map_err(kerr("Surface to replace with needs to extend size of faces."))?;
    let tool_names: Vec<Name> = (0..prism.shape.count(ShapeType::Face) as u32).map(|i| Arc::new(TopoName::face(feature, Role::Generated(Arc::new(TopoName::Split { parent: fi_name.clone(), by: feature, ordinal: i as u16 }))))).collect();
    let tool = crate::regen::eval::NamedShape::new(prism.shape, tool_names);
    let op = if s > 0.0 { k::BooleanOp::Fuse } else { k::BooleanOp::Cut };
    let res = k::boolean(op, std::slice::from_ref(&body.shape), std::slice::from_ref(&tool.shape), &k::BooleanOptions::default())?;
    let names = ev.name_result(&[&body.shape_names(), &tool.names], &res.history, &res.shape, &BTreeMap::new(), None);
    let un = k::unify_same_domain(&res.shape)?;
    let rn = crate::regen::eval::names_for(&res.shape, names);
    let names = ev.name_result(&[&rn], &un.history, &un.shape, &BTreeMap::new(), None);
    if un.shape.count(ShapeType::Solid) == 0 {
        return err("Could not replace faces as requested.");
    }
    ev.replace_body(bi, un.shape, names);
    Ok(())
}
