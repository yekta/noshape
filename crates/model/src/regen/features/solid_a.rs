//! Sweep, Loft, Thicken, Enclose, Rib and the FeatureScript primitives
//! (Cube, Sphere).
//!
//! Thin sweeps sweep the offset wall regions of `thin`; thin lofts loft the
//! wire profiles into a surface and thicken it. Unsupported options return a
//! clear error (see each evaluator): loft guides and path.

use super::helpers_a::*;
use crate::display::BodyKind;
use crate::naming::Name;
use crate::params::ParamValues;
use crate::reference::{BodyId, Reference, Role, Source};
use crate::regen::eval::{EvalCtx, FResult, NamedShape, Profile, err, kernel_plane, sweep_names};
use glam::DVec3;
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;

// ------------------------------------------------------------------ sweep

/// Onshape's SWEEP_FAILED.
const SWEEP_FAILED: &str = "Could not create valid swept body, check input.";

/// Onshape's message for a circle swept round a bend tighter than itself
/// (live "kern (10-23 18:40)"): it depends on where the circle's start (its
/// x direction) points. In the bend's plane: SWEEP_FAILED (G1: Ø6 on Front,
/// R2 bend on Top); across it: "Result of sweep intersects itself" (Sw2: Ø10
/// on Right, R4 bend on Front); turning the circle's start 90° swaps them.
fn tight_bend_message(profile: &Shape, path: &Shape) -> &'static str {
    const SELF_INTERSECTS: &str = "Result of sweep intersects itself, adjust path or profile selections.";
    let circle = match profile.edges().as_slice() {
        [e] => match e.edge_curve() {
            Ok(k::Curve::Circle { x_dir, radius, .. }) => Some((x_dir.normalize(), radius)),
            _ => None,
        },
        _ => None,
    };
    let Some((x_dir, r)) = circle else { return SWEEP_FAILED };
    let across = path.edges().iter().any(|e| match e.edge_curve() {
        Ok(k::Curve::Circle { axis, radius, .. }) => radius < r && x_dir.dot(axis.normalize()).abs() > 0.5,
        _ => false,
    });
    if across { SELF_INTERSECTS } else { SWEEP_FAILED }
}

/// The part of the sweep path a profile in its middle sweeps along (live
/// L12): the piece from the profile toward the path's start, or toward its
/// end with the path's Opposite direction. Unchanged when the profile's
/// plane doesn't cut the path.
fn half_path(path: &Shape, origin: DVec3, normal: DVec3, opposite: bool) -> Shape {
    let x = if normal.cross(DVec3::X).length() > 0.1 { DVec3::X } else { DVec3::Y };
    let plane = k::Plane::new(origin, normal, x);
    let Some(start) = path.edges().first().and_then(|e| e.endpoints().ok()).map(|e| e.0) else { return path.clone() };
    let Ok(cut) = k::split_by_plane(path, &plane) else { return path.clone() };
    let edges = cut.shape.edges();
    if edges.len() <= path.edges().len() {
        return path.clone();
    }
    // The edges on the start's side of the profile's plane (or the other).
    let side = |p: DVec3| (p - origin).dot(normal) >= 0.0;
    let start_side = side(start);
    let kept: Vec<Shape> = edges
        .into_iter()
        .filter(|e| e.edge_info().ok().and_then(|i| e.point_at((i.first_param + i.last_param) * 0.5).ok()).is_some_and(|m| (side(m) == start_side) != opposite))
        .collect();
    if kept.is_empty() {
        return path.clone();
    }
    k::make_wire(&kept).unwrap_or_else(|_| path.clone())
}

/// A loft end condition as loft.fs createProfileConditions builds it, from
/// `vals` (the feature's parameters or a Boundary surface profile row) and
/// ids [condition, magnitude, direction]. The profile's plane is its own
/// or else its sketch's (`sketch_normal`). Match tangent / curvature aren't
/// applied yet (a warning).
pub(super) fn loft_end(ev: &mut EvalCtx, vals: &ParamValues, ids: [&str; 3], profile: &Shape, sketch_normal: Option<DVec3>, which: &str) -> FResult<k::LoftEnd> {
    let [cond, mag, dir] = ids;
    let kind = vals.enum_(cond).to_string();
    let m = |ev: &EvalCtx| ev.eval_num_in(vals, mag, crate::params::NumKind::Real);
    Ok(match kind.as_str() {
        "NORMAL_TO_PROFILE" | "TANGENT_TO_PROFILE" => {
            let own = profile.face_plane().ok().flatten().or_else(|| k::make_face_from_wire(profile, true).ok().and_then(|f| f.face_plane().ok().flatten()));
            let Some(n) = own.map(|p| p.unit_normal()).or(sketch_normal) else {
                return err(format!("Only planar profiles are valid for Normal/Tangent to profile {which} condition."));
            };
            if kind == "NORMAL_TO_PROFILE" { k::LoftEnd::AlongDirection(m(ev)?, n) } else { k::LoftEnd::AcrossDirection(m(ev)?, n) }
        }
        "TANGENT_DIRECTION" | "NORMAL_DIRECTION" => {
            let d = vals.query(dir).first().and_then(|r| ev.resolve_direction(r));
            let Some(d) = d else { return err(format!("Select direction for {which} profile tangency.")) };
            if kind == "TANGENT_DIRECTION" { k::LoftEnd::AlongDirection(m(ev)?, d) } else { k::LoftEnd::AcrossDirection(m(ev)?, d) }
        }
        "MATCH_TANGENT" | "MATCH_CURVATURE" => {
            if !ev.warnings.iter().any(|w| w.starts_with("Match tangent")) {
                ev.warnings.push("Match tangent / Match curvature conditions are not supported yet and were ignored".into());
            }
            k::LoftEnd::Free
        }
        _ => k::LoftEnd::Free,
    })
}

/// Onshape's LOFT_FAILED.
const LOFT_FAILED: &str = "Could not create loft with given information. Check profile order, guide/profile intersections or end conditions.";

pub fn sweep(ev: &mut EvalCtx) -> FResult<()> {
    let body_type = ev.enum_("bodyType").to_string();
    // sweep.fs: a thin sweep checks its wall before the path.
    if body_type == "THIN" && ev.query("wallShape").is_empty() {
        return crate::regen::eval::err_at(&["wallShape"], "Select faces or sketch regions to sweep.");
    }
    let path_edges = resolve_curves(ev, ev.query("path"))?;
    if path_edges.is_empty() {
        return crate::regen::eval::err_at(&["path"], "Select entities for sweep path.");
    }
    let wires = wires_from(&path_edges)?;
    if wires.len() != 1 {
        // Live L13: no field is flagged.
        return err("Sweep path curves are not all connected.");
    }
    let path = wires[0].clone();
    // Twist: total rotation about the path (Turns, Angle, or one turn per Pitch length).
    let twist = if ev.bool("hasTwist") && matches!(ev.enum_("profileControl"), "NONE" | "KEEP_ORIENTATION") {
        let total = match ev.enum_("twistType") {
            "ANGLE" => ev.angle("angle")?,
            "PITCH" => {
                let len: f64 = path.edges().iter().filter_map(|e| e.length().ok()).sum();
                let turns = len / ev.length("pitch")?;
                if !(0.0..=1000.0).contains(&turns) {
                    return crate::regen::eval::err_at(&["pitch"], format!("Computed number of revolutions outside limit (between 0 and 1000). Current value is {turns}."));
                }
                std::f64::consts::TAU * turns
            }
            _ => std::f64::consts::TAU * ev.real("turns")?,
        };
        if ev.bool("ccw") { total } else { -total }
    } else {
        0.0
    };
    let mut opts = k::SweepOptions { scale_end: if ev.bool("hasScale") { ev.real("scaleFactor")? } else { 1.0 }, twist, ..Default::default() };
    let lock_dir = match ev.enum_("profileControl") {
        "LOCK_DIRECTION" => {
            let no_dir = || crate::regen::eval::err_at(&["lockDirectionQuery"], "Select direction to lock.");
            let Some(r) = ev.query("lockDirectionQuery").first() else { return no_dir() };
            let Some(d) = ev.resolve_direction(r) else { return no_dir() };
            Some(d)
        }
        _ => None,
    };
    let keep = ev.is("profileControl", "KEEP_ORIENTATION");
    // Lock profile faces: the profile keeps its orientation relative to the
    // faces' normal along the path (which lies on them).
    let lock_faces = if ev.is("profileControl", "LOCK_FACES") {
        let faces: Vec<Shape> = ev.resolve_faces(ev.query("lockFaces")).into_iter().filter_map(|(bi, fi)| ev.ctx.bodies[bi].shape.sub_shape(ShapeType::Face, fi).ok()).collect();
        // Live L13: no faces to lock sweeps as with no profile control.
        (!faces.is_empty()).then(|| Shape::compound(&faces))
    } else {
        None
    };
    let (extend, path_opposite) = (ev.bool("extendToFullPath"), ev.bool("pathOppositeDirection"));
    let sweep = |profile: &Shape, frame: Option<&crate::display::PlaneFrame>, opts: &k::SweepOptions| {
        let path = match frame {
            Some(f) if !extend => half_path(&path, f.origin, f.normal, path_opposite),
            _ => path.clone(),
        };
        match &lock_faces {
            Some(f) => k::sweep_locked_to_faces(std::slice::from_ref(profile), &path, f, opts),
            None => k::sweep(profile, &path, opts),
        }
    };
    let mut tools = Vec::new();
    if body_type == "SURFACE" {
        let edges = resolve_curves(ev, ev.query("surfaceProfiles"))?;
        if edges.is_empty() {
            return err("Select sketch curves to sweep.");
        }
        opts.solid = false;
        for (i, w) in wires_from(&edges)?.into_iter().enumerate() {
            let p = wire_profile(w, &edges, Source::Index(i as u32));
            if keep {
                opts.mode = fixed_mode_for(&p.shape)?;
            } else if let Some(d) = lock_dir {
                opts.mode = k::SweepMode::Binormal(d);
            }
            let res = sweep(&p.shape, Some(&p.frame), &opts).map_err(|_| fe(SWEEP_FAILED))?;
            let names = sweep_names(ev, &p, &res, false, false);
            tools.push(NamedShape::new(res.shape, names));
        }
        let op = ev.enum_("surfaceOperationType").to_string();
        return ev.apply_operation(tools, &op, None, BodyKind::Surface);
    }
    // Thin: the wall curves are offset into planar regions (see `thin`),
    // which are swept like solid profiles.
    let thin = body_type == "THIN";
    let refs = ev.query(if thin { "wallShape" } else { "profiles" });
    if refs.is_empty() {
        return crate::regen::eval::err_at(&[if thin { "wallShape" } else { "profiles" }], "Select faces or sketch regions to sweep.");
    }
    let path_open = !path.is_closed().unwrap_or(false);
    // Live "psb (10-24 c)": a wall that can't be made fails with Onshape's
    // trim text when Trim ends applies, else its thicken text.
    let thin_fail = if path_open && ev.bool("trimEnds") { THIN_TRIM_FAILED } else { THIN_THICKEN_FAILED };
    let along = |f: &crate::display::PlaneFrame, m: DVec3| path_end_near(&path, m).map_or(f.normal, |(_, t)| t);
    let profiles = if thin { super::thin::thin_profiles_sided(ev, refs, super::thin::WallSide::Along(&along)).map_err(|_| fe(thin_fail))? } else { ev.profiles(refs)? };
    if profiles.is_empty() {
        return err("No valid profiles to sweep");
    }
    // sweep.fs thinSweep: Onshape sweeps the wall as a sheet, thickens it
    // (normal to the sheet) and trims the ends to the cap planes. Swept
    // walls offset in their plane are the same while the path leaves the
    // wall square and nothing scales or twists; otherwise build it that way.
    if thin && lock_faces.is_none() && (opts.scale_end != 1.0 || opts.twist != 0.0 || profiles.iter().any(|p| oblique_to(&path, p))) {
        let walls = super::thin::wall_wires(ev, refs)?;
        let (t1, t2) = super::thin::thicknesses(ev)?;
        for (w, frame) in walls {
            let mut o = opts.clone();
            if keep {
                o.mode = k::SweepMode::Fixed { z: frame.normal, x: frame.x };
            } else if let Some(d) = lock_dir {
                o.mode = k::SweepMode::Binormal(d);
            }
            let eff = if extend { path.clone() } else { half_path(&path, frame.origin, frame.normal, path_opposite) };
            let shape = thin_sheet_sweep(&w, &frame, &eff, &o, t1, t2, path_open && ev.bool("trimEnds"), thin_fail)?;
            let names = name_new(ev, &shape);
            tools.push(NamedShape::new(shape, names));
        }
        let op = ev.enum_("operationType").to_string();
        let scope = super::extrude::scope(ev);
        return ev.apply_operation(tools, &op, scope, BodyKind::Solid);
    }
    for p in &profiles {
        if keep {
            opts.mode = k::SweepMode::Fixed { z: p.frame.normal, x: p.frame.x };
        } else if let Some(d) = lock_dir {
            opts.mode = k::SweepMode::Binormal(d);
        }
        let res = sweep(&p.shape, Some(&p.frame), &opts).map_err(|_| fe(SWEEP_FAILED))?;
        if res.shape.count(ShapeType::Solid) == 0 {
            return err(SWEEP_FAILED);
        }
        // OCCT sweeps a profile round a bend tighter than the profile into a
        // self-intersecting solid; Onshape refuses it.
        if !res.shape.is_valid() || res.shape.self_intersects() {
            return err(tight_bend_message(&p.shape, &path));
        }
        let names = sweep_names(ev, p, &res, true, true);
        tools.push(NamedShape::new(res.shape, names));
    }
    let op = ev.enum_("operationType").to_string();
    let scope = super::extrude::scope(ev);
    ev.apply_operation(tools, &op, scope, BodyKind::Solid)
}

/// sweep.fs THIN_SWEEP_THICKEN_FAILED / THIN_SWEEP_TRIM_FAILED.
const THIN_THICKEN_FAILED: &str = "Failed to thicken swept body, check thickness.";
const THIN_TRIM_FAILED: &str = "Failed to trim result body, try deselecting \"Trim ends\" option.";

/// Whether the path leaves the wall `p` other than square to its plane.
fn oblique_to(path: &Shape, p: &Profile) -> bool {
    let Some((_, t)) = path_end_near(path, p.shape.bounding_box(false).map(|b| b.center()).unwrap_or(p.frame.origin)) else { return false };
    t.normalize_or_zero().dot(p.frame.normal).abs() < 1.0 - 1e-9
}

/// The path end nearest `p` and the path's direction there, pointing into
/// the path.
fn path_end_near(path: &Shape, p: DVec3) -> Option<(DVec3, DVec3)> {
    let edges = path.edges();
    let mut best: Option<(f64, DVec3, DVec3)> = None;
    for e in &edges {
        let i = e.edge_info().ok()?;
        for (u, sign) in [(i.first_param, 1.0), (i.last_param, -1.0)] {
            let q = e.point_at(u).ok()?;
            let free = edges.iter().filter(|o| o.endpoints().is_ok_and(|(a, b)| a.distance(q) < 1e-9 || b.distance(q) < 1e-9)).count() == 1;
            if !free && edges.len() > 1 {
                continue;
            }
            let t = e.tangent_at(u).ok()?.normalize_or_zero() * sign;
            if best.is_none_or(|b| q.distance(p) < b.0) {
                best = Some((q.distance(p), q, t));
            }
        }
    }
    best.map(|(_, q, t)| (q, t))
}

/// sweep.fs thinSweep for one wall: the wall `w` swept along `path` as a
/// sheet, thickened `t1` outside a closed wall / right of an open one (seen
/// along the path) and `t2` on the other side, the thickness normal to the
/// sheet (live "psb (10-24 c)": Scale 2 keeps the wall 2 thick across the
/// cone). With `trim` the ends are cut to the cap planes (the wall's plane
/// at the start; at the end the end cap's plane, or for straight walls the
/// wall's plane turned as the path turns): the sheet is swept past both ends
/// along the path's end tangents (exactly, scale and twist included, on a
/// straight path), thickened and cut.
#[allow(clippy::too_many_arguments)]
fn thin_sheet_sweep(w: &Shape, frame: &crate::display::PlaneFrame, path: &Shape, opts: &k::SweepOptions, t1: f64, t2: f64, trim: bool, fail: &'static str) -> FResult<Shape> {
    let failed = |_| fe(fail);
    let sheet_opts = k::SweepOptions { solid: false, ..opts.clone() };
    let sheet = k::sweep(w, path, &sheet_opts).map_err(failed)?.shape;
    // A point on the wall, its direction and the path's at the wall.
    let e0 = w.edges().first().cloned().ok_or_else(|| fe(fail))?;
    let i0 = e0.edge_info()?;
    let um = (i0.first_param + i0.last_param) * 0.5;
    let (pm, tm) = (e0.point_at(um)?, e0.tangent_at(um)?.normalize_or_zero());
    let (p_start, ts) = path_end_near(path, pm).ok_or_else(|| fe(fail))?;
    // Side 1: right of the wall seen along the path; for a closed wall that
    // is turned to the outside.
    let mut s1 = tm.cross(ts).normalize_or_zero();
    if w.is_closed().unwrap_or(false) {
        let size = w.bounding_box(false).map(|b| (b.max - b.min).length()).unwrap_or(1.0);
        let inplane = (s1 - frame.normal * s1.dot(frame.normal)).normalize_or_zero();
        let face = k::make_face_from_wire(w, true).map_err(failed)?;
        let probe = k::make_vertex(pm + inplane * size * 1e-4)?;
        if k::distance(&probe, &face).is_ok_and(|d| d.distance < size * 1e-9) {
            s1 = -s1;
        }
    }
    let side = |sheet: &Shape| -> f64 {
        let n = sheet
            .faces()
            .iter()
            .filter_map(|f| f.project_point(pm).ok().map(|pr| (f.clone(), pr)))
            .min_by(|a, b| a.1.distance.total_cmp(&b.1.distance))
            .and_then(|(f, pr)| f.normal_at_uv(pr.u, pr.v).ok());
        if n.is_some_and(|n| n.dot(s1) < 0.0) { -1.0 } else { 1.0 }
    };
    let thicken = |sheet: &Shape| -> FResult<Shape> {
        let (a, b) = if side(sheet) > 0.0 { (t1, t2) } else { (t2, t1) };
        Ok(k::thicken(sheet, a, b).map_err(failed)?.shape)
    };
    if !trim {
        return thicken(&sheet);
    }
    // The cap planes. Start: the wall's plane, facing along the path.
    let ns = if frame.normal.dot(ts) < 0.0 { -frame.normal } else { frame.normal };
    let turn = glam::DQuat::from_rotation_arc(ts, ns);
    let start_plane = (pm - ns * (pm - frame.origin).dot(ns), ns);
    // End: the sheet's free edges wholly away from the start plane (a
    // closed wall's seam runs from it to the end and isn't one).
    let ef = sheet.edge_faces()?;
    let mut far: Vec<DVec3> = Vec::new();
    for (i, e) in sheet.edges().iter().enumerate() {
        if ef.get(i).is_some_and(|f| f.len() == 1)
            && let Ok(info) = e.edge_info()
        {
            let pts: Vec<DVec3> = (0..=8).map(|k| e.point_at(info.first_param + (info.last_param - info.first_param) * k as f64 / 8.0)).collect::<k::Result<_>>()?;
            if pts.iter().all(|q| (*q - start_plane.0).dot(ns).abs() > 1e-7) {
                far.extend(pts);
            }
        }
    }
    let (_, te) = path_end_far(path, p_start).ok_or_else(|| fe(fail))?;
    let end_plane = match fit_plane(&far) {
        Some((o, n)) => (o, if n.dot(te) < 0.0 { -n } else { n }),
        None => (far.iter().copied().sum::<DVec3>() / far.len().max(1) as f64, turn * te),
    };
    // Sweep past both ends and thicken.
    let reach = 10.0 * (t1 + t2) + 1e-3;
    let edges = path.edges();
    let len: f64 = edges.iter().filter_map(|e| e.length().ok()).sum();
    let straight = edges.len() == 1 && matches!(edges[0].edge_curve(), Ok(k::Curve::Line { .. }));
    let ext = if straight && opts.twist == 0.0 {
        // A straight, untwisted sweep is ruled between the wall and its
        // scaled copy at the end: rule it between the copies past both ends.
        let s = opts.scale_end;
        let at = |d: f64| -> FResult<Shape> {
            let moved = k::transform(w, &k::Transform::Translate(ts * d), true)?.shape;
            let f = 1.0 + (s - 1.0) * d / len;
            Ok(if (f - 1.0).abs() > 1e-12 { k::transform(&moved, &k::Transform::Scale { center: p_start + ts * d, factor: f }, true)?.shape } else { moved })
        };
        let ends = [at(-reach)?, at(len + reach)?];
        k::loft(&ends, &k::LoftOptions { ruled: true, ..Default::default() }).ok().map(|r| r.shape)
    } else if straight {
        // One longer line; the scale and twist laws carried on linearly.
        let s = opts.scale_end;
        let s0 = 1.0 - (s - 1.0) * reach / len;
        let line = k::make_line(p_start - ts * reach, p_start + ts * (len + reach))?;
        let moved = k::transform(w, &k::Transform::Translate(-ts * reach), true)?.shape;
        let scaled = if (s0 - 1.0).abs() > 1e-12 { k::transform(&moved, &k::Transform::Scale { center: p_start - ts * reach, factor: s0 }, true)?.shape } else { moved };
        let turned = if opts.twist != 0.0 {
            k::transform(&scaled, &k::Transform::Rotate { origin: p_start, axis: ts, angle: -opts.twist * reach / len }, true)?.shape
        } else {
            scaled
        };
        let o = k::SweepOptions { solid: false, scale_end: (s + (s - 1.0) * reach / len) / s0, twist: opts.twist * (len + 2.0 * reach) / len, ..opts.clone() };
        k::make_wire(&[line]).ok().and_then(|l| k::sweep(&turned, &l, &o).ok()).map(|r| r.shape)
    } else if opts.scale_end == 1.0 && opts.twist == 0.0 {
        let (q, tq) = path_end_far(path, p_start).ok_or_else(|| fe(fail))?;
        let mut all = vec![k::make_line(p_start - ts * reach, p_start)?];
        all.extend(edges.iter().cloned());
        all.push(k::make_line(q, q + tq * reach)?);
        let moved = k::transform(w, &k::Transform::Translate(-ts * reach), true)?.shape;
        k::make_wire(&all).ok().and_then(|l| k::sweep(&moved, &l, &sheet_opts).ok()).map(|r| r.shape)
    } else {
        None
    };
    let thick = thicken(ext.as_ref().unwrap_or(&sheet)).map_err(|_| fe(THIN_TRIM_FAILED))?;
    // Keep what lies between the two planes.
    let mut body = thick;
    for (o, n, keep_above) in [(start_plane.0, start_plane.1, true), (end_plane.0, end_plane.1, false)] {
        let x = if n.cross(DVec3::X).length() > 0.1 { DVec3::X } else { DVec3::Y };
        let plane = k::Plane::new(o, n, n.cross(x).normalize());
        let tol = 1e-7 * (1.0 + len);
        let pieces = match k::split_by_plane(&body, &plane) {
            Ok(r) => r.shape.solids(),
            Err(_) => vec![body.clone()],
        };
        let kept: Vec<Shape> = pieces.into_iter().filter(|s| s.centroid().is_ok_and(|c| ((c - o).dot(n) > tol) == keep_above)).collect();
        body = match kept.as_slice() {
            [] => return err(THIN_TRIM_FAILED),
            [one] => one.clone(),
            many => Shape::compound(many),
        };
    }
    Ok(body)
}

/// The path end away from `start` and the path's direction there, pointing
/// out of the path.
fn path_end_far(path: &Shape, start: DVec3) -> Option<(DVec3, DVec3)> {
    let edges = path.edges();
    let mut best: Option<(f64, DVec3, DVec3)> = None;
    for e in &edges {
        let i = e.edge_info().ok()?;
        for (u, sign) in [(i.first_param, -1.0), (i.last_param, 1.0)] {
            let q = e.point_at(u).ok()?;
            let free = edges.iter().filter(|o| o.endpoints().is_ok_and(|(a, b)| a.distance(q) < 1e-9 || b.distance(q) < 1e-9)).count() == 1;
            if !free && edges.len() > 1 {
                continue;
            }
            let t = e.tangent_at(u).ok()?.normalize_or_zero() * sign;
            if best.is_none_or(|b| q.distance(start) > b.0) {
                best = Some((q.distance(start), q, t));
            }
        }
    }
    best.map(|(_, q, t)| (q, t))
}

/// The plane through `pts` if they span one (not all on a line).
fn fit_plane(pts: &[DVec3]) -> Option<(DVec3, DVec3)> {
    let o = *pts.first()?;
    let far = pts.iter().copied().max_by(|a, b| a.distance(o).total_cmp(&b.distance(o)))?;
    let d = (far - o).try_normalize()?;
    let third = pts.iter().copied().max_by(|a, b| (*a - o).cross(d).length().total_cmp(&(*b - o).cross(d).length()))?;
    let n = d.cross(third - o).try_normalize()?;
    let size = far.distance(o).max(1e-9);
    if (third - o).cross(d).length() < size * 1e-6 || pts.iter().any(|p| (*p - o).dot(n).abs() > size * 1e-7) {
        return None;
    }
    Some((o, n))
}

/// Fixed trihedron for a wire profile: its plane if planar, else world Z.
fn fixed_mode_for(wire: &Shape) -> FResult<k::SweepMode> {
    match k::make_face_from_wire(wire, true).ok().and_then(|f| f.face_plane().ok().flatten()) {
        Some(pl) => Ok(k::SweepMode::Fixed { z: pl.unit_normal(), x: pl.unit_x() }),
        None => Ok(k::SweepMode::Fixed { z: DVec3::Z, x: DVec3::X }),
    }
}

// ------------------------------------------------------------------ loft

/// One loft section: a shape and (for faces/wires) its profile naming.
struct Section {
    shape: Shape,
    profile: Option<Profile>,
}

fn loft_sections(ev: &EvalCtx, items: &[ParamValues], query_id: &str, wire: bool) -> FResult<Vec<Section>> {
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let refs = item.query(query_id);
        if refs.is_empty() {
            continue;
        }
        // Point profile.
        if (refs.len() == 1 && matches!(refs[0], Reference::Vertex(_) | Reference::Origin | Reference::MateConnector { .. } | Reference::Implicit { .. }))
            || matches!(&refs[0], Reference::SketchEntity { sketch, entity } if ev.ctx.sketch(*sketch).is_some_and(|s| s.sketch.point(noshape_sketch::EntityId(*entity)).is_some()))
        {
            let p = ev.resolve_vertex_point(&refs[0]).ok_or_else(|| fe("Could not create valid profiles from selections."))?;
            out.push(Section { shape: k::make_vertex(p)?, profile: None });
            continue;
        }
        if wire {
            let edges = resolve_curves(ev, refs)?;
            let mut wires = wires_from(&edges)?;
            // Faces in a wire profile: use their outer boundary.
            for (bi, fi) in ev.resolve_faces(refs) {
                let f = ev.face_shape(bi, fi)?;
                if let Some(w) = f.wires().first() {
                    wires.push(w.clone());
                }
            }
            if wires.len() != 1 {
                return err("Could not create valid profiles from selections.");
            }
            let w = wires.remove(0);
            out.push(Section { shape: w.clone(), profile: Some(wire_profile(w, &edges, Source::Index(i as u32))) });
            continue;
        }
        let mut profiles = ev.profiles(refs)?;
        // Sheet bodies with a single face.
        for r in refs {
            if let Reference::Body(id) = r
                && let Some(b) = ev.ctx.body(id)
                && b.kind == BodyKind::Surface
            {
                let faces = b.shape.faces();
                if faces.len() != 1 {
                    return err("Select a single sketch region or face per profile.");
                }
                let f = faces[0].clone();
                let edge_sources = f.edges().iter().map(|e| b.shape.index_of(e, ShapeType::Edge).map(|k| Source::Topo(b.edges[k].clone())).unwrap_or(Source::Index(0))).collect();
                let frame = match f.face_plane()? {
                    Some(pl) => crate::display::PlaneFrame { origin: pl.origin, x: pl.unit_x(), y: pl.unit_y(), normal: pl.unit_normal() },
                    None => crate::display::PlaneFrame::TOP,
                };
                profiles.push(Profile { shape: f, frame, edge_sources, key: Source::Topo(b.faces[0].clone()), from_face: true });
            }
        }
        if profiles.len() != 1 {
            return err("Select a single sketch region or face per profile.");
        }
        let p = profiles.remove(0);
        if p.shape.wires().len() > 1 {
            return err("Cannot use faces or regions with inner loops as profiles.");
        }
        out.push(Section { shape: p.shape.clone(), profile: Some(p) });
    }
    Ok(out)
}

/// Two circles in parallel planes whose centres are apart sideways by at least
/// the larger radius: Parasolid's loft between them twists through itself
/// (live L3: Ø20 circles 30 apart OK up to dx 9.999, failing from dx 10; a Ø10
/// top at dx 30 fails too).
fn oblique_circles(shapes: &[Shape]) -> bool {
    let circle = |s: &Shape| match s.edges().as_slice() {
        [e] => match e.edge_curve() {
            Ok(k::Curve::Circle { center, axis, radius, .. }) => Some((center, axis.normalize(), radius)),
            _ => None,
        },
        _ => None,
    };
    let [a, b] = shapes else { return false };
    let (Some((c1, n1, r1)), Some((c2, n2, r2))) = (circle(a), circle(b)) else { return false };
    if n1.cross(n2).length() > 1e-9 {
        return false;
    }
    let d = c2 - c1;
    let side = (d - n1 * d.dot(n1)).length();
    side >= r1.max(r2) * (1.0 - 1e-9)
}

fn loft_names(ev: &EvalCtx, sections: &[Section], res: &k::OpResult, caps: bool) -> Vec<Name> {
    let mut roles = BTreeMap::new();
    if caps {
        if let Some(p) = sections.first().and_then(|s| s.profile.as_ref()) {
            for f in res.history.first_faces() {
                roles.insert(f, Role::CapStart(Some(p.key.clone())));
            }
        }
        if let Some(p) = sections.last().and_then(|s| s.profile.as_ref()) {
            for f in res.history.last_faces() {
                roles.insert(f, Role::CapEnd(Some(p.key.clone())));
            }
        }
    }
    let mut recs: Vec<&k::HistoryRecord> = res.history.records.iter().filter(|r| r.input_kind == ShapeType::Edge && r.relation == k::Relation::Generated && r.output_kind == ShapeType::Face).collect();
    recs.sort_by_key(|r| (r.arg, r.input_index));
    for r in recs {
        if let Some(src) = sections.get(r.arg as usize).and_then(|s| s.profile.as_ref()).and_then(|p| p.edge_sources.get(r.input_index as usize)) {
            roles.entry(r.output_index).or_insert_with(|| Role::Side(src.clone()));
        }
    }
    let empty = crate::naming::ShapeNames::default();
    ev.name_result(&[&empty], &res.history, &res.shape, &roles, None)
}

pub fn loft(ev: &mut EvalCtx) -> FResult<()> {
    let body_type = ev.enum_("bodyType").to_string();
    if ev.bool("addGuides") && ev.params.array("guidesArray").iter().any(|g| !g.query("guideEntities").is_empty()) {
        return err("Loft guides are not supported yet");
    }
    if ev.bool("addSections") && !ev.query("spine").is_empty() {
        return err("Loft path is not supported yet");
    }
    let solid = body_type == "SOLID";
    let sections = if solid {
        loft_sections(ev, ev.params.array("sheetProfilesArray"), "sheetProfileEntities", false)?
    } else {
        loft_sections(ev, ev.params.array("wireProfilesArray"), "wireProfileEntities", true)?
    };
    // Live L15: no profile marks the field; one profile has its own text.
    if sections.is_empty() {
        return crate::regen::eval::err_at(&["sheetProfilesArray", "wireProfilesArray"], "Select profiles in order from start to finish.");
    }
    if sections.len() < 2 {
        return err("Select at least two profiles, a closed guide curve or a closed path.");
    }
    if sections[1..sections.len() - 1].iter().any(|s| s.profile.is_none()) {
        return err("Point profiles can only exist as first or last profiles.");
    }
    let shapes: Vec<Shape> = sections.iter().map(|s| s.shape.clone()).collect();
    let mut opts = k::LoftOptions { solid, closed: ev.bool("makePeriodic"), ..Default::default() };
    // loft.fs createProfileConditions ("End conditions" is only a group
    // header: the conditions apply whether it's open or not).
    let vals = ev.params;
    let normal = |s: &Section| s.profile.as_ref().map(|p| p.frame.normal);
    let (first, last) = (&sections[0], &sections[sections.len() - 1]);
    opts.start = loft_end(ev, vals, ["startCondition", "startMagnitude", "startDirection"], &first.shape, normal(first), "start")?;
    opts.end = loft_end(ev, vals, ["endCondition", "endMagnitude", "endDirection"], &last.shape, normal(last), "end")?;
    // Onshape's LOFT_CONNECTION_MATCHING: every connection needs a point on
    // each profile (point profiles excepted).
    if ev.bool("matchConnections") {
        for c in ev.params.array("connections") {
            let pts: Vec<DVec3> = c.query("connectionEntities").iter().filter_map(|r| ev.resolve_vertex_point(r).or_else(|| edge_ref_mid(ev, r))).collect();
            let on = |s: &Shape| pts.iter().any(|p| k::make_vertex(*p).and_then(|v| k::distance(&v, s)).is_ok_and(|d| d.distance < 1e-6));
            if shapes.iter().any(|s| s.shape_type() != ShapeType::Vertex && !on(s)) {
                return crate::regen::eval::err_at(&["connections"], "Select a point on each profile to define a connection.");
            }
        }
    }
    // Match connections: each connection's point on each profile. One sets
    // where the profiles start; several split the profiles and are lofted
    // connection to connection.
    if ev.bool("matchConnections") {
        let mut all = Vec::new();
        for c in ev.params.array("connections") {
            let pts: Vec<DVec3> = c.query("connectionEntities").iter().filter_map(|r| ev.resolve_vertex_point(r).or_else(|| edge_ref_mid(ev, r))).collect();
            if pts.is_empty() {
                continue;
            }
            let mut on = vec![DVec3::NAN; shapes.len()];
            for (si, s) in shapes.iter().enumerate() {
                if s.shape_type() == ShapeType::Vertex {
                    continue;
                }
                let best = pts.iter().filter_map(|p| k::make_vertex(*p).ok().and_then(|v| k::distance(&v, s).ok()).map(|d| (*p, d.distance))).min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((p, d)) = best
                    && d < 1e-6
                {
                    on[si] = p;
                }
            }
            all.push(on);
        }
        if all.len() == 1 {
            opts.start_points = all.pop();
        } else {
            opts.connections = all;
        }
    }
    if oblique_circles(&shapes) {
        return err("Current selections would create a self-intersecting body.");
    }
    let res = k::loft(&shapes, &opts).map_err(|_| fe(LOFT_FAILED))?;
    if solid && res.shape.count(ShapeType::Solid) == 0 {
        return err(LOFT_FAILED);
    }
    let names = loft_names(ev, &sections, &res, solid);
    if body_type == "THIN" {
        // Thin loft: the lofted surface (through the wire profiles) is
        // thickened by thickness 1 / 2 (Flip wall swaps the sides).
        let (t1, t2) = super::thin::thicknesses(ev)?;
        let inp = ThickenInput {
            shape: res.shape.clone(),
            face_sources: names.iter().map(|n| Source::Topo(n.clone())).collect(),
            edge_sources: crate::regen::eval::names_for(&res.shape, names.clone()).edges.iter().map(|n| Source::Topo(n.clone())).collect(),
            group: None,
        };
        let th = k::thicken(&res.shape, t1, t2).map_err(|_| fe("Failed to thicken lofted body, check thickness."))?;
        if th.shape.count(ShapeType::Solid) == 0 {
            return err("Thin loft did not produce a solid");
        }
        let names = thicken_names(ev, &inp, &th.shape, t1.max(t2));
        let tools = vec![NamedShape::new(th.shape, names)];
        let op = ev.enum_("operationType").to_string();
        let scope = super::extrude::scope(ev);
        return ev.apply_operation(tools, &op, scope, BodyKind::Solid);
    }
    let tools = vec![NamedShape::new(res.shape, names)];
    if solid {
        let op = ev.enum_("operationType").to_string();
        let scope = super::extrude::scope(ev);
        ev.apply_operation(tools, &op, scope, BodyKind::Solid)
    } else {
        let op = ev.enum_("surfaceOperationType").to_string();
        ev.apply_operation(tools, &op, None, BodyKind::Surface)
    }
}

fn edge_ref_mid(ev: &EvalCtx, r: &Reference) -> Option<DVec3> {
    let (bi, ei) = *ev.resolve_edges(std::slice::from_ref(r)).first()?;
    edge_mid(&ev.edge_shape(bi, ei).ok()?)
}

// ------------------------------------------------------------------ thicken

/// A face set to thicken with the persistent sources of its faces/edges.
struct ThickenInput {
    shape: Shape,
    face_sources: Vec<Source>,
    edge_sources: Vec<Source>,
    /// Faces of one body picked together: their slabs become one part.
    group: Option<usize>,
}

pub fn thicken(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("entities");
    if refs.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Select surfaces or faces to thicken.");
    }
    let (mut t1, mut t2) = if ev.bool("midplane") {
        let t = ev.length("thickness")? * 0.5;
        (t, t)
    } else {
        (ev.length("thickness1")?, ev.length("thickness2")?)
    };
    if ev.bool("oppositeDirection") && !ev.bool("midplane") {
        std::mem::swap(&mut t1, &mut t2);
    }
    // Live L19: both thicknesses 0 fail with this text and no field.
    if t1 + t2 <= 0.0 {
        return err("Invalid input selections.");
    }
    let mut inputs: Vec<ThickenInput> = Vec::new();
    let mut consumed: Vec<BodyId> = Vec::new();
    // Faces picked per body: one body's faces are thickened together (live
    // L17: two adjacent faces give one part), and a surface is used up only
    // when all of its faces are (L16).
    let mut picked: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for r in refs {
        match r {
            Reference::SketchRegion { .. } => {
                for p in ev.profiles(std::slice::from_ref(r))? {
                    inputs.push(ThickenInput { face_sources: vec![p.key.clone()], edge_sources: p.edge_sources.clone(), shape: p.shape, group: None });
                }
            }
            Reference::Face(_) => {
                for (bi, fi) in ev.resolve_faces(std::slice::from_ref(r)) {
                    let v = picked.entry(bi).or_default();
                    if !v.contains(&fi) {
                        v.push(fi);
                    }
                }
            }
            Reference::Body(id) => {
                if let Some(b) = ev.ctx.body(id)
                    && b.kind == BodyKind::Surface
                {
                    consumed.push(b.id.clone());
                    inputs.push(ThickenInput {
                        shape: b.shape.clone(),
                        face_sources: b.faces.iter().map(|x| Source::Topo(x.clone())).collect(),
                        edge_sources: b.edges.iter().map(|x| Source::Topo(x.clone())).collect(),
                        group: None,
                    });
                }
            }
            _ => {}
        }
    }
    for (bi, fis) in picked {
        let b = &ev.ctx.bodies[bi];
        let faces: Vec<Shape> = fis.iter().map(|&fi| b.shape.sub_shape(ShapeType::Face, fi)).collect::<k::Result<_>>()?;
        if b.kind == BodyKind::Surface && fis.len() >= b.shape.faces().len() {
            consumed.push(b.id.clone());
        }
        // Several faces of one body make one part (live L17). Each face is
        // thickened and the slabs are fused; Onshape's square corner fill
        // between them is still missing (kernel).
        let group = (fis.len() > 1).then_some(bi);
        for f in faces {
            let n = sub_names(b, &f);
            inputs.push(ThickenInput {
                face_sources: n.faces.iter().map(|x| Source::Topo(x.clone())).collect(),
                edge_sources: n.edges.iter().map(|x| Source::Topo(x.clone())).collect(),
                shape: f,
                group,
            });
        }
    }
    if inputs.is_empty() {
        return err("Select surfaces or faces to thicken.");
    }
    let mut tools: Vec<NamedShape> = Vec::new();
    let mut grouped: BTreeMap<usize, usize> = BTreeMap::new();
    for inp in &inputs {
        let res = k::thicken(&inp.shape, t1, t2).map_err(kerr("Thicken failed"))?;
        if res.shape.count(ShapeType::Solid) == 0 {
            return err("Thicken did not produce a solid");
        }
        let shape = res.shape;
        let names = thicken_names(ev, inp, &shape, t1.max(t2));
        let slab = NamedShape::new(shape, names);
        match inp.group.and_then(|g| grouped.get(&g).copied()) {
            Some(ti) => {
                let a = tools[ti].clone();
                let res = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(&a.shape), std::slice::from_ref(&slab.shape), &k::BooleanOptions::default())?;
                let names = ev.name_result(&[&a.names, &slab.names], &res.history, &res.shape, &BTreeMap::new(), None);
                tools[ti] = NamedShape::new(res.shape, names);
            }
            None => {
                if let Some(g) = inp.group {
                    grouped.insert(g, tools.len());
                }
                tools.push(slab);
            }
        }
    }
    let op = ev.enum_("operationType").to_string();
    let scope = scope_ids(ev);
    if !ev.bool("keepTools") {
        consumed.sort();
        consumed.dedup();
        remove_bodies(ev, &consumed);
    }
    let scope = ids_to_indices(ev, &scope);
    ev.apply_operation(tools, &op, scope, BodyKind::Solid)
}

/// Geometric naming for thicken: faces on the input are start caps, faces
/// parallel to it are end caps, the rest are sides of the nearest input edge.
fn thicken_names(ev: &EvalCtx, inp: &ThickenInput, out: &Shape, tmax: f64) -> Vec<Name> {
    let in_faces = inp.shape.faces();
    let in_edges = inp.shape.edges();
    let tol = 1e-7_f64.max(tmax * 1e-6);
    let mut roles = BTreeMap::new();
    for (j, f) in out.faces().iter().enumerate() {
        let Some(p) = point_on_face(f) else { continue };
        let Ok(v) = k::make_vertex(p) else { continue };
        let nearest = |shapes: &[Shape]| shapes.iter().enumerate().filter_map(|(i, s)| k::distance(&v, s).ok().map(|d| (i, d.distance))).min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((fi, d)) = nearest(&in_faces) else { continue };
        let src_f = inp.face_sources.get(fi).cloned().unwrap_or(Source::Index(fi as u32));
        if d < tol {
            roles.insert(j as u32, Role::CapStart(Some(src_f)));
            continue;
        }
        let n_out = f.normal_at_point(p).unwrap_or(DVec3::Z);
        let n_in = face_normal_near(&in_faces[fi], p).map(|x| x.1).unwrap_or(DVec3::Z);
        if n_out.dot(n_in).abs() > 0.5 {
            roles.insert(j as u32, Role::CapEnd(Some(src_f)));
        } else if let Some((ei, _)) = nearest(&in_edges) {
            roles.insert(j as u32, Role::Side(inp.edge_sources.get(ei).cloned().unwrap_or(Source::Index(ei as u32))));
        }
    }
    let empty = crate::naming::ShapeNames::default();
    let names = ev.name_result(&[&empty], &k::History::default(), out, &roles, None);
    dedup_names(ev, names)
}

// ------------------------------------------------------------------ enclose

pub fn enclose(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("entities");
    // Live L29: the empty field is marked.
    if refs.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Select faces, parts, or planes to form a solid.");
    }
    let mut shapes: Vec<Shape> = Vec::new();
    let mut names: Vec<crate::naming::ShapeNames> = Vec::new();
    let mut owners: Vec<BodyId> = Vec::new();
    for r in refs {
        match r {
            Reference::Body(id) => {
                if let Some(b) = ev.ctx.body(id) {
                    shapes.push(b.shape.clone());
                    names.push(b.shape_names());
                    owners.push(b.id.clone());
                }
            }
            Reference::Face(_) => {
                for (bi, fi) in ev.resolve_faces(std::slice::from_ref(r)) {
                    let b = &ev.ctx.bodies[bi];
                    let f = b.shape.sub_shape(ShapeType::Face, fi)?;
                    names.push(sub_names(b, &f));
                    shapes.push(f);
                    owners.push(b.id.clone());
                }
            }
            // Live L31: sketch regions can be picked.
            Reference::SketchRegion { .. } => {
                for p in ev.profiles(std::slice::from_ref(r))? {
                    shapes.push(p.shape);
                    names.push(Default::default());
                }
            }
            _ => {
                if let Some(frame) = ev.resolve_plane(r) {
                    shapes.push(big_plane_face(ev, &kernel_plane(&frame))?);
                    names.push(Default::default());
                }
            }
        }
    }
    if shapes.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Select faces, parts, or planes to form a solid.");
    }
    let mut res = k::enclose(&shapes).map_err(|_| fe("Failed to create a part from enclosed region."))?;
    if res.shape.count(ShapeType::Solid) == 0 {
        return err("Selections do not enclose a region.");
    }
    // Live L31: every enclosed region goes into one part.
    let solids = res.shape.solids();
    if solids.len() > 1 {
        let fused = k::boolean(k::BooleanOp::Fuse, &solids[..1], &solids[1..], &k::BooleanOptions::default()).map_err(|_| fe("Failed to create a part from enclosed region."))?;
        let un = k::unify_same_domain(&fused.shape)?;
        res = k::OpResult { shape: un.shape, history: res.history };
    }
    let arg_refs: Vec<&crate::naming::ShapeNames> = names.iter().collect();
    let faces = ev.name_result(&arg_refs, &res.history, &res.shape, &BTreeMap::new(), None);
    let faces = rebrand(ev, faces);
    let tools = vec![NamedShape::new(res.shape, faces)];
    let op = ev.enum_("operationType").to_string();
    let scope = scope_ids(ev);
    if !ev.bool("keepTools") {
        owners.sort();
        owners.dedup();
        remove_bodies(ev, &owners);
    }
    let scope = ids_to_indices(ev, &scope);
    ev.apply_operation(tools, &op, scope, BodyKind::Solid)
}

// ------------------------------------------------------------------ rib

pub fn rib(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("profiles");
    if refs.is_empty() {
        return crate::regen::eval::err_at(&["profiles"], "Select sketch profiles to create the ribs.");
    }
    // rib.fs applyDraft: the rib's sides drafted about its top edges, pulled
    // back toward the profile (wider into the part) unless opposite.
    let draft = if ev.bool("hasDraft") { Some((ev.angle("draftAngle")?, ev.bool("draftOpposite"))) } else { None };
    let t = ev.length("thickness")?;
    if t <= 0.0 {
        return err("Rib thickness must be positive");
    }
    let frame = refs
        .iter()
        .find_map(|r| match r {
            Reference::SketchEntity { sketch, .. } | Reference::Sketch(sketch) => ev.ctx.sketch(*sketch).map(|s| s.frame),
            _ => None,
        })
        .ok_or_else(|| fe("Rib profiles must be sketch curves"))?;
    let edges = resolve_curves(ev, refs)?;
    let wires = wires_from(&edges)?;
    if wires.is_empty() {
        return err("The rib profiles no longer exist");
    }
    let normal = ev.is("ribExtrusionDirection", "NORMAL_TO_SKETCH_PLANE");
    // Live L23/L24: parallel to the sketch plane a closed profile or a chain
    // of several edges fails on the profiles.
    if !normal && wires.iter().any(|w| w.is_closed().unwrap_or(false) || w.edges().len() > 1) {
        return crate::regen::eval::err_at(&["profiles"], RIB_FAILED);
    }
    let dir = if ev.bool("oppositeDirection") { -frame.normal } else { frame.normal };
    let mut parts = ev.resolve_bodies(ev.query("parts"));
    parts.retain(|i| ev.ctx.bodies[*i].kind == BodyKind::Solid);
    // Live L22: no Parts is an error, no field marked (the dialog fills
    // Parts when it opens). Ribs saved before that used every part.
    if parts.is_empty() && ev.params.bool("legacyAllParts") {
        parts = (0..ev.ctx.bodies.len()).filter(|i| ev.ctx.bodies[*i].kind == BodyKind::Solid).collect();
    }
    if parts.is_empty() {
        return err("Select parts where the rib will be fitted into.");
    }
    let merge = ev.bool("mergeRibs");
    let (opposite, extend) = (ev.bool("oppositeDirection"), ev.bool("extendProfilesUpToPart"));
    let mut made = 0;
    let mut last_err = None;
    let mut new_ribs: Vec<NamedShape> = Vec::new();
    for bi in parts.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let mut shape = body.shape.clone();
        for w in &wires {
            let res = if normal && w.edges().len() > 1 {
                rib_normal(&shape, w, dir, t, draft)
            } else if normal {
                rib_sheet(&shape, w, dir, None, t, extend, draft)
            } else {
                rib_parallel(&shape, w, frame.normal, opposite, t, extend, draft)
            };
            match res {
                Ok(r) => {
                    shape = r.shape;
                    made += 1;
                }
                Err(e) => last_err = Some(e),
            }
        }
        if shape.is_same(&body.shape) {
            continue;
        }
        if merge {
            let res = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(&body.shape), std::slice::from_ref(&shape), &k::BooleanOptions::default()).map_err(|_| fe("Failed to merge ribs into parts."))?;
            let empty = crate::naming::ShapeNames::default();
            let names = ev.name_result(&[&body.shape_names(), &empty], &res.history, &res.shape, &BTreeMap::new(), None);
            ev.replace_body(bi, res.shape, names);
        } else {
            let res = k::cut(&shape, &body.shape)?;
            let names = name_new(ev, &res.shape);
            new_ribs.push(NamedShape::new(res.shape, names));
        }
    }
    if made == 0 {
        // Onshape: RIB_PROFILE_FAILED when a profile failed, else RIB_NO_INTERSECTIONS.
        return Err(last_err.filter(|e| e.message != RIB_MISS).unwrap_or_else(|| fe(RIB_MISS)));
    }
    if !new_ribs.is_empty() {
        for (i, t) in new_ribs.into_iter().enumerate() {
            for (j, p) in crate::regen::eval::split_solids(&t.shape, &t.names.faces).into_iter().enumerate() {
                push_body(ev, BodyId { feature: ev.feature, index: (i * 64 + j) as u32 }, BodyKind::Solid, p.shape, p.names.faces);
            }
        }
    }
    Ok(())
}

/// Onshape's RIB_PROFILE_FAILED / RIB_NO_INTERSECTIONS.
const RIB_FAILED: &str = "Failed to create a rib from a selected profile.";
const RIB_MISS: &str = "None of the ribs intersected a part.";

/// Rib parallel to the sketch plane as rib.fs builds it (live L22): the
/// profile, extended at an end when "Extend profiles to part" is on or when
/// the end sits on a part face it points into, is swept along the rib
/// direction past the part and thickened about the sketch plane; the part
/// splits it, and the pieces that touch the profile but not the far end of
/// the sweep or of an extension are the rib.
fn rib_parallel(base: &Shape, wire: &Shape, normal: DVec3, opposite: bool, t: f64, extend: bool, draft: Option<(f64, bool)>) -> FResult<k::OpResult> {
    let edges = wire.edges();
    let [e] = edges.as_slice() else { return err(RIB_FAILED) };
    let info = e.edge_info()?;
    let (p0, p1) = (e.point_at(info.first_param)?, e.point_at(info.last_param)?);
    let mut dir = normal.cross((p1 - p0).normalize_or_zero());
    if opposite {
        dir = -dir;
    }
    rib_sheet(base, wire, dir, Some(normal), t, extend, draft)
}

/// rib.fs's rib from one profile curve: the curve, extended at an end when
/// asked or when the end sits on a part face it points into, is extruded
/// along the rib direction `dir` past the part and thickened (about the
/// sketch plane `plane` for a parallel rib, about the surface for a rib
/// normal to the sketch); the part splits it, and the pieces that touch the
/// profile but not the far end of the sweep or of an extension are the rib.
fn rib_sheet(base: &Shape, wire: &Shape, dir: DVec3, plane: Option<DVec3>, t: f64, extend: bool, draft: Option<(f64, bool)>) -> FResult<k::OpResult> {
    let edges = wire.edges();
    let [e] = edges.as_slice() else { return err(RIB_FAILED) };
    let info = e.edge_info()?;
    let (p0, p1) = (e.point_at(info.first_param)?, e.point_at(info.last_param)?);
    let ends = [(p0, -e.tangent_at(info.first_param)?.normalize_or_zero()), (p1, e.tangent_at(info.last_param)?.normalize_or_zero())];
    let (bb, pb) = (base.bounding_box(false)?, wire.bounding_box(false)?);
    // rib.fs calculateLength: the diagonal of the parts and profiles' box.
    let len = (bb.max.max(pb.max) - bb.min.min(pb.min)).length();
    let tol = 1e-7 * (1.0 + len);
    let near = |s: &Shape, p: DVec3| k::make_vertex(p).and_then(|v| k::distance(s, &v)).is_ok_and(|d| d.distance < tol);
    let (faces, part_edges) = (base.faces(), base.edges());
    // rib.fs endNeedsExtension. An end on an edge of the part (live's rim
    // line, L24's L) isn't extended.
    let needs = |p: DVec3, d: DVec3| !part_edges.iter().any(|pe| near(pe, p)) && faces.iter().any(|f| near(f, p) && f.normal_at_point(p).is_ok_and(|n| d.dot(n) < -1e-9));
    let mut curves = vec![e.clone()];
    let mut tips = Vec::new();
    for (p, d) in ends {
        if extend || needs(p, d) {
            curves.push(k::make_line(p, p + d * len)?);
            tips.push(p + d * len);
        }
    }
    let mut slab: Option<Shape> = None;
    for c in &curves {
        let face = k::extrude(c, dir * len).map_err(|_| fe(RIB_FAILED))?.shape;
        let s = match plane {
            Some(normal) => k::extrude_symmetric(&face, normal * t),
            None => k::thicken(&face, t * 0.5, t * 0.5),
        }
        .map_err(|_| fe(RIB_FAILED))?
        .shape;
        slab = Some(match slab {
            None => s,
            Some(a) => k::boolean(k::BooleanOp::Fuse, &[a], &[s], &k::BooleanOptions::default()).map_err(|_| fe(RIB_FAILED))?.shape,
        });
    }
    let Some(slab) = slab else { return err(RIB_FAILED) };
    let pieces = k::cut(&slab, base)?.shape.solids();
    // rib.fs applyBoolean: the part must split the sweep. Live L22: with
    // extended ends that fails on the profile.
    if pieces.len() < 2 {
        return err(if tips.is_empty() { RIB_MISS } else { RIB_FAILED });
    }
    let far = p0 + dir * len;
    let mut rib: Vec<Shape> = pieces.into_iter().filter(|s| k::distance(s, e).is_ok_and(|d| d.distance < tol) && !near(s, far) && !tips.iter().any(|q| near(s, *q))).collect();
    if rib.is_empty() {
        return err(RIB_FAILED);
    }
    if let Some((angle, draft_opposite)) = draft {
        rib = draft_rib(base, &rib, e, |s| rib_sides(s, e, dir, tol), p0, dir, angle, draft_opposite, tol)?;
    }
    Ok(k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(base), &rib, &k::BooleanOptions::default())?)
}

/// A rib piece's thicken caps: the faces along the rib direction beside the
/// profile's interior (not its ends, nor the top and the part's side).
fn rib_sides(piece: &Shape, profile: &Shape, dir: DVec3, tol: f64) -> Vec<Shape> {
    let ends: Vec<DVec3> = profile.vertices().iter().filter_map(|v| v.vertex_point().ok()).collect();
    piece
        .faces()
        .into_iter()
        .filter(|f| {
            let Ok(c) = f.centroid() else { return false };
            let along = f.normal_at_point(c).is_ok_and(|n| n.dot(dir).abs() < 1e-9);
            let Ok(d) = k::make_vertex(c).and_then(|v| k::distance(&v, profile)) else { return false };
            along && !ends.iter().any(|p| (d.point_b - *p).length() < tol)
        })
        .collect()
}

/// rib.fs applyDraft: each rib piece's `caps` drafted by `angle` about the
/// plane through the profile normal to the rib direction `dir` (the rib's
/// top keeps its thickness), pulled along −dir (wider into the part) or
/// +dir when opposite; then split by the part again, keeping what touches
/// the profile (the draft can poke through the part elsewhere).
#[allow(clippy::too_many_arguments)]
fn draft_rib(base: &Shape, rib: &[Shape], profile: &Shape, caps: impl Fn(&Shape) -> Vec<Shape>, top: DVec3, dir: DVec3, angle: f64, opposite: bool, tol: f64) -> FResult<Vec<Shape>> {
    let x = if dir.cross(DVec3::X).length() > 0.1 { DVec3::X } else { DVec3::Y };
    let neutral = k::Plane::new(top, dir, dir.cross(x).normalize());
    let pull = if opposite { dir } else { -dir };
    let mut out = Vec::new();
    for piece in rib {
        let faces = caps(piece);
        let drafted = if faces.is_empty() { piece.clone() } else { super::detail_a::draft_robust(piece, &faces, pull, &neutral, angle).map_err(|_| fe(RIB_FAILED))?.shape };
        let cut = k::cut(&drafted, base)?.shape;
        out.extend(cut.solids().into_iter().filter(|s| k::distance(s, profile).is_ok_and(|d| d.distance < tol)));
    }
    if out.is_empty() {
        return err(RIB_FAILED);
    }
    Ok(out)
}

/// Rib normal to the sketch plane from a chain of curves: the profile
/// thickened in-plane, extruded along `dir` up to the part, fused with it.
fn rib_normal(base: &Shape, wire: &Shape, dir: DVec3, t: f64, draft: Option<(f64, bool)>) -> FResult<k::OpResult> {
    let outline = k::offset_wire_2d(wire, t * 0.5, k::JoinType::Arc, false).map_err(|_| fe(RIB_FAILED))?;
    let outline = outline.wires().into_iter().next().unwrap_or(outline);
    let face = k::make_face_from_wire(&outline, true).map_err(|_| fe(RIB_FAILED))?;
    let body = k::extrude_up_to(&face, dir, base, 0.0).map_err(|_| fe(RIB_MISS))?;
    if body.shape.count(ShapeType::Solid) == 0 {
        return err(RIB_MISS);
    }
    let mut ribs = vec![body.shape];
    if let Some((angle, opposite)) = draft {
        let size = (outline.bounding_box(false)?.max - outline.bounding_box(false)?.min).length();
        let tol = 1e-7 * (1.0 + size);
        let top = wire.vertices().first().and_then(|v| v.vertex_point().ok()).unwrap_or_default();
        ribs = draft_rib(base, &ribs, wire, |s| rib_sides(s, wire, dir, tol), top, dir, angle, opposite, tol)?;
    }
    let res = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(base), &ribs, &k::BooleanOptions::default())?;
    if res.shape.count(ShapeType::Solid) != base.count(ShapeType::Solid) {
        return err(RIB_MISS);
    }
    Ok(res)
}

// ------------------------------------------------------------------ primitives

pub fn cube(ev: &mut EvalCtx) -> FResult<()> {
    let s = ev.length("sideLength")?;
    if s <= 0.0 {
        return err("Side length must be positive");
    }
    let shape = k::make_box(&k::Plane::xy(), s, s, s)?;
    let names = name_new(ev, &shape);
    ev.apply_operation(vec![NamedShape::new(shape, names)], "NEW", None, BodyKind::Solid)
}

pub fn sphere(ev: &mut EvalCtx) -> FResult<()> {
    let r = ev.length("radius")?;
    if r <= 0.0 {
        return err("Radius must be positive");
    }
    let c = match ev.query("center").first() {
        Some(q) => ev.resolve_vertex_point(q).ok_or_else(|| fe("The sphere center no longer exists"))?,
        None => return err("Select a center point"),
    };
    let shape = k::make_sphere(&k::Plane::new(c, DVec3::Z, DVec3::X), r, std::f64::consts::TAU)?;
    let names = name_new(ev, &shape);
    ev.apply_operation(vec![NamedShape::new(shape, names)], "NEW", None, BodyKind::Solid)
}
