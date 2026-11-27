//! Extrude (solid and surface), with all end conditions, second direction,
//! starting offset and draft.

use crate::display::BodyKind;
use crate::features::common::is_up_to;
use crate::reference::{FeatureId, Reference, Source};
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
    // Up to next that finds nothing to stop at: EXTRUDE_UPTO_NEXT_NO_DIVISION (live err18).
    let next = ev.is("endBound", "UP_TO_NEXT") || (ev.bool("hasSecondDirection") && ev.is("secondDirectionBound", "UP_TO_NEXT"));
    let res = eval_inner(ev).map_err(|e| {
        // (A Boolean noshape's declined with NOSHAPE_BOOL_FALLBACK=0: its reason kept.)
        if e.message.starts_with(crate::regen::eval::KERNEL_FAILURE) && !e.message.contains("noshape's Boolean declined") {
            log::debug!("extrude: {}", e.message);
            let message = if next { "Up to next termination problem, consider switching it to up to part." } else { "Failed to extrude selections, check input." };
            crate::regen::eval::FeatureError { message: message.into(), ..e }
        } else {
            e
        }
    });
    // Live errors-1123 depth0: its region drawn red.
    if res.as_ref().is_err_and(|e| e.message == "Failed to extrude selections, check input.") {
        ev.error_refs = ev.query("entities").to_vec();
    }
    res
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
    // std extrude.fs: a model-space solid Extrude of a sheet metal flat's sketch regions (live smflat-1122 Extrude 8).
    let on_flat = |r: &Reference| matches!(r, Reference::SketchRegion { sketch, .. } if ev.ctx.flat_sketches.contains_key(sketch));
    if !thin && refs.iter().any(on_flat) {
        let message = if refs.iter().all(on_flat) { "Feature defined in model space cannot reference sheet metal flat geometry." } else { "Cannot select from the flattened sheet metal view and model view at the same time." };
        return crate::regen::eval::err_at(&["entities"], message);
    }
    let t0 = std::time::Instant::now();
    let profiles = if thin { super::thin::thin_profiles_sided(ev, refs, super::thin::WallSide::Along(&|f, _| f.normal), false)? } else { ev.profiles(&ev.distinct_regions(refs))? };
    if profiles.is_empty() {
        // (What was picked is gone: std's query is empty.)
        return crate::regen::eval::err_at(&["entities"], "Select face or sketch region to extrude.");
    }
    let t_profiles = t0.elapsed();
    let mut tools = Vec::new();
    if profiles.len() >= 16 && profiles.iter().all(|p| !p.from_face) {
        // Many sketch regions: each prism on its own profile (they share no
        // edges), in parallel, kept in order.
        let dirs = profiles.iter().map(|p| direction(ev, p, thin)).collect::<FResult<Vec<_>>>()?;
        let ev_ro: &EvalCtx = ev;
        use rayon::prelude::*;
        let legacy = noshape_kernel::kernel_legacy();
        let made: Vec<FResult<NamedShape>> = profiles.par_iter().zip(dirs.par_iter()).map(|(p, d)| noshape_kernel::with_kernel_legacy(legacy.clone(), || extrude_profile(ev_ro, p, *d))).collect();
        tools = made.into_iter().collect::<FResult<Vec<_>>>()?;
    } else {
        for p in &profiles {
            let dir = direction(ev, p, thin)?;
            tools.push(extrude_profile(ev, p, dir)?);
        }
    }
    let tools = tools.into_iter().map(|t| drawn_edge_directions(ev, t)).collect::<FResult<Vec<_>>>()?;
    let t_tools = t0.elapsed();
    let op = ev.enum_("operationType").to_string();
    let scope = scope(ev);
    let sketches: Vec<Option<FeatureId>> = profiles.iter().map(|p| if let Source::Region { sketch, .. } = &p.key { Some(*sketch) } else { None }).collect();
    let first_new = ev.ctx.bodies.len();
    let r = ev.apply_operation_from(tools, (!thin).then_some(&sketches[..]), &op, scope, BodyKind::Solid);
    if r.is_ok() && !thin && op == "NEW" {
        number_in_drawing_order(ev, first_new);
    }
    if crate::regen::display::perf_enabled() {
        log::info!(target: "perf", "extrude: {} profiles {:.1} ms, tools {:.1} ms, operation {:.1} ms", profiles.len(), t_profiles.as_secs_f64() * 1e3, (t_tools - t_profiles).as_secs_f64() * 1e3, (t0.elapsed() - t_tools).as_secs_f64() * 1e3);
    }
    r
}

/// A New extrude's parts of one sketch's regions are listed and numbered in the sketch's drawing order, whatever the
/// pick order (live q-extrude-part-order-1119: two circles, three, two rectangles, both pick orders); a part placed by
/// its region's outer loop's first drawn entity (welding 9c7704 Extrude 8: the disc inside the ring, its circle drawn
/// first, is Part 5). Only names, colours and list places move: ids and face names stay the pick order's, which saved
/// documents hold.
fn number_in_drawing_order(ev: &mut EvalCtx, first_new: usize) {
    use crate::reference::{Role, TopoName};
    let Some(new) = ev.ctx.bodies.untracked().get(first_new..).filter(|b| b.len() > 1) else { return };
    let drawn = |b: &crate::regen::context::Body| -> Option<(FeatureId, u32)> {
        let regions = b.faces.iter().filter_map(|n| match &**n {
            TopoName::Face { feature, role: Role::CapStart(Some(Source::Region { sketch, key })) } if *feature == ev.feature => Some((*sketch, key)),
            _ => None,
        });
        let mut sketch = None;
        let mut first = None::<u32>;
        for (sk, key) in regions {
            if sketch.is_some_and(|s| s != sk) {
                return None;
            }
            sketch = Some(sk);
            let at = ev.resolve_region(sk, key).and_then(|(_, region)| region.outer.sources.iter().map(|s| s.entity.0).min())?;
            first = Some(first.map_or(at, |f| f.min(at)));
        }
        Some((sketch?, first?))
    };
    let Some(keys) = new.iter().map(|b| drawn(b)).collect::<Option<Vec<_>>>() else { return };
    if keys.iter().any(|k| k.0 != keys[0].0) {
        return;
    }
    let mut order: Vec<usize> = (0..new.len()).collect();
    order.sort_by_key(|&i| keys[i].1);
    if order.iter().enumerate().all(|(k, &i)| k == i) {
        return;
    }
    let labels: Vec<(String, usize)> = new.iter().map(|b| (b.name.clone(), b.color)).collect();
    let moved: Vec<_> = order.iter().zip(labels).map(|(&i, (name, color))| std::sync::Arc::new(new[i].renamed(name).recoloured(color))).collect();
    for (k, b) in moved.into_iter().enumerate() {
        ev.ctx.bodies.replace_untracked(first_new + k, b);
    }
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
                    let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(r)).first().ok_or_else(|| crate::regen::eval::FeatureError::at(&[face_id], "Select a face to terminate the extrude."))?;
                    let face = ev.face_shape(bi, fi)?;
                    // A planar face: its plane (live ends a profile reaching past the face's edges on its surface:
                    // dx's EXTRUDE-UP-TO-FACE-BEYOND).
                    // A cylinder: the whole cylinder, as live ends a profile reaching past the face round it (c2-rings-loft's
                    // Extrude 3: up to the bore, of which its rounds left the top).
                    let ext = ev.model_extent() * 4.0;
                    // A mesh face as it is (live: up to it where it covers the profile, MIXED; qa mistakes-1125).
                    if face.is_mesh() {
                        let target = if off != 0.0 { face.translated(-dir * off)? } else { face };
                        return Ok(End::UpTo(target));
                    }
                    match (face.face_plane()?, face.face_surface()?) {
                        (Some(pl), _) => k::make_plane_face(&pl, -ext, ext, -ext, ext)?,
                        (None, k::Surface::Cylinder { origin, axis, x_dir, radius }) => {
                            let axis = axis.normalize();
                            let whole = k::make_cylinder(&k::Plane::new(origin - axis * ext, axis, x_dir), radius, 2.0 * ext, std::f64::consts::TAU)?;
                            whole.faces().into_iter().find(|f| matches!(f.face_surface(), Ok(k::Surface::Cylinder { .. }))).unwrap_or(face)
                        }
                        // A B-spline: its whole surface, without the face's trims (c2-hunting-knife's Extrude 10: a pin up
                        // to the handle's side, through the pin's holes in it).
                        (None, k::Surface::BSpline { .. }) => k::extend_face_over(&face, &[], 0.0).unwrap_or(face),
                        _ => face,
                    }
                }
                _ => {
                    let f = ev.resolve_plane(r).ok_or_else(|| crate::regen::eval::FeatureError::at(&[face_id], "Select a face to terminate the extrude."))?;
                    let ext = ev.model_extent() * 4.0;
                    k::make_plane_face(&crate::regen::eval::kernel_plane(&f), -ext, ext, -ext, ext)?
                }
            };
            let target = if off != 0.0 { target.translated(-dir * off)? } else { target };
            Ok(End::UpTo(target))
        }
        "UP_TO_BODY" => {
            let Some(r) = ev.query(body_id).first() else { return err_at(&[body_id], "Select a part to terminate the extrude.") };
            let bi = *ev.resolve_bodies(std::slice::from_ref(r)).first().ok_or_else(|| crate::regen::eval::FeatureError::at(&[body_id], "Select a part to terminate the extrude."))?;
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
            let p = ev.resolve_vertex_point(r).ok_or_else(|| crate::regen::eval::FeatureError::at(&[vertex_id], "Select a vertex or mate connector to terminate the extrude."))?;
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
        Reference::SketchEntity { sketch, entity } => match ev.ctx.sketch_holding(*sketch, *entity).and_then(|s| Some((s.frame, s.sketch.curve(noshape_sketch::EntityId(*entity))?))) {
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
    if matches!(r, Reference::Face(_) | Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. }) {
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

/// What Up to next extrudes up to: the parts and surfaces (std's BoundingType: "the next solid body or surface body";
/// c2-surf-workshop-split's surface Extrude 1 up to its Boundary surface).
pub fn next_parts(ev: &EvalCtx) -> FResult<k::Shape> {
    let solids: Vec<k::Shape> = ev.ctx.bodies.iter().filter(|b| matches!(b.kind, BodyKind::Solid | BodyKind::Surface)).map(|b| b.shape.clone()).collect();
    if solids.is_empty() {
        return err("Parts or Surfaces must be present in the Part Studio.");
    }
    Ok(k::Shape::compound(&solids))
}

/// How far along `dir` the parts reach past `profile`, with a tenth of their size to spare: a drafted Through all's
/// length (the model's extent, 2 m at least, flared c2-valve-config's drafted holes into cones a metre wide, meeting
/// below its disc, and their cut took one at a time). None where no part lies ahead.
fn reach(ev: &EvalCtx, profile: &k::Shape, dir: DVec3) -> Option<f64> {
    let (a, b) = ev.ctx.bounds()?;
    let p = profile.bounding_box(false).ok()?;
    let dir = dir.normalize_or_zero();
    let corners = |lo: DVec3, hi: DVec3| (0..8).map(move |i| DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z }).dot(dir));
    let far = corners(a, b).fold(f64::MIN, f64::max) - corners(p.min, p.max).fold(f64::MAX, f64::min);
    (far > 0.0).then(|| far + 0.1 * (b - a).length())
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
            // std processStartOffsetData: along the extrude axis before Opposite direction turns it (live:
            // c2-fan-blade-sweep's Extrude 4, 0.1 mm behind its sketch with Opposite direction ticked).
            let axis = if ev.bool("oppositeDirection") { -dir } else { dir };
            start_shift = axis * if ev.bool("startOffsetOppositeDirection") { -d } else { d };
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
    // (A cut's or intersect's only: a New or Add body keeps its whole length.)
    let cuts = matches!(ev.seed_op.as_deref().unwrap_or(ev.enum_("operationType")), "REMOVE" | "INTERSECT");
    let end = match end {
        End::Distance(d) if cuts && bound == "THROUGH_ALL" && draft != 0.0 => {
            let r = if symmetric { reach(ev, &profile, dir).zip(reach(ev, &profile, -dir)).map(|(a, b)| 2.0 * a.max(b)) } else { reach(ev, &profile, dir) };
            End::Distance(r.map_or(d, |r| r.min(d)))
        }
        e => e,
    };

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
        // std: the second side runs away from the first only when its flip
        // differs from the first's (both flips are against the normal).
        let away = ev.bool("secondDirectionOppositeDirection") != ev.bool("oppositeDirection");
        let d2 = if away { -dir } else { dir };
        let end2 = end_for(ev, &bound2, "secondDirectionDepth", "secondDirectionBoundEntityFace", "secondDirectionBoundEntityBody", "secondDirectionBoundEntityVertex", "hasSecondDirectionOffset", "secondDirectionOffsetDistance", "secondDirectionOffsetOppositeDirection", d2)?;
        // std: the second side has only its own draft (unticked: away from
        // the profile plane, like the first).
        let draft2 = if ev.bool("hasSecondDirectionDraft") && d2.dot(dir) < 0.0 {
            let a = ev.angle("secondDirectionDraftAngle")?;
            if ev.bool("secondDirectionDraftPullDirection") { a } else { -a }
        } else {
            0.0
        };
        let end2 = match end2 {
            End::Distance(d) if cuts && bound2 == "THROUGH_ALL" && draft2 != 0.0 => End::Distance(reach(ev, &profile, d2).map_or(d, |r| r.min(d))),
            e => e,
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
    // Both sides the same way (std's isStartBoundOpposite false): opExtrude's solid runs between the two caps, not
    // from the profile (asm-suspension's Knuckle Extrude 2: up to a face 20 mm short of it, then through all).
    let fused = if d2.dot(dir) > 0.0 {
        let (long, short) = if r1.shape.volume()? >= r2.shape.volume()? { (&r1.shape, &r2.shape) } else { (&r2.shape, &r1.shape) };
        let between = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(long), std::slice::from_ref(short), &k::BooleanOptions::default())?;
        if between.shape.count(k::ShapeType::Solid) == 0 {
            return err("Failed to extrude selections, check input.");
        }
        between
    } else {
        k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(&r1.shape), std::slice::from_ref(&r2.shape), &k::BooleanOptions::default())?
    };
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
    // A sketch picked whole (its row in the feature list: "Edges of Sketch 2",
    // qa SURF-EXTRUDE-SKETCH-EDGES) stands for its curves, as live.
    let mut curves: Vec<(crate::reference::FeatureId, u32)> = Vec::new();
    for r in refs {
        match r {
            Reference::SketchEntity { sketch, entity } => curves.push((*sketch, *entity)),
            Reference::Sketch(sketch) => {
                if let Some(s) = ev.ctx.sketch(*sketch) {
                    curves.extend(s.sketch.entities().filter(|e| e.kind().is_curve() && !e.construction).map(|e| (*sketch, e.id.0)));
                }
            }
            _ => {}
        }
    }
    let mut tools = Vec::new();
    let mut ends: Vec<Vec<DVec3>> = Vec::new();
    for (sketch, entity) in &curves {
        let Some(s) = ev.ctx.sketch_holding(*sketch, *entity).cloned() else { continue };
        // Every curve kind (splines and conics too, live: cf-curve-pattern-normal's fit spline).
        let Some(edge) = super::bshared::sketch_edge(ev, *sketch, *entity)? else { continue };
        ends.push(edge.vertices().iter().filter_map(|v| v.vertex_point().ok()).collect());
        let dir = if ev.bool("oppositeDirection") { -s.frame.normal } else { s.frame.normal };
        let res = sheet_extrude(ev, &edge, &s.frame, dir)?;
        let mut roles = std::collections::BTreeMap::new();
        for (i, _) in res.shape.faces().iter().enumerate() {
            roles.insert(i as u32, crate::reference::Role::Side(crate::reference::Source::Sketch { sketch: *sketch, entity: *entity, segment: i as u16 }));
        }
        let names = ev.name_result(&[&Default::default()], &res.history, &res.shape, &roles, None);
        tools.push(NamedShape::new(res.shape, names));
    }
    if tools.is_empty() {
        return crate::regen::eval::err_at(&["surfaceEntities"], "Select sketch curves to extrude.");
    }
    // One surface per chain of curves meeting end to end, as live (c2-surf-intro-simple's Extrude 1: a spline apart
    // from a rectangle, 2 surfaces).
    let mut chains: Vec<(Vec<NamedShape>, Vec<DVec3>)> = Vec::new();
    for (tool, points) in tools.into_iter().zip(ends) {
        let mut chain = (vec![tool], points);
        let mut i = chains.len();
        while i > 0 {
            i -= 1;
            if chains[i].1.iter().any(|p| chain.1.iter().any(|q| p.distance(*q) <= 1e-6)) {
                let (t, p) = chains.remove(i);
                chain.0.extend(t);
                chain.1.extend(p);
            }
        }
        chains.push(chain);
    }
    for (i, (mut tools, _)) in chains.into_iter().enumerate() {
        let shapes: Vec<k::Shape> = tools.iter().map(|t| t.shape.clone()).collect();
        let sewn = if shapes.len() > 1 { k::sew(&shapes, 1e-6, false).map(|r| r.shape).unwrap_or_else(|_| k::Shape::compound(&shapes)) } else { shapes[0].clone() };
        let names: Vec<_> = tools.iter().flat_map(|t| t.names.faces.clone()).collect();
        let named = if sewn.count(k::ShapeType::Face) == names.len() { NamedShape::new(sewn, names) } else { tools.remove(0) };
        // Add sews it to the surfaces it meets (c2-surf-workshop-split's Extrude 2 up to Surface 1's hole: one sheet).
        if ev.is("surfaceOperationType", "ADD") {
            super::bshared::surface_result(ev, named)?;
        } else {
            ev.add_new_bodies(named, BodyKind::Surface, i as u32)?;
        }
    }
    Ok(())
}

/// One curve's sheet with the solid's end conditions (std extrude.fs: opExtrude of the curves): blind, symmetric,
/// up to a face, part, the next part or a vertex, a starting offset, a second direction and draft.
fn sheet_extrude(ev: &EvalCtx, edge: &k::Shape, frame: &crate::display::PlaneFrame, dir: DVec3) -> FResult<k::OpResult> {
    let bound = ev.enum_("endBound").to_string();
    let end = end_for(ev, &bound, "depth", "endBoundEntityFace", "endBoundEntityBody", "endBoundEntityVertex", "hasOffset", "offsetDistance", "offsetOppositeDirection", dir)?;
    let draft = if ev.bool("hasDraft") {
        let a = ev.angle("draftAngle")?;
        if ev.bool("draftPullDirection") { a } else { -a }
    } else {
        0.0
    };
    let mut start_shift = DVec3::ZERO;
    if ev.bool("startOffset") {
        if ev.is("startOffsetBound", "BLIND") {
            let d = ev.length("startOffsetDistance")?;
            start_shift = dir * if ev.bool("startOffsetOppositeDirection") { -d } else { d };
        } else {
            let Some(r) = ev.query("startOffsetEntity").first() else { return err_at(&["startOffsetEntity"], "Select a valid entity.") };
            let t = start_offset_point(ev, r, frame.normal)?;
            start_shift = dir * ((t - frame.origin).dot(frame.normal) / dir.dot(frame.normal));
        }
    }
    let profile = if start_shift.length() > 0.0 { edge.translated(start_shift)? } else { edge.clone() };
    // Up to a vertex: as far as its height over the profile's plane (a lone line spans no plane of its own).
    let origin = frame.origin + start_shift;
    let flat = |end: End, d: DVec3| match end {
        End::UpToPoint(pt) => End::Distance((pt - origin).dot(d)),
        e => e,
    };
    let end = flat(end, dir);
    if ev.bool("symmetric") && matches!(bound.as_str(), "BLIND" | "THROUGH_ALL") {
        let End::Distance(depth) = end else { unreachable!("symmetric is only offered for blind / through all") };
        return Ok(k::extrude_between(&profile, -dir * depth * 0.5, dir * depth * 0.5, draft)?);
    }
    if !ev.bool("hasSecondDirection") {
        return one_side(&profile, dir, end, draft);
    }
    let away = ev.bool("secondDirectionOppositeDirection") != ev.bool("oppositeDirection");
    let d2 = if away { -dir } else { dir };
    let bound2 = ev.enum_("secondDirectionBound").to_string();
    let end2 = flat(end_for(ev, &bound2, "secondDirectionDepth", "secondDirectionBoundEntityFace", "secondDirectionBoundEntityBody", "secondDirectionBoundEntityVertex", "hasSecondDirectionOffset", "secondDirectionOffsetDistance", "secondDirectionOffsetOppositeDirection", d2)?, d2);
    let draft2 = if ev.bool("hasSecondDirectionDraft") && d2.dot(dir) < 0.0 {
        let a = ev.angle("secondDirectionDraftAngle")?;
        if ev.bool("secondDirectionDraftPullDirection") { a } else { -a }
    } else {
        0.0
    };
    if let (End::Distance(x1), End::Distance(x2)) = (&end, &end2)
        && draft == 0.0
        && draft2 == 0.0
    {
        // (Swept from the second end to the first, as live's: c2-surf-intro-boundary's Tail fin surface's side edge
        // J1F runs from its second end to its first, and 3D fit spline 1 leaves along it.)
        return Ok(k::extrude_between(&profile, dir * *x1, d2 * *x2, 0.0)?);
    }
    // Two separately bounded halves, sewn along the curve.
    let r1 = one_side(&profile, dir, end, draft)?;
    let r2 = one_side(&profile, d2, end2, draft2)?;
    Ok(k::sew(&[r1.shape, r2.shape], 1e-6, false)?)
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

/// The tool's cap edges running as their sketch lines and arcs were drawn (live: an extruded rectangle's edges keep the
/// drawing order, a pattern along one goes its way; PATTERN-EDGE-DIRECTION), where the profile's loop runs them the
/// other way: their curves reversed, the swept faces as the loop made them (a unify merges those; built from the
/// drawn ways they didn't merge, pulley02's fillet and clamp-hms's extrude). Studios saved before keep the loop's.
fn drawn_edge_directions(ev: &EvalCtx, t: NamedShape) -> FResult<NamedShape> {
    use crate::reference::{Role, TopoName};
    let side = |f: &TopoName| match f {
        TopoName::Face { role: Role::Side(Source::Sketch { sketch, entity, .. }), .. } => Some((*sketch, *entity)),
        _ => None,
    };
    let cap = |f: &TopoName| matches!(f, TopoName::Face { role: Role::CapStart(_) | Role::CapEnd(_), .. });
    let edges = t.shape.edges();
    let mut flip = Vec::new();
    for (i, n) in t.names.edges.iter().enumerate() {
        let TopoName::Edge { faces: [a, b], .. } = &**n else { continue };
        let Some((sk, entity)) = side(a).filter(|_| cap(b)).or_else(|| side(b).filter(|_| cap(a))) else { continue };
        let (Some(s), Some(e)) = (ev.ctx.sketch(sk), edges.get(i)) else { continue };
        if s.legacy_edge_directions {
            return Ok(t);
        }
        let (Some(curve), Ok(info)) = (s.sketch.curve(noshape_sketch::EntityId(entity)), e.edge_info()) else { continue };
        let against = match (curve, info.curve) {
            (noshape_sketch::Curve2::Line { a, b }, k::Curve::Line { .. }) => {
                let d = b - a;
                (info.end - info.start).dot(s.frame.x * d.x + s.frame.y * d.y) < 0.0
            }
            // (Sketch arcs and circles run counter-clockwise about the sketch's normal, those stored clockwise clockwise:
            // live core q-arc-sense-1120.)
            (noshape_sketch::Curve2::Arc { .. } | noshape_sketch::Curve2::Circle { .. }, k::Curve::Circle { axis, .. }) => {
                let clockwise = s.sketch.entity(noshape_sketch::EntityId(entity)).is_some_and(|e| e.clockwise);
                (axis.dot(s.frame.x.cross(s.frame.y)) < 0.0) != clockwise
            }
            _ => false,
        };
        if against {
            flip.push(e.clone());
        }
    }
    if flip.is_empty() {
        return Ok(t);
    }
    Ok(NamedShape { shape: k::reverse_edge_curves(&t.shape, &flip)?, names: t.names })
}

/// Extrude's domain from its regions (live q-flat-ops-1119): a region of a sketch on a sheet metal flat makes it the
/// flat's (Add | Remove), regions elsewhere the model's; none leaves it. `None`: no change.
pub fn domain_logic(ctx: &crate::regen::context::Context, params: &crate::params::ParamValues) -> Option<&'static str> {
    let picks = params.query("entities");
    if picks.is_empty() {
        return None;
    }
    let flat = picks.iter().any(|r| matches!(r, Reference::SketchRegion { sketch, .. } if ctx.flat_sketches.contains_key(sketch)));
    let want = if flat { "FLAT" } else { "MODEL" };
    let now = match params.enum_("domain") {
        "" => "MODEL",
        d => d,
    };
    (now != want).then_some(want)
}
