//! Extrude (solid and surface), with all end conditions, second direction,
//! starting offset and draft.

use crate::display::BodyKind;
use crate::features::common::is_up_to;
use crate::reference::Reference;
use crate::regen::eval::{EvalCtx, FResult, NamedShape, Profile, err, err_at, names_for, sweep_names};
use glam::DVec3;
use noshape_kernel as k;

/// End of one extrude direction: either a distance along the direction or an "up to" target.
enum End {
    Distance(f64),
    UpTo(k::Shape),
    UpToPoint(DVec3),
}

pub fn eval(ev: &mut EvalCtx) -> FResult<()> {
    // Onshape's EXTRUDE_FAILED for a failing geometry operation (live
    // "Contrast: a failed EXTRUDE", 10-22): the kernel's reason isn't shown.
    eval_inner(ev).map_err(|e| {
        if e.message.starts_with(crate::regen::eval::KERNEL_FAILURE) {
            log::debug!("extrude: {}", e.message);
            crate::regen::eval::FeatureError { message: "Failed to extrude selections, check input.".into(), ..e }
        } else {
            e
        }
    })
}

fn eval_inner(ev: &mut EvalCtx) -> FResult<()> {
    let body_type = ev.enum_("bodyType").to_string();
    let thin = match body_type.as_str() {
        "SURFACE" => return surface(ev),
        "THIN" => true,
        _ => false,
    };
    let refs = ev.query(if thin { "wallShape" } else { "entities" });
    if refs.is_empty() {
        // std extrude.fs: OFFSET_WIRE_SELECT_WALL_PATH / EXTRUDE_NO_SELECTED_REGION.
        return crate::regen::eval::err_at(if thin { &["wallShape"] } else { &["entities"] }, if thin { "Select edges or faces to define the path of the thin wall." } else { "Select face or sketch region to extrude." });
    }
    let profiles = if thin { super::thin::thin_profiles(ev, refs)? } else { ev.profiles(refs)? };
    if profiles.is_empty() {
        return err("No valid regions to extrude");
    }
    let mut tools = Vec::new();
    for p in &profiles {
        let dir = direction(ev, p, thin)?;
        tools.push(extrude_profile(ev, p, dir)?);
    }
    let op = ev.enum_("operationType").to_string();
    let scope = scope(ev);
    ev.apply_operation(tools, &op, scope, BodyKind::Solid)
}

pub fn scope(ev: &EvalCtx) -> Option<Vec<usize>> {
    if ev.bool("defaultScope") {
        Some((0..ev.ctx.bodies.len()).filter(|i| ev.ctx.bodies[*i].kind == BodyKind::Solid).collect())
    } else {
        let s = ev.resolve_bodies(ev.query("booleanScope"));
        if s.is_empty() { None } else { Some(s) }
    }
}

fn direction(ev: &EvalCtx, p: &Profile, thin: bool) -> FResult<DVec3> {
    let mut d = p.frame.normal;
    // A custom direction is not available for Thin (Onshape).
    // std extrude.fs processExtrudeDirection.
    if ev.bool("hasExtrudeDirection") && !thin {
        let Some(r) = ev.query("extrudeDirection").first() else { return err_at(&["extrudeDirection"], "Select Extrude direction.") };
        match ev.resolve_direction(r) {
            Some(x) => {
                if x.dot(p.frame.normal).abs() < 1e-9 {
                    return err_at(&["extrudeDirection"], "Extrude direction cannot be parallel to selected profile.");
                }
                d = if x.dot(p.frame.normal) < 0.0 { -x } else { x };
            }
            None => return err_at(&["extrudeDirection"], "Extrude direction entity is invalid."),
        }
    }
    if ev.bool("oppositeDirection") {
        d = -d;
    }
    // Remove/intersect point into the material by default in Onshape only via the
    // user's flip; keep the raw direction here.
    Ok(d.normalize())
}

#[allow(clippy::too_many_arguments)]
fn end_for(ev: &EvalCtx, bound: &str, depth_id: &str, face_id: &'static str, body_id: &'static str, vertex_id: &'static str, has_off: &str, off_id: &str, off_flip: &str, dir: DVec3) -> FResult<End> {
    let off = if is_up_to(bound) && ev.bool(has_off) {
        let o = ev.length(off_id)?;
        if ev.bool(off_flip) { -o } else { o }
    } else {
        0.0
    };
    match bound {
        "BLIND" => Ok(End::Distance(ev.length(depth_id)?)),
        "THROUGH_ALL" => {
            require_parts(ev)?;
            Ok(End::Distance(ev.model_extent()))
        }
        // std extrudeCommon.fs: EXTRUDE_SELECT_TERMINATING_*.
        "UP_TO_SURFACE" => {
            let Some(r) = ev.query(face_id).first() else { return err_at(&[face_id], "Select a face to terminate the extrude.") };
            let target = match r {
                Reference::Face(_) => {
                    let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(r)).first().ok_or_else(|| crate::regen::eval::FeatureError::new("Up to face no longer exists"))?;
                    ev.face_shape(bi, fi)?
                }
                _ => {
                    let f = ev.resolve_plane(r).ok_or_else(|| crate::regen::eval::FeatureError::new("Select a face or plane"))?;
                    let ext = ev.model_extent() * 4.0;
                    k::make_plane_face(&crate::regen::eval::kernel_plane(&f), -ext, ext, -ext, ext)?
                }
            };
            let target = if off != 0.0 { target.translated(-dir * off)? } else { target };
            Ok(End::UpTo(target))
        }
        "UP_TO_BODY" => {
            let Some(r) = ev.query(body_id).first() else { return err_at(&[body_id], "Select a part to terminate the extrude.") };
            let bi = *ev.resolve_bodies(std::slice::from_ref(r)).first().ok_or_else(|| crate::regen::eval::FeatureError::new("Up to part no longer exists"))?;
            let s = ev.ctx.bodies[bi].shape.clone();
            Ok(End::UpTo(if off != 0.0 { s.translated(-dir * off)? } else { s }))
        }
        "UP_TO_NEXT" => {
            require_parts(ev)?;
            let c = next_parts(ev)?;
            Ok(End::UpTo(if off != 0.0 { c.translated(-dir * off)? } else { c }))
        }
        "UP_TO_VERTEX" => {
            let Some(r) = ev.query(vertex_id).first() else { return err_at(&[vertex_id], "Select a vertex or mate connector to terminate the extrude.") };
            let p = ev.resolve_vertex_point(r).ok_or_else(|| crate::regen::eval::FeatureError::new("Up to vertex no longer exists"))?;
            Ok(End::UpToPoint(p - dir * off))
        }
        other => err(format!("Unsupported end type {other}")),
    }
}

/// A point of the starting offset entity at its height above the profile
/// plane (normal `n`), checked as std's checkStartOffsetEntity: lines must
/// lie parallel to the profile, other edges and faces must be planar and
/// parallel to it; vertices, mate connectors take their point.
fn start_offset_point(ev: &EvalCtx, r: &Reference, n: DVec3) -> FResult<DVec3> {
    const NOT_PLANAR: &str = "Selected start offset bound entity is not planar.";
    const NOT_PARALLEL: &str = "Start offset bound entity must be parallel to extruded entities.";
    let parallel = |m: DVec3| m.normalize_or_zero().cross(n).length() < 1e-9;
    let edge = match r {
        Reference::Edge(_) => ev.resolve_edges(std::slice::from_ref(r)).first().map(|&(b, e)| ev.edge_shape(b, e)).transpose()?,
        Reference::SketchEntity { sketch, entity } => match ev.ctx.sketch(*sketch).and_then(|s| Some((s.frame, s.sketch.curve(noshape_sketch::EntityId(*entity))?))) {
            Some((frame, c)) => curve_to_segment(&c).map(|seg| k::make_edge_2d(&crate::regen::eval::kernel_plane(&frame), &seg)).transpose()?,
            None => None,
        },
        _ => None,
    };
    if let Some(e) = edge {
        let info = e.edge_info()?;
        return match info.curve {
            k::Curve::Line { direction, .. } if direction.normalize_or_zero().dot(n).abs() > 1e-9 => err_at(&["startOffsetEntity"], NOT_PARALLEL),
            k::Curve::Line { .. } => Ok(info.start),
            k::Curve::Circle { axis, .. } | k::Curve::Ellipse { axis, .. } if parallel(axis) => Ok(info.start),
            k::Curve::Circle { .. } | k::Curve::Ellipse { .. } => err_at(&["startOffsetEntity"], NOT_PARALLEL),
            _ => err_at(&["startOffsetEntity"], NOT_PLANAR),
        };
    }
    if matches!(r, Reference::Face(_) | Reference::Plane(_) | Reference::FeaturePlane(_)) {
        let Some(f) = ev.resolve_plane(r) else { return err_at(&["startOffsetEntity"], NOT_PLANAR) };
        return if parallel(f.normal) { Ok(f.origin) } else { err_at(&["startOffsetEntity"], NOT_PARALLEL) };
    }
    ev.resolve_vertex_point(r).ok_or_else(|| crate::regen::eval::FeatureError { faulty: vec!["startOffsetEntity"], ..crate::regen::eval::FeatureError::new("Select a valid entity.") })
}

/// Through all and Up to next need parts or surfaces to reach (live: the
/// dialog's "Parts or Surfaces must be present in the Part Studio.",
/// std's CANNOT_COMPUTE_BBOX).
pub fn require_parts(ev: &EvalCtx) -> FResult<()> {
    if ev.ctx.bodies.iter().any(|b| matches!(b.kind, BodyKind::Solid | BodyKind::Surface)) {
        Ok(())
    } else {
        err("Parts or Surfaces must be present in the Part Studio.")
    }
}

/// What Up to next extrudes up to: the parts.
pub fn next_parts(ev: &EvalCtx) -> FResult<k::Shape> {
    let solids: Vec<k::Shape> = ev.ctx.bodies.iter().filter(|b| b.kind == BodyKind::Solid).map(|b| b.shape.clone()).collect();
    if solids.is_empty() {
        return err("Parts or Surfaces must be present in the Part Studio.");
    }
    Ok(k::Shape::compound(&solids))
}

/// One extrusion of `profile` from its plane along `dir` to `end`.
fn one_side(profile: &k::Shape, dir: DVec3, end: End, draft: f64) -> FResult<k::OpResult> {
    Ok(match end {
        End::Distance(depth) if draft == 0.0 => k::extrude(profile, dir * depth)?,
        End::Distance(depth) => k::extrude_between(profile, DVec3::ZERO, dir * depth, draft)?,
        End::UpTo(target) => k::extrude_up_to(profile, dir, &target, 0.0)?,
        End::UpToPoint(pt) => k::extrude_up_to_point(profile, dir, pt)?,
    })
}

/// Extrude one profile with every end condition; returns the named tool.
///
/// Two-sided extrusions with two blind ends are one prism. When either end
/// is an "up to" end (face, part, vertex, next), each side is extruded
/// separately from the profile plane and the halves are fused (and their
/// coplanar side faces unified): side 1 keeps `CapEnd`, the second end is
/// `CapStart`, side faces stay `Side(<profile edge>)`.
fn extrude_profile(ev: &EvalCtx, p: &Profile, dir: DVec3) -> FResult<NamedShape> {
    let bound = ev.enum_("endBound").to_string();
    let end = end_for(ev, &bound, "depth", "endBoundEntityFace", "endBoundEntityBody", "endBoundEntityVertex", "hasOffset", "offsetDistance", "offsetOppositeDirection", dir)?;
    let symmetric = ev.bool("symmetric") && matches!(bound.as_str(), "BLIND" | "THROUGH_ALL");
    let draft = if ev.bool("hasDraft") {
        let a = ev.angle("draftAngle")?;
        // std extrude.fs: unticked drafts outwards (the part grows away
        // from the profile; std's opDraft pulls against the direction).
        if ev.bool("draftPullDirection") { a } else { -a }
    } else {
        0.0
    };
    // Starting offset moves the profile before extruding.
    let mut start_shift = DVec3::ZERO;
    if ev.bool("startOffset") {
        if ev.is("startOffsetBound", "BLIND") {
            let d = ev.length("startOffsetDistance")?;
            start_shift = dir * if ev.bool("startOffsetOppositeDirection") { -d } else { d };
        } else {
            let Some(r) = ev.query("startOffsetEntity").first() else { return err_at(&["startOffsetEntity"], "Select a valid entity.") };
            let t = start_offset_point(ev, r, p.frame.normal)?;
            // std processStartOffsetData: along the extrude direction to the
            // entity's height above the profile plane.
            start_shift = dir * ((t - p.frame.origin).dot(p.frame.normal) / dir.dot(p.frame.normal));
        }
    }
    let profile = if start_shift.length() > 0.0 { p.shape.translated(start_shift)? } else { p.shape.clone() };
    let named = |res: k::OpResult| NamedShape::new(res.shape.clone(), sweep_names(ev, p, &res, true, true));

    // std splits a drafted symmetric extrude at the profile plane and drafts
    // both halves away from it.
    let (end, d2, end2, draft2) = if symmetric {
        let End::Distance(depth) = end else { unreachable!("symmetric is only offered for blind / through all") };
        if draft == 0.0 {
            return Ok(named(k::extrude_between(&profile, -dir * depth * 0.5, dir * depth * 0.5, 0.0)?));
        }
        (End::Distance(depth * 0.5), -dir, End::Distance(depth * 0.5), draft)
    } else {
        if !ev.bool("hasSecondDirection") {
            return Ok(named(one_side(&profile, dir, end, draft)?));
        }
        let bound2 = ev.enum_("secondDirectionBound").to_string();
        let sign = if ev.bool("secondDirectionOppositeDirection") { -1.0 } else { 1.0 };
        let d2 = dir * sign;
        let end2 = end_for(ev, &bound2, "secondDirectionDepth", "secondDirectionBoundEntityFace", "secondDirectionBoundEntityBody", "secondDirectionBoundEntityVertex", "hasSecondDirectionOffset", "secondDirectionOffsetDistance", "secondDirectionOffsetOppositeDirection", d2)?;
        // std: the second side has only its own draft (unticked: away from
        // the profile plane, like the first).
        let draft2 = if ev.bool("hasSecondDirectionDraft") && d2.dot(dir) < 0.0 {
            let a = ev.angle("secondDirectionDraftAngle")?;
            if ev.bool("secondDirectionDraftPullDirection") { a } else { -a }
        } else {
            0.0
        };
        (end, d2, end2, draft2)
    };
    if let (End::Distance(x1), End::Distance(x2)) = (&end, &end2)
        && draft == 0.0
        && draft2 == 0.0
    {
        let (a, b) = (d2 * *x2, dir * *x1);
        return Ok(named(if a.length() < 1e-12 { k::extrude(&profile, b)? } else { k::extrude_between(&profile, a, b, 0.0)? }));
    }
    // Two separately bounded halves, fused.
    let r1 = one_side(&profile, dir, end, draft)?;
    let r2 = one_side(&profile, d2, end2, draft2)?;
    let n1 = names_for(&r1.shape, sweep_names(ev, p, &r1, false, true));
    let n2 = {
        let mut roles = std::collections::BTreeMap::new();
        for f in r2.history.last_faces() {
            roles.insert(f, crate::reference::Role::CapStart(Some(p.key.clone())));
        }
        for r in &r2.history.records {
            if r.arg == 0 && r.input_kind == k::ShapeType::Edge && r.relation == k::Relation::Generated && r.output_kind == k::ShapeType::Face
                && let Some(src) = p.edge_sources.get(r.input_index as usize)
            {
                roles.entry(r.output_index).or_insert_with(|| crate::reference::Role::Side(src.clone()));
            }
        }
        let empty = crate::naming::ShapeNames::default();
        names_for(&r2.shape, ev.name_result(&[&empty], &r2.history, &r2.shape, &roles, None))
    };
    let fused = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(&r1.shape), std::slice::from_ref(&r2.shape), &k::BooleanOptions::default())?;
    let fnames = ev.name_result(&[&n1, &n2], &fused.history, &fused.shape, &std::collections::BTreeMap::new(), None);
    let unified = k::unify_same_domain(&fused.shape)?;
    let un = names_for(&fused.shape, fnames);
    let names = ev.name_result(&[&un], &unified.history, &unified.shape, &std::collections::BTreeMap::new(), None);
    Ok(NamedShape::new(unified.shape, names))
}

fn surface(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("surfaceEntities");
    if refs.is_empty() {
        return err_at(&["surfaceEntities"], "Select sketch curves to extrude.");
    }
    let mut tools = Vec::new();
    for r in refs {
        let Reference::SketchEntity { sketch, entity } = r else { continue };
        let Some(s) = ev.ctx.sketch(*sketch).cloned() else { continue };
        let id = noshape_sketch::EntityId(*entity);
        let Some(curve) = s.sketch.curve(id) else { continue };
        let seg = curve_to_segment(&curve);
        let Some(seg) = seg else { continue };
        let edge = k::make_edge_2d(&crate::regen::eval::kernel_plane(&s.frame), &seg)?;
        let depth = ev.length("depth")?;
        let dir = if ev.bool("oppositeDirection") { -s.frame.normal } else { s.frame.normal };
        let res = k::extrude(&edge, dir * depth)?;
        let mut roles = std::collections::BTreeMap::new();
        for (i, _) in res.shape.faces().iter().enumerate() {
            roles.insert(i as u32, crate::reference::Role::Side(crate::reference::Source::Sketch { sketch: *sketch, entity: *entity, segment: i as u16 }));
        }
        let names = ev.name_result(&[&Default::default()], &res.history, &res.shape, &roles, None);
        tools.push(NamedShape::new(res.shape, names));
    }
    if tools.is_empty() {
        return err("No valid curves to extrude");
    }
    let shapes: Vec<k::Shape> = tools.iter().map(|t| t.shape.clone()).collect();
    let sewn = if shapes.len() > 1 { k::sew(&shapes, 1e-6, false).map(|r| r.shape).unwrap_or_else(|_| k::Shape::compound(&shapes)) } else { shapes[0].clone() };
    let names: Vec<_> = tools.iter().flat_map(|t| t.names.faces.clone()).collect();
    let named = if sewn.count(k::ShapeType::Face) == names.len() { NamedShape::new(sewn, names) } else { tools.remove(0) };
    ev.apply_operation(vec![named], "NEW", None, BodyKind::Surface)
}

pub fn curve_to_segment(c: &noshape_sketch::Curve2) -> Option<k::Segment2d> {
    use noshape_sketch::Curve2 as C;
    Some(match c {
        C::Line { a, b } => k::Segment2d::Line { a: *a, b: *b },
        C::Circle { center, radius } => k::Segment2d::Circle { center: *center, radius: *radius },
        C::Arc { center, radius, start_angle, end_angle } => {
            let mut e = *end_angle;
            if e <= *start_angle {
                e += std::f64::consts::TAU;
            }
            k::Segment2d::Arc { center: *center, radius: *radius, start_angle: *start_angle, end_angle: e }
        }
        _ => return None,
    })
}
