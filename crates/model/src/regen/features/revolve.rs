//! Revolve (solid and thin), full or partial, one or two directions,
//! symmetric, with blind and "up to" end types.
//!
//! Up-to end types (method):
//! * Up to vertex / mate connector: the angle from the profile to the point
//!   around the axis is computed directly (angle between the radial
//!   directions of the profile centroid and the point, in the revolve
//!   sense), then the profile is revolved by that angle (plus the angular
//!   offset).
//! * Up to face / plane / part / next: the profile is revolved by almost a
//!   full turn (an open "C"), the C is split by the bounding entity (a plane
//!   becomes a large planar face; a part or "next" uses the solids), and the
//!   piece that contains the start of the sweep (a point of the profile
//!   rotated by a tiny angle) is kept. Angular offsets rotate the bounding
//!   entity back towards the profile. The cut face is named `CapEnd`.
//! * A second end position ("hasStartBound") is built the same way around
//!   the reversed axis and fused with the first side; two blind sides are a
//!   single revolve.

use crate::display::BodyKind;
use crate::naming::ShapeNames;
use crate::reference::{Reference, Role};
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, Profile, err, kernel_plane, names_for, sweep_names};
use glam::DVec3;
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;
use std::f64::consts::TAU;

pub fn eval(ev: &mut EvalCtx) -> FResult<()> {
    let thin = match ev.enum_("bodyType") {
        "SOLID" => false,
        "THIN" => true,
        _ => return surface(ev),
    };
    let refs = ev.query(if thin { "wallShape" } else { "entities" });
    if refs.is_empty() {
        // std revolve.fs: REVOLVE_SELECT_FACES, else (Thin too) REVOLVE_SURF_NO_CURVE.
        return crate::regen::eval::err_at(if thin { &["surfaceEntities"] } else { &["entities"] }, if thin { "Select sketch curves to revolve." } else { "Select face or sketch region to revolve." });
    }
    let Some(axis_ref) = ev.query("axis").first() else { return crate::regen::eval::err_at(&["axis"], "Select the axis to revolve around.") };
    let Some((origin, axis)) = ev.resolve_axis(axis_ref) else { return err("The revolve axis could not be resolved") };
    // Live "psb (10-24 e)": an open wall's Thickness 1 follows the axis as
    // its edge is directed, not Opposite direction.
    let turning = |f: &crate::display::PlaneFrame, m: DVec3| {
        let r = m - origin - axis * (m - origin).dot(axis);
        let d = axis.cross(r).normalize_or_zero();
        if d == DVec3::ZERO { f.normal } else { d }
    };
    let axis = if ev.bool("oppositeDirection") { -axis } else { axis };
    let profiles = if thin { super::thin::thin_profiles_sided(ev, refs, super::thin::WallSide::Along(&turning), true)? } else { ev.profiles(refs)? };
    let full = ev.bool("fullRevolve");
    let mut tools = Vec::new();
    for p in &profiles {
        if full {
            let res = k::revolve(&p.shape, origin, axis, TAU)?;
            let names = sweep_names(ev, p, &res, false, false);
            tools.push(NamedShape::new(res.shape, names));
            continue;
        }
        tools.push(partial(ev, p, origin, axis)?);
    }
    let op = ev.enum_("operationType").to_string();
    let scope = super::extrude::scope(ev);
    ev.apply_operation(tools, &op, scope, BodyKind::Solid)
}

/// Surface revolve: edges and sketch curves revolved into surfaces (each
/// connected chain one surface body), with the same end types as solids.
fn surface(ev: &mut EvalCtx) -> FResult<()> {
    use super::helpers_a::{resolve_curves, wires_from};
    let refs = ev.query("surfaceEntities").to_vec();
    if refs.is_empty() {
        return err("Select edges or sketch curves to revolve");
    }
    let Some(axis_ref) = ev.query("axis").first() else { return crate::regen::eval::err_at(&["axis"], "Select the axis to revolve around.") };
    let Some((origin, axis)) = ev.resolve_axis(axis_ref) else { return err("The revolve axis could not be resolved") };
    let axis = if ev.bool("oppositeDirection") { -axis } else { axis };
    let edges = resolve_curves(ev, &refs)?;
    if edges.is_empty() {
        return err("Select edges or sketch curves to revolve");
    }
    let x = axis.any_orthonormal_vector();
    let frame = crate::display::PlaneFrame { origin, x, y: axis.cross(x), normal: axis };
    let mut tools = Vec::new();
    for (i, w) in wires_from(&edges)?.into_iter().enumerate() {
        let edge_sources: Vec<crate::reference::Source> = w
            .edges()
            .iter()
            .map(|e| {
                let mid = |s: &Shape| s.edge_info().ok().map(|inf| s.point_at((inf.first_param + inf.last_param) / 2.0).unwrap_or(inf.start));
                edges.iter().find(|x| x.shape.is_same(e) || matches!((mid(&x.shape), mid(e)), (Some(a), Some(b)) if a.distance(b) < 1e-9)).map(|x| x.source.clone()).unwrap_or(crate::reference::Source::Index(i as u32))
            })
            .collect();
        let key = edge_sources.first().cloned().unwrap_or(crate::reference::Source::Index(i as u32));
        let p = Profile { shape: w, frame, edge_sources, key, from_face: false };
        if ev.bool("fullRevolve") {
            let res = k::revolve(&p.shape, origin, axis, TAU)?;
            let names = sweep_names(ev, &p, &res, false, false);
            tools.push(NamedShape::new(res.shape, names));
        } else {
            tools.push(partial(ev, &p, origin, axis)?);
        }
    }
    ev.apply_operation(tools, "NEW", None, BodyKind::Surface)
}

/// End of one revolve side.
enum Side {
    Angle(f64),
    /// Split the sweep by this shape.
    Tool(Shape),
}

fn fe(m: &str) -> FeatureError {
    FeatureError::new(m)
}

/// Point inside the profile face (projection of its centroid).
fn profile_point(p: &Profile) -> FResult<DVec3> {
    let c = p.shape.centroid()?;
    Ok(p.shape.project_point(c).map(|x| x.point).unwrap_or(c))
}

/// Revolve angle (0, 2π] from `from` to `to` around the axis.
fn angle_to(origin: DVec3, axis: DVec3, from: DVec3, to: DVec3) -> Option<f64> {
    let radial = |q: DVec3| {
        let v = q - origin;
        v - axis * v.dot(axis)
    };
    let (a, b) = (radial(from), radial(to));
    if a.length() < 1e-12 || b.length() < 1e-12 {
        return None;
    }
    let mut ang = a.cross(b).dot(axis).atan2(a.dot(b));
    if ang <= 1e-12 {
        ang += TAU;
    }
    Some(ang)
}

/// One side's end (`prefix` "end" or "start" selects the parameter ids).
fn side_end(ev: &EvalCtx, p: &Profile, origin: DVec3, axis: DVec3, first: bool) -> FResult<Side> {
    let (bound_id, face_id, body_id, vertex_id, has_off, off_id, off_flip) = if first {
        ("endBound", "endBoundEntityFace", "endBoundEntityBody", "endBoundEntityVertex", "endBoundHasOffset", "endBoundOffset", "endBoundOffsetFlip")
    } else {
        ("startBound", "startBoundEntityFace", "startBoundEntityBody", "startBoundEntityVertex", "startBoundHasOffset", "startBoundOffset", "startBoundOffsetFlip")
    };
    let bound = ev.enum_(bound_id).to_string();
    if bound == "BLIND" {
        // std revolve.fs: a blind second end position is a start angle in
        // the revolve's own sense (the sweep runs from it to the end); its
        // flip only applies to the up-to types (live oracle_p1 R4/R4b).
        return Ok(Side::Angle(if first { ev.angle("angle")? } else { ev.angle("angleBack")?.rem_euclid(TAU) }));
    }
    // Positive offsets stop short of the bounding entity.
    let off = if ev.bool(has_off) {
        let o = ev.angle(off_id)?;
        if ev.bool(off_flip) { -o } else { o }
    } else {
        0.0
    };
    let rot_back = |s: Shape| -> FResult<Shape> {
        if off == 0.0 {
            return Ok(s);
        }
        Ok(s.transformed(&k::Transform::Rotate { origin, axis, angle: -off })?)
    };
    match bound.as_str() {
        "UP_TO_VERTEX" => {
            let Some(r) = ev.query(vertex_id).first() else { return err("Select a vertex to revolve up to") };
            let q = ev.resolve_vertex_point(r).ok_or_else(|| fe("Up to vertex no longer exists"))?;
            let a = angle_to(origin, axis, profile_point(p)?, q).ok_or_else(|| fe("The vertex lies on the revolve axis"))?;
            let a = a - off;
            if a <= 1e-9 {
                return err("The revolve offset is larger than the angle to the vertex");
            }
            Ok(Side::Angle(a))
        }
        "UP_TO_SURFACE" => {
            let Some(r) = ev.query(face_id).first() else { return err("Select a face to revolve up to") };
            let target = match r {
                Reference::Face(_) => {
                    let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(r)).first().ok_or_else(|| fe("Up to face no longer exists"))?;
                    let f = ev.face_shape(bi, fi)?;
                    match f.face_plane()? {
                        // Planar faces bound like their (large) plane.
                        Some(pl) => big_face(ev, &pl)?,
                        None => f,
                    }
                }
                _ => {
                    let f = ev.resolve_plane(r).ok_or_else(|| fe("Select a face or plane"))?;
                    big_face(ev, &kernel_plane(&f))?
                }
            };
            Ok(Side::Tool(rot_back(target)?))
        }
        "UP_TO_BODY" => {
            let Some(r) = ev.query(body_id).first() else { return err("Select a part to revolve up to") };
            let bi = *ev.resolve_bodies(std::slice::from_ref(r)).first().ok_or_else(|| fe("Up to part no longer exists"))?;
            Ok(Side::Tool(rot_back(ev.ctx.bodies[bi].shape.clone())?))
        }
        "UP_TO_NEXT" => {
            let solids: Vec<Shape> = ev.ctx.bodies.iter().filter(|b| b.kind == BodyKind::Solid).map(|b| b.shape.clone()).collect();
            if solids.is_empty() {
                return err("Nothing to revolve up to");
            }
            Ok(Side::Tool(rot_back(Shape::compound(&solids))?))
        }
        other => err(format!("Unsupported end type {other}")),
    }
}

fn big_face(ev: &EvalCtx, pl: &k::Plane) -> FResult<Shape> {
    let ext = ev.model_extent() * 4.0;
    let c = ev.ctx.bounds().map(|(a, b)| (a + b) * 0.5).unwrap_or(DVec3::ZERO);
    let l = pl.to_local(c);
    Ok(k::make_plane_face(pl, l.x - ext, l.x + ext, l.y - ext, l.y + ext)?)
}

/// Named revolve of one side up to its end, around `axis` (already
/// oriented). `cap_end` names the far cap.
fn revolve_side(ev: &EvalCtx, p: &Profile, origin: DVec3, axis: DVec3, side: Side, cap_end: bool) -> FResult<NamedShape> {
    let far_role = |p: &Profile| if cap_end { Role::CapEnd(Some(p.key.clone())) } else { Role::CapStart(Some(p.key.clone())) };
    match side {
        Side::Angle(a) => {
            let (axis, a) = if a < 0.0 { (-axis, -a) } else { (axis, a) };
            let res = k::revolve(&p.shape, origin, axis, a.min(TAU))?;
            Ok(NamedShape::new(res.shape.clone(), side_names(ev, p, &res, &far_role(p))))
        }
        Side::Tool(tool) => {
            // Almost a full turn, split by the bounding entity.
            let res = k::revolve(&p.shape, origin, axis, TAU * (1.0 - 1e-4)).map_err(|e| fe(&format!("Revolve failed: {e}")))?;
            let names = names_for(&res.shape, side_names(ev, p, &res, &far_role(p)));
            let split = k::split(std::slice::from_ref(&res.shape), std::slice::from_ref(&tool), &k::BooleanOptions::default()).map_err(|e| fe(&format!("Revolve up to failed: {e}")))?;
            // Faces coming from the tool are the new end cap.
            let mut roles = BTreeMap::new();
            for r in &split.history.records {
                if r.arg >= 1 && r.input_kind == ShapeType::Face && r.output_kind == ShapeType::Face && matches!(r.relation, k::Relation::Modified | k::Relation::Kept | k::Relation::Generated) {
                    roles.insert(r.output_index, far_role(p));
                }
            }
            let empty = ShapeNames::default();
            let all = ev.name_result(&[&names, &empty], &split.history, &split.shape, &roles, None);
            // The piece containing the start of the sweep.
            let start = profile_point(p)?;
            let probe = k::Transform::Rotate { origin, axis, angle: 1e-3 }.matrix().transform_point3(start);
            let piece = split
                .shape
                .solids()
                .into_iter()
                .find(|s| matches!(s.classify_point(probe, 1e-9), Ok(k::PointClass::Inside)))
                .ok_or_else(|| fe("Revolve up to failed: the bounding entity does not stop the revolve"))?;
            if piece.volume().unwrap_or(0.0) >= res.shape.volume().unwrap_or(0.0) * (1.0 - 1e-6) {
                return err("The revolve does not reach the bounding entity");
            }
            let faces = piece
                .faces()
                .iter()
                .map(|f| split.shape.index_of(f, ShapeType::Face).and_then(|i| all.get(i).cloned()).unwrap_or_else(|| std::sync::Arc::new(crate::reference::TopoName::face(ev.feature, Role::New(0)))))
                .collect();
            Ok(NamedShape::new(piece, faces))
        }
    }
}

/// Start cap, far cap (`far` role) and side names of one revolved side.
fn side_names(ev: &EvalCtx, p: &Profile, res: &k::OpResult, far: &Role) -> Vec<crate::naming::Name> {
    let mut roles = BTreeMap::new();
    for f in res.history.first_faces() {
        roles.insert(f, Role::CapStart(Some(p.key.clone())));
    }
    for f in res.history.last_faces() {
        roles.insert(f, far.clone());
    }
    for r in &res.history.records {
        if r.arg == 0
            && r.input_kind == ShapeType::Edge
            && r.relation == k::Relation::Generated
            && r.output_kind == ShapeType::Face
            && let Some(src) = p.edge_sources.get(r.input_index as usize)
        {
            roles.entry(r.output_index).or_insert_with(|| Role::Side(src.clone()));
        }
    }
    let empty = ShapeNames::default();
    ev.name_result(&[&empty], &res.history, &res.shape, &roles, None)
}

fn partial(ev: &EvalCtx, p: &Profile, origin: DVec3, axis: DVec3) -> FResult<NamedShape> {
    let blind = ev.is("endBound", "BLIND");
    if blind && ev.bool("symmetric") {
        let a = ev.angle("angle")?;
        let res = k::revolve_between(&p.shape, origin, axis, -a * 0.5, a * 0.5)?;
        return Ok(NamedShape::new(res.shape.clone(), sweep_names(ev, p, &res, true, true)));
    }
    let first = side_end(ev, p, origin, axis, true)?;
    let second = if ev.bool("hasStartBound") { Some(side_end(ev, p, origin, -axis, false)?) } else { None };
    match (first, second) {
        (Side::Angle(a), None) => {
            let res = k::revolve(&p.shape, origin, axis, a)?;
            Ok(NamedShape::new(res.shape.clone(), sweep_names(ev, p, &res, true, true)))
        }
        // Two blind angles: one revolve from the start angle to the end
        // (start 45°, end 90°: 45° to 90°; start 330°, end 30°: −30° to 30°).
        (Side::Angle(a), Some(Side::Angle(_))) if ev.is("startBound", "BLIND") => {
            let start = ev.angle("angleBack")?.rem_euclid(TAU);
            let res = k::revolve_between(&p.shape, origin, axis, if start < a { start } else { start - TAU }, a)?;
            Ok(NamedShape::new(res.shape.clone(), sweep_names(ev, p, &res, true, true)))
        }
        (first, None) => revolve_side(ev, p, origin, axis, first, true),
        // Up to an entity from a blind start angle: the profile turned to
        // the start first.
        (first, Some(Side::Angle(start))) if ev.is("startBound", "BLIND") => {
            if start.abs() < 1e-12 {
                return revolve_side(ev, p, origin, axis, first, true);
            }
            let turned = Profile {
                shape: p.shape.transformed(&k::Transform::Rotate { origin, axis, angle: start })?,
                frame: p.frame,
                edge_sources: p.edge_sources.clone(),
                key: p.key.clone(),
                from_face: p.from_face,
            };
            revolve_side(ev, &turned, origin, axis, first, true)
        }
        (first, Some(second)) => {
            let a = revolve_side(ev, p, origin, axis, first, true)?;
            let b = revolve_side(ev, p, origin, -axis, second, false)?;
            let fused = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(&a.shape), std::slice::from_ref(&b.shape), &k::BooleanOptions::default())?;
            let names = ev.name_result(&[&a.names, &b.names], &fused.history, &fused.shape, &BTreeMap::new(), None);
            let unified = k::unify_same_domain(&fused.shape)?;
            let un = names_for(&fused.shape, names);
            let names = ev.name_result(&[&un], &unified.history, &unified.shape, &BTreeMap::new(), None);
            Ok(NamedShape::new(unified.shape, names))
        }
    }
}
