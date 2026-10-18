//! Extrude (solid and surface), with all end conditions, second direction,
//! starting offset and draft.

use crate::display::BodyKind;
use crate::features::common::is_up_to;
use crate::reference::Reference;
use crate::regen::eval::{EvalCtx, FResult, NamedShape, Profile, err, names_for, sweep_names};
use glam::DVec3;
use noshape_kernel as k;

/// End of one extrude direction: either a distance along the direction or an "up to" target.
enum End {
    Distance(f64),
    UpTo(k::Shape),
    UpToPoint(DVec3),
}

pub fn eval(ev: &mut EvalCtx) -> FResult<()> {
    let body_type = ev.enum_("bodyType").to_string();
    let thin = match body_type.as_str() {
        "SURFACE" => return surface(ev),
        "THIN" => true,
        _ => false,
    };
    let refs = ev.query(if thin { "wallShape" } else { "entities" });
    if refs.is_empty() {
        return err(if thin { "Select edges or faces to define the path of the thin wall" } else { "Select sketch regions or planar faces to extrude" });
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
    if ev.bool("hasExtrudeDirection")
        && !thin
        && let Some(r) = ev.query("extrudeDirection").first()
    {
        match ev.resolve_direction(r) {
            Some(x) => {
                if x.dot(p.frame.normal).abs() < 1e-9 {
                    return err("Extrude direction can't be parallel to the profile");
                }
                d = if x.dot(p.frame.normal) < 0.0 { -x } else { x };
            }
            None => return err("Select an extrude direction"),
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
fn end_for(ev: &EvalCtx, bound: &str, depth_id: &str, face_id: &str, body_id: &str, vertex_id: &str, has_off: &str, off_id: &str, off_flip: &str, dir: DVec3) -> FResult<End> {
    let off = if is_up_to(bound) && ev.bool(has_off) {
        let o = ev.length(off_id)?;
        if ev.bool(off_flip) { -o } else { o }
    } else {
        0.0
    };
    match bound {
        "BLIND" => Ok(End::Distance(ev.length(depth_id)?)),
        "THROUGH_ALL" => Ok(End::Distance(ev.model_extent())),
        "UP_TO_SURFACE" => {
            let Some(r) = ev.query(face_id).first() else { return err("Select a face to extrude up to") };
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
            let Some(r) = ev.query(body_id).first() else { return err("Select a part to extrude up to") };
            let bi = *ev.resolve_bodies(std::slice::from_ref(r)).first().ok_or_else(|| crate::regen::eval::FeatureError::new("Up to part no longer exists"))?;
            let s = ev.ctx.bodies[bi].shape.clone();
            Ok(End::UpTo(if off != 0.0 { s.translated(-dir * off)? } else { s }))
        }
        "UP_TO_NEXT" => {
            let solids: Vec<k::Shape> = ev.ctx.bodies.iter().filter(|b| b.kind == BodyKind::Solid).map(|b| b.shape.clone()).collect();
            if solids.is_empty() {
                return err("Nothing to extrude up to");
            }
            let c = k::Shape::compound(&solids);
            Ok(End::UpTo(if off != 0.0 { c.translated(-dir * off)? } else { c }))
        }
        "UP_TO_VERTEX" => {
            let Some(r) = ev.query(vertex_id).first() else { return err("Select a vertex to extrude up to") };
            let p = ev.resolve_vertex_point(r).ok_or_else(|| crate::regen::eval::FeatureError::new("Up to vertex no longer exists"))?;
            Ok(End::UpToPoint(p - dir * off))
        }
        other => err(format!("Unsupported end type {other}")),
    }
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
        if ev.bool("draftPullDirection") { -a } else { a }
    } else {
        0.0
    };
    // Starting offset moves the profile before extruding.
    let mut start_shift = DVec3::ZERO;
    if ev.bool("startOffset") {
        if ev.is("startOffsetBound", "BLIND") {
            let d = ev.length("startOffsetDistance")?;
            start_shift = dir * if ev.bool("startOffsetOppositeDirection") { -d } else { d };
        } else if let Some(r) = ev.query("startOffsetEntity").first() {
            let target = ev.resolve_plane(r).map(|f| f.origin).or_else(|| ev.resolve_vertex_point(r));
            if let Some(t) = target {
                start_shift = dir * (t - p.frame.origin).dot(dir);
            }
        }
    }
    let profile = if start_shift.length() > 0.0 { p.shape.translated(start_shift)? } else { p.shape.clone() };
    let named = |res: k::OpResult| NamedShape::new(res.shape.clone(), sweep_names(ev, p, &res, true, true));

    if symmetric {
        let End::Distance(depth) = end else { unreachable!("symmetric is only offered for blind / through all") };
        return Ok(named(k::extrude_between(&profile, -dir * depth * 0.5, dir * depth * 0.5, draft)?));
    }
    if !ev.bool("hasSecondDirection") {
        return Ok(named(one_side(&profile, dir, end, draft)?));
    }
    let bound2 = ev.enum_("secondDirectionBound").to_string();
    let sign = if ev.bool("secondDirectionOppositeDirection") { -1.0 } else { 1.0 };
    let d2 = dir * sign;
    let end2 = end_for(ev, &bound2, "secondDirectionDepth", "secondDirectionBoundEntityFace", "secondDirectionBoundEntityBody", "secondDirectionBoundEntityVertex", "hasSecondDirectionOffset", "secondDirectionOffsetDistance", "secondDirectionOffsetOppositeDirection", d2)?;
    let draft2 = if ev.bool("hasSecondDirectionDraft") {
        let a = ev.angle("secondDirectionDraftAngle")?;
        if ev.bool("secondDirectionDraftPullDirection") { -a } else { a }
    } else {
        draft
    };
    if let (End::Distance(x1), End::Distance(x2)) = (&end, &end2)
        && draft2 == draft
    {
        let (a, b) = (d2 * *x2, dir * *x1);
        return Ok(named(if a.length() < 1e-12 && draft == 0.0 { k::extrude(&profile, b)? } else { k::extrude_between(&profile, a, b, draft)? }));
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
        return err("Select sketch curves to extrude");
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
