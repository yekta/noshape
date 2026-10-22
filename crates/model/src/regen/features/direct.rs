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
        if ev.bool("includeFillet") {
            set.extend(adjacent_fillets(&body, &set));
        }
        let all = body.shape.faces();
        if set.len() >= all.len() {
            return err("Cannot delete all faces of a part or surface.");
        }
        match heal.as_str() {
            "HEAL" => {
                let del: Vec<Shape> = set.iter().map(|f| all[*f].clone()).collect();
                let res = k::delete_faces(&body.shape, &del).map_err(failed)?;
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
        && matches!(r, Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::Face(_))
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
pub(crate) fn move_rigid(ev: &mut EvalCtx, bi: usize, faces: &[usize], m: &DMat4) -> FResult<()> {
    let body = ev.ctx.bodies[bi].clone();
    let shapes: Vec<Shape> = faces.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
    let healed = k::delete_faces(&body.shape, &shapes).map_err(kerr("Move face failed (the faces could not be removed from the part)"))?;
    let h_names = ev.name_result(&[&body.shape_names()], &healed.history, &healed.shape, &BTreeMap::new(), None);
    let healed_n = crate::regen::eval::names_for(&healed.shape, h_names);
    let (v0, v1) = (body.shape.volume()?, healed.shape.volume()?);
    let pocket = v1 > v0;
    // The region the faces bound (material removed by a pocket, or a boss).
    let region = if pocket { k::cut(&healed.shape, &body.shape)? } else { k::cut(&body.shape, &healed.shape)? };
    if region.shape.count(ShapeType::Solid) == 0 {
        return err("The selected faces don't bound a region that can be moved");
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
        let Some(r) = ev.query("axis").first() else { return err("Select a rotation axis") };
        let (o, a) = ev.resolve_axis(r).ok_or_else(|| fe("The axis could not be resolved"))?;
        Some((o, a.normalize(), ev.angle("angle")? * sign))
    } else {
        None
    };
    if create {
        return move_face_create(ev, &ty, translate_dir, rotation, sign);
    }
    // Onshape's texts: the kernel's reason isn't shown.
    let failed_msg = if ty == "OFFSET" { "Could not offset selected faces as requested." } else { "Could not transform selected faces as requested." };
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
                    replace_with(ev, bi, &name, &template).map_err(|_| fe(failed_msg))?;
                }
            } else {
                move_rigid(ev, bi, &faces, &m).map_err(|_| fe(failed_msg))?;
            }
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
                k::offset_faces_each(&body.shape, &shapes, &each).map_err(|_| fe(failed_msg))?
            }
        };
        let offset_role = |n: &Name| Role::Offset(n.clone());
        let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), Some(&offset_role));
        ev.replace_body(bi, res.shape, names);
    }
    Ok(())
}

/// The region a face sweeps when offset by `d` along its outward normal (d < 0:
/// into the part). A cylinder face offset past its axis (a hole closing)
/// sweeps the whole cylinder.
fn face_slab(f: &Shape, d: f64) -> FResult<Shape> {
    if let k::Surface::Cylinder { origin, axis, radius, .. } = f.face_surface()? {
        // Offset toward the axis by at least the radius (outward normal pointing in).
        // A point on the face (a full cylinder's centroid is on its axis).
        let c = f.project_point(f.centroid()? + axis.any_orthonormal_vector() * radius)?.point;
        let n = f.normal_at_point(c)?;
        let toward_axis = n.dot((origin + axis * (c - origin).dot(axis)) - c) > 0.0;
        if toward_axis && d >= radius * (1.0 - 1e-9) && f.edges().iter().any(|e| e.is_closed().unwrap_or(false)) {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for v in f.vertices() {
                let t = (v.vertex_point()? - origin).dot(axis);
                lo = lo.min(t);
                hi = hi.max(t);
            }
            return Ok(k::make_cylinder(&k::Plane::from_normal(origin + axis * lo, axis), radius, hi - lo, std::f64::consts::TAU)?);
        }
    }
    // Thicken, then make sure the slab is on the requested side.
    let p = f.centroid()?;
    let probe = f.project_point(p)?.point + f.normal_at_point(p)? * (d * 0.5);
    for (t1, t2) in [(d.max(0.0), (-d).max(0.0)), ((-d).max(0.0), d.max(0.0))] {
        let Ok(r) = k::thicken(f, t1, t2) else { continue };
        if !r.shape.is_valid() {
            continue;
        }
        if r.shape.classify_point(probe, d.abs() * 1e-6)? == k::PointClass::Inside {
            return Ok(r.shape);
        }
    }
    err("The face could not be offset")
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
    let mut pieces = crate::regen::eval::split_solids(&un.shape, &names);
    if pieces.is_empty() {
        return err("Nothing is left of the part");
    }
    let key = |p: &crate::regen::eval::NamedShape| -> (f64, [f64; 3]) {
        let v = k::volume_properties(&p.shape).map(|m| m.mass).unwrap_or(0.0);
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
        return err("Select faces, surfaces or sketch regions");
    }
    let mut faces = Vec::new();
    let mut names = Vec::new();
    for (f, n) in &sources {
        let out = match (ty, dir, rotation) {
            ("ROTATE", _, Some((o, a, ang))) => f.transformed(&k::Transform::Rotate { origin: o, axis: a, angle: ang })?,
            ("TRANSLATE", Some(d), _) => f.translated(d * ev.length("translationDistance")? * sign)?,
            _ => {
                let d = ev.length("offsetDistance")? * sign;
                if d.abs() < 1e-12 { f.clone() } else { k::offset_shape(f, d, k::JoinType::Arc).map_err(kerr("The face could not be offset"))?.shape }
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
    let Some(tr) = ev.query("templateFace").first().cloned() else { return err("Select the face or plane to replace with") };
    let extent = ev.model_extent();
    let mut template = match &tr {
        Reference::Face(_) => {
            let Some(&(tb, tf)) = ev.resolve_faces(std::slice::from_ref(&tr)).first() else { return err("The template face no longer exists") };
            ev.face_shape(tb, tf)?
        }
        _ => {
            let Some(p) = ev.resolve_plane(&tr) else { return err("Select a face or plane to replace with") };
            k::make_plane_face(&crate::regen::eval::kernel_plane(&p), -extent, extent, -extent, extent)?
        }
    };
    let off = ev.length("offset")? * if ev.bool("oppositeDirection") { -1.0 } else { 1.0 };
    if off.abs() > 1e-12 {
        template = match template.face_plane()? {
            Some(pl) => template.translated(pl.unit_normal() * off)?,
            None => k::offset_shape(&template, off, k::JoinType::Arc).map_err(|_| fe(REPLACE_FAILED))?.shape,
        };
    }
    for (bi, faces) in groups.into_iter().rev() {
        let names: Vec<Name> = faces.iter().map(|&f| ev.ctx.bodies[bi].faces[f].clone()).collect();
        for n in names {
            replace_with(ev, bi, &n, &template).map_err(|e| {
                log::debug!("replace face: {}", e.message);
                fe(REPLACE_FAILED)
            })?;
        }
    }
    Ok(())
}

/// Replace one face (by name) of body `bi` with the template's surface.
fn replace_with(ev: &mut EvalCtx, bi: usize, fi_name: &Name, template: &Shape) -> FResult<()> {
    let feature = ev.feature;
    let body = ev.ctx.bodies[bi].clone();
    let Some(&fi) = body.find_faces(fi_name).first() else { return Ok(()) };
    let face = body.shape.sub_shape(ShapeType::Face, fi)?;
    let c = face.centroid()?;
    let n = face.normal_at_point(c)?;
    let q = template.project_point(c)?;
    let s = (q.point - c).dot(n);
    if s.abs() < 1e-9 {
        return Ok(());
    }
    let dir = if s > 0.0 { n } else { -n };
    let prism = k::extrude_up_to(&face, dir, template, 0.0).map_err(kerr("The face could not be extended to the template (it must cover the whole face)"))?;
    let tool_names: Vec<Name> = (0..prism.shape.count(ShapeType::Face) as u32).map(|i| Arc::new(TopoName::face(feature, Role::Generated(Arc::new(TopoName::Split { parent: fi_name.clone(), by: feature, ordinal: i as u16 }))))).collect();
    let tool = crate::regen::eval::NamedShape::new(prism.shape, tool_names);
    let op = if s > 0.0 { k::BooleanOp::Fuse } else { k::BooleanOp::Cut };
    let res = k::boolean(op, std::slice::from_ref(&body.shape), std::slice::from_ref(&tool.shape), &k::BooleanOptions::default())?;
    let names = ev.name_result(&[&body.shape_names(), &tool.names], &res.history, &res.shape, &BTreeMap::new(), None);
    let un = k::unify_same_domain(&res.shape)?;
    let rn = crate::regen::eval::names_for(&res.shape, names);
    let names = ev.name_result(&[&rn], &un.history, &un.shape, &BTreeMap::new(), None);
    if un.shape.count(ShapeType::Solid) == 0 {
        return err("Replacing the face removed the whole part");
    }
    ev.replace_body(bi, un.shape, names);
    Ok(())
}
