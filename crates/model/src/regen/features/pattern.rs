//! Linear / circular / curve pattern, Mirror, Transform and Copy part.
//!
//! * Part pattern: transformed copies of the selected bodies (and mate
//!   connectors). New: every copy is a separate new part; Add / Remove /
//!   Intersect: copies are merged with `apply_operation`.
//! * Face pattern: the selected faces of a body are closed into a tool
//!   solid (planar boundary loops are capped), which is classified as a boss
//!   (adds material) or a pocket (removes material) from the body's face
//!   orientation; each instance of the tool is fused into / cut from the
//!   parts it touches. Faces with non-planar boundaries give a clear error.
//! * Feature pattern, "Reapply features" on (default for Mirror): for every
//!   instance (in instance order), the selected features (in feature-list
//!   order, sketches skipped) are re-evaluated against the current context
//!   with the instance transform pushed on the `EvalCtx`: their sketches are
//!   transformed in a scratch context, and the `resolve_*` helpers map
//!   points, planes, axes, directions, mate connectors and face profiles,
//!   while topology lookups (up-to faces, merge scopes, Up to next) see the
//!   real geometry, so end conditions re-evaluate per instance. Faces an
//!   instance creates are renamed `Copy { instance, of: <name the seed
//!   feature gives> }` (the same names the geometric path produces), bodies
//!   it creates get ids of the pattern feature. Instances that fail are
//!   skipped with a warning; if the features can't be re-applied at all
//!   (unknown or later features) or every instance fails, the geometric
//!   path below is used instead.
//! * Feature pattern, "Reapply features" off: features are patterned through
//!   their geometry: bodies created by the feature are copied as new parts,
//!   and the faces the feature left on other bodies are face-patterned as
//!   above (falls back to re-applying when that fails, e.g. non-planar
//!   boundaries). Features are processed in feature-list order.
//!
//! Naming: every face of an instance is `Copy { instance, of }` where
//! `instance` encodes the instance's grid index relative to the seed (zig-zag
//! encoded, second direction in the high 16 bits), so references to an
//! instance survive upstream edits and count changes.

use super::helpers_a::*;
use crate::display::BodyKind;
use crate::naming::Name;
use crate::reference::{BodyId, FeatureId, Reference, Role, Source, TopoName};
use crate::regen::context::{Body, Connector, SketchResult};
use crate::regen::eval::{EffectiveFeature, EvalCtx, FResult, FeatureError, NamedShape, err, err_at, kernel_plane, transform_frame};
use glam::{DMat4, DQuat, DVec3, DVec4};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeSet;
use std::sync::Arc;

/// One pattern instance (the seed is never an instance).
#[derive(Clone)]
pub struct Instance {
    /// Stable instance identity used in `Role::Copy`.
    pub code: u32,
    pub t: k::Transform,
}

fn skipped(ev: &EvalCtx, two: bool) -> FResult<BTreeSet<(i64, i64)>> {
    let mut out = BTreeSet::new();
    if !ev.bool("skipInstances") {
        return Ok(out);
    }
    for item in ev.params.array("skippedInstances") {
        if two {
            let a = ev.eval_num_in(item, "index1", crate::params::NumKind::Integer)?.round() as i64;
            let b = ev.eval_num_in(item, "index2", crate::params::NumKind::Integer)?.round() as i64;
            out.insert((a, b));
        } else {
            let a = ev.eval_num_in(item, "index", crate::params::NumKind::Integer)?.round() as i64;
            out.insert((a, 0));
        }
    }
    Ok(out)
}

/// linearPattern.fs / circularPattern.fs / curvePattern.fs: the info for a
/// skipped seed (index 0) or an index outside `(count, centered)` per
/// direction, the seed's first.
fn skip_info(ev: &mut EvalCtx, skip: &BTreeSet<(i64, i64)>, one: (i64, bool), two: (i64, bool)) {
    let outside = |i: i64, (n, centered): (i64, bool)| i >= n || i < if centered { -n + 1 } else { 0 };
    if skip.contains(&(0, 0)) {
        ev.infos.push("Original pattern instance will not be skipped.".into());
    } else if skip.iter().any(|&(a, b)| outside(a, one) || outside(b, two)) {
        ev.infos.push("Index is outside patterned instance count.".into());
    }
}

fn index_range(n: i64, centered: bool) -> std::ops::RangeInclusive<i64> {
    if centered { -(n - 1)..=(n - 1) } else { 0..=(n - 1) }
}

fn code2(i: i64, j: i64) -> u32 {
    zigzag(i) | (zigzag(j) << 16)
}

fn linear_instances(ev: &EvalCtx) -> FResult<Vec<Instance>> {
    let Some(r1) = ev.query("directionOne").first() else { return crate::regen::eval::err_at(&["directionOne"], "Select a pattern direction.") };
    let Some(d1) = ev.resolve_direction(r1) else { return crate::regen::eval::err_at(&["directionOne"], "Select a pattern direction.") };
    let mut d1 = d1.normalize();
    if ev.bool("oppositeDirection") {
        d1 = -d1;
    }
    let s1 = ev.length("distance")?;
    let n1 = ev.integer("instanceCount")?.max(1);
    // linearPattern.fs: an empty second direction only matters when its
    // count is above 1.
    let n2_set = ev.integer("instanceCountTwo")?.max(1);
    let second = if ev.bool("hasSecondDir") { ev.query("directionTwo").first().and_then(|r| ev.resolve_direction(r)) } else { None };
    let (d2, s2, n2, c2) = match second {
        Some(d) => {
            let d = d.normalize() * if ev.bool("oppositeDirectionTwo") { -1.0 } else { 1.0 };
            (d, ev.length("distanceTwo")?, n2_set, ev.bool("isCenteredTwo"))
        }
        None if ev.bool("hasSecondDir") && n2_set > 1 => return crate::regen::eval::err_at(&["directionTwo"], "Select a pattern direction."),
        None => (DVec3::ZERO, 0.0, 1, false),
    };
    if n1 * n2 > 2500 {
        return err("Cannot have more than 2500 instances in a pattern.");
    }
    let skip = skipped(ev, true)?;
    let mut out = Vec::new();
    for j in index_range(n2, c2) {
        for i in index_range(n1, ev.bool("isCentered")) {
            if (i == 0 && j == 0) || skip.contains(&(i, j)) {
                continue;
            }
            out.push(Instance { code: code2(i, j), t: k::Transform::Translate(d1 * s1 * i as f64 + d2 * s2 * j as f64) });
        }
    }
    Ok(out)
}

fn circular_instances(ev: &EvalCtx) -> FResult<Vec<Instance>> {
    let Some(r) = ev.query("axis").first() else { return crate::regen::eval::err_at(&["axis"], "Select a pattern axis.") };
    let Some((origin, mut axis)) = ev.resolve_axis(r) else { return crate::regen::eval::err_at(&["axis"], "Select a pattern axis.") };
    if ev.bool("oppositeDirection") {
        axis = -axis;
    }
    let angle = ev.angle("angle")?;
    let n = ev.integer("instanceCount")?.max(1);
    let step = if ev.bool("equalSpace") {
        if (angle.abs() - std::f64::consts::TAU).abs() < 1e-9 {
            angle / n as f64
        } else if n > 1 {
            angle / (n - 1) as f64
        } else {
            0.0
        }
    } else {
        angle
    };
    let skip = skipped(ev, false)?;
    let mut out = Vec::new();
    for i in index_range(n, ev.bool("isCentered")) {
        if i == 0 || skip.contains(&(i, 0)) {
            continue;
        }
        out.push(Instance { code: code2(i, 0), t: k::Transform::Rotate { origin, axis, angle: step * i as f64 } });
    }
    Ok(out)
}

/// Densely sampled chained path.
struct Path {
    pts: Vec<DVec3>,
    cum: Vec<f64>,
}

impl Path {
    fn new(pts: Vec<DVec3>) -> Self {
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            let l = cum.last().copied().unwrap_or(0.0) + w[0].distance(w[1]);
            cum.push(l);
        }
        Path { pts, cum }
    }
    fn length(&self) -> f64 {
        *self.cum.last().unwrap_or(&0.0)
    }
    fn at(&self, s: f64) -> (DVec3, DVec3) {
        let n = self.pts.len();
        let s = s.clamp(0.0, self.length());
        let i = match self.cum.binary_search_by(|c| c.total_cmp(&s)) {
            Ok(i) => i.min(n - 2),
            Err(i) => i.saturating_sub(1).min(n - 2),
        };
        let seg = self.cum[i + 1] - self.cum[i];
        let f = if seg > 0.0 { (s - self.cum[i]) / seg } else { 0.0 };
        let t = (self.pts[i + 1] - self.pts[i]).normalize_or_zero();
        (self.pts[i].lerp(self.pts[i + 1], f), t)
    }
    /// Unit normal of the path plane if planar.
    fn plane_normal(&self) -> Option<DVec3> {
        let c = self.pts.iter().copied().sum::<DVec3>() / self.pts.len() as f64;
        let mut n = DVec3::ZERO;
        for w in self.pts.windows(2) {
            n += (w[0] - c).cross(w[1] - c);
        }
        let n = n.try_normalize()?;
        let scale = self.length().max(1e-9);
        self.pts.iter().all(|p| (*p - c).dot(n).abs() < 1e-7 * scale.max(1.0)).then_some(n)
    }
}

fn sample_edge(e: &Shape) -> FResult<Vec<DVec3>> {
    let info = e.edge_info()?;
    let n = if matches!(info.curve, k::Curve::Line { .. }) { 1 } else { 96 };
    (0..=n).map(|i| Ok(e.point_at(info.first_param + (info.last_param - info.first_param) * i as f64 / n as f64)?)).collect()
}

fn chain_path(edges: &[SourcedEdge]) -> FResult<Path> {
    let mut rest: Vec<Vec<DVec3>> = edges.iter().map(|e| sample_edge(&e.shape)).collect::<FResult<_>>()?;
    if rest.is_empty() {
        return err("Select a path");
    }
    let mut chain = rest.remove(0);
    let tol = 1e-6;
    // Orient the first edge towards the next one when possible.
    if let Some(next) = rest.first()
        && let (Some(s), Some(n0), Some(n1)) = (chain.first().copied(), next.first().copied(), next.last().copied())
        && (s.distance(n0) < tol || s.distance(n1) < tol)
    {
        chain.reverse();
    }
    while !rest.is_empty() {
        let end = *chain.last().unwrap_or(&DVec3::ZERO);
        let start = *chain.first().unwrap_or(&DVec3::ZERO);
        let Some((i, how)) = rest.iter().enumerate().find_map(|(i, s)| {
            let (a, b) = (*s.first()?, *s.last()?);
            if a.distance(end) < tol {
                Some((i, 0))
            } else if b.distance(end) < tol {
                Some((i, 1))
            } else if b.distance(start) < tol {
                Some((i, 2))
            } else if a.distance(start) < tol {
                Some((i, 3))
            } else {
                None
            }
        }) else {
            return err("The pattern path must be a connected chain of edges");
        };
        let mut s = rest.remove(i);
        match how {
            0 => chain.extend(s.into_iter().skip(1)),
            1 => {
                s.reverse();
                chain.extend(s.into_iter().skip(1));
            }
            2 => {
                s.pop();
                s.extend(chain);
                chain = s;
            }
            _ => {
                s.reverse();
                s.pop();
                s.extend(chain);
                chain = s;
            }
        }
    }
    if chain.len() < 2 {
        return err("The pattern path is degenerate");
    }
    Ok(Path::new(chain))
}

/// Tight box of what a pattern copies (parts, faces, or the selected
/// features' bodies and faces).
fn seed_box(ev: &EvalCtx) -> Option<(DVec3, DVec3)> {
    let mut shapes: Vec<Shape> = Vec::new();
    match ev.enum_("patternType") {
        "FACE" => shapes.extend(ev.resolve_faces(ev.query("faces")).into_iter().filter_map(|(bi, fi)| ev.ctx.bodies[bi].shape.sub_shape(ShapeType::Face, fi).ok())),
        "FEATURE" => {
            let feats: Vec<FeatureId> = ev.query("instanceFunction").iter().filter_map(|r| if let Reference::Feature(f) = r { Some(*f) } else { None }).collect();
            for b in &ev.ctx.bodies {
                if feats.contains(&b.id.feature) {
                    shapes.push(b.shape.clone());
                } else {
                    shapes.extend(b.faces.iter().enumerate().filter(|(_, n)| n.creator().is_some_and(|c| feats.contains(&c))).filter_map(|(i, _)| b.shape.sub_shape(ShapeType::Face, i).ok()));
                }
            }
        }
        _ => shapes.extend(ev.resolve_bodies(ev.query("entities")).into_iter().map(|i| ev.ctx.bodies[i].shape.clone())),
    }
    let boxes: Vec<k::BoundingBox> = shapes.iter().filter_map(|s| s.bounding_box(true).ok()).collect();
    let first = boxes.first()?;
    Some(boxes.iter().fold((first.min, first.max), |(lo, hi), b| (lo.min(b.min), hi.max(b.max))))
}

/// `TOLERANCE.zeroLength` (meters).
const ZERO_LENGTH: f64 = 1e-8;

fn curve_instances(ev: &EvalCtx) -> FResult<(Vec<Instance>, Option<&'static str>)> {
    let edges = resolve_curves(ev, ev.query("edges"))?;
    if edges.is_empty() {
        return crate::regen::eval::err_at(&["edges"], "Select a path to pattern along.");
    }
    let orient = ev.enum_("orientationType").to_string();
    // curvePattern.fs checks the faces before building the path.
    if orient == "LOCK_FACES" && ev.query("lockedFaces").is_empty() {
        return crate::regen::eval::err_at(&["lockedFaces"], "Select face to be normal to.");
    }
    let mut path = chain_path(&edges).or_else(|_| crate::regen::eval::err_at(&["edges"], "Edges do not form a continuous path."))?;
    let len = path.length();
    let n = ev.integer("instanceCount")?.max(1);
    let closed = path.pts.first().zip(path.pts.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-6);
    // path.fs: the path starts at the end nearer the seed (a closed one at
    // its point nearest the seed); curvePattern.fs's info when that start
    // isn't in the seed's box.
    let mut info = None;
    if let Some((lo, hi)) = seed_box(ev) {
        let c = (lo + hi) * 0.5;
        if closed {
            let i = path.pts.iter().enumerate().min_by(|a, b| a.1.distance(c).total_cmp(&b.1.distance(c))).map_or(0, |(i, _)| i);
            if i > 0 && i + 1 < path.pts.len() {
                let mut pts = path.pts[i..path.pts.len() - 1].to_vec();
                pts.extend_from_slice(&path.pts[..=i]);
                path = Path::new(pts);
            }
        } else if path.pts.last().is_some_and(|e| e.distance(c) < path.pts[0].distance(c)) {
            let mut pts = path.pts.clone();
            pts.reverse();
            path = Path::new(pts);
        }
        let tol = 1e-9 + (hi - lo).length() * 1e-9;
        let start = path.pts[0];
        let inside = start.cmpge(lo - DVec3::splat(tol)).all() && start.cmple(hi + DVec3::splat(tol)).all();
        if !inside && n > 1 {
            info = Some(if closed { "For best results ensure that the path and pattern seed are touching." } else { "For best results ensure that the path start point and pattern seed are touching." });
        }
    }
    let spacing = if ev.is("spacingType", "DISTANCE") {
        let d = ev.length("distance")?;
        // curvePattern.fs: evPathLength (the edges' exact length; the sampled
        // path is a little shorter). A closed path keeps a gap of d back to
        // the start, and instance i sits at path parameter d·i / (L − d) of
        // the whole path (L for an open one), so 25 mm × 3 on a 100 mm circle
        // lands at 120° / 240° (live L45) but 10 mm × 3 at 40° / 80°.
        let exact = edges.iter().map(|e| e.shape.length()).sum::<Result<f64, _>>().unwrap_or(len);
        let room = if closed { exact - d } else { exact };
        let need = d * if closed { n as f64 } else { (n - 1) as f64 };
        if n > 1 && room - need < -ZERO_LENGTH {
            return crate::regen::eval::err_at(&["distance"], "Cannot create a curve pattern when the entered distance is larger than the path length.");
        }
        if room > 0.0 { d / room * len } else { 0.0 }
    } else if closed {
        len / n as f64
    } else if n > 1 {
        len / (n - 1) as f64
    } else {
        0.0
    };
    // Normal to face: each instance's frame is (curve tangent, the faces' normal
    // made perpendicular to it), mapped from the frame at the seed.
    let faces: Vec<k::Shape> = if orient == "LOCK_FACES" {
        let f: Vec<k::Shape> = ev.resolve_faces(ev.query("lockedFaces")).into_iter().filter_map(|(bi, fi)| ev.ctx.bodies[bi].shape.sub_shape(ShapeType::Face, fi).ok()).collect();
        if f.is_empty() {
            return crate::regen::eval::err_at(&["lockedFaces"], "Select face to be normal to.");
        }
        f
    } else {
        Vec::new()
    };
    let frame = |p: DVec3, t: DVec3| -> FResult<glam::DMat3> {
        let n = faces
            .iter()
            .filter_map(|f| f.project_point(p).ok().map(|pr| (f, pr)))
            .min_by(|a, b| a.1.distance.total_cmp(&b.1.distance))
            .and_then(|(f, pr)| f.normal_at_uv(pr.u, pr.v).ok())
            .ok_or_else(|| fe("Could not find the faces' normal along the path"))?;
        let x = t.normalize();
        let z = (n - x * n.dot(x)).try_normalize().ok_or_else(|| fe("The faces' normal is along the path"))?;
        Ok(glam::DMat3::from_cols(x, z.cross(x), z))
    };
    let (p0, t0) = path.at(0.0);
    let f0 = if faces.is_empty() { None } else { Some(frame(p0, t0)?) };
    let pn = path.plane_normal();
    let skip = skipped(ev, false)?;
    let mut out = Vec::new();
    for i in 1..n {
        if skip.contains(&(i, 0)) {
            continue;
        }
        let (p, t) = path.at(spacing * i as f64);
        let m = if orient == "KEEP_ORIENTATION" {
            DMat4::from_translation(p - p0)
        } else if let Some(f0) = f0 {
            let r = frame(p, t)? * f0.transpose();
            DMat4::from_translation(p) * DMat4::from_mat3(r) * DMat4::from_translation(-p0)
        } else {
            let rot = match pn {
                Some(nrm) => {
                    let a = t0.cross(t).dot(nrm).atan2(t0.dot(t));
                    DQuat::from_axis_angle(nrm, a)
                }
                None => DQuat::from_rotation_arc(t0, t),
            };
            DMat4::from_translation(p) * DMat4::from_quat(rot) * DMat4::from_translation(-p0)
        };
        out.push(Instance { code: code2(i, 0), t: k::Transform::Matrix(m) });
    }
    Ok((out, info))
}

// ------------------------------------------------------------------ entry points

/// Both linear directions set and parallel (linearPattern.fs reports it).
fn parallel_directions(ev: &EvalCtx) -> bool {
    if !ev.bool("hasSecondDir") {
        return false;
    }
    let dir = |id: &str| ev.query(id).first().and_then(|r| ev.resolve_direction(r));
    dir("directionOne").zip(dir("directionTwo")).is_some_and(|(a, b)| a.normalize().cross(b.normalize()).length() < 1e-9)
}

pub fn linear_pattern(ev: &mut EvalCtx) -> FResult<()> {
    check_seeds(ev, "pattern")?;
    let inst = linear_instances(ev)?;
    let two = ev.bool("hasSecondDir");
    let (n1, n2) = (ev.integer("instanceCount")?.max(1), if two { ev.integer("instanceCountTwo")?.max(1) } else { 1 });
    let (c1, c2) = (ev.bool("isCentered"), two && ev.bool("isCenteredTwo"));
    let skip = skipped(ev, true)?;
    skip_info(ev, &skip, (n1, c1), (n2, c2));
    if parallel_directions(ev) {
        ev.infos.push("Parallel directions selected for first and second direction.".into());
    }
    run_pattern(ev, inst)
}

pub fn circular_pattern(ev: &mut EvalCtx) -> FResult<()> {
    check_seeds(ev, "pattern")?;
    let inst = circular_instances(ev)?;
    let (n, centered) = (ev.integer("instanceCount")?.max(1), ev.bool("isCentered"));
    let skip = skipped(ev, false)?;
    skip_info(ev, &skip, (n, centered), (1, false));
    run_pattern(ev, inst)
}

pub fn curve_pattern(ev: &mut EvalCtx) -> FResult<()> {
    check_seeds(ev, "pattern")?;
    let (inst, info) = curve_instances(ev)?;
    ev.infos.extend(info.map(String::from));
    let n = ev.integer("instanceCount")?.max(1);
    let skip = skipped(ev, false)?;
    skip_info(ev, &skip, (n, false), (1, false));
    run_pattern(ev, inst)
}

pub fn mirror(ev: &mut EvalCtx) -> FResult<()> {
    check_seeds(ev, "mirror")?;
    let Some(r) = ev.query("mirrorPlane").first() else { return crate::regen::eval::err_at(&["mirrorPlane"], "Select a mirror plane.") };
    let Some(frame) = ev.resolve_plane(r) else { return crate::regen::eval::err_at(&["mirrorPlane"], "Select a mirror plane.") };
    run_pattern(ev, vec![Instance { code: 1, t: k::Transform::Mirror(kernel_plane(&frame)) }])
}

/// Onshape checks what is patterned before the direction / axis / plane:
/// an empty dialog says "Select parts to pattern." (or mirror).
fn check_seeds(ev: &EvalCtx, verb: &str) -> FResult<()> {
    match ev.enum_("patternType") {
        "FEATURE" if ev.query("instanceFunction").is_empty() => crate::regen::eval::err_at(&["instanceFunction"], format!("Select features to {verb}.")),
        "FEATURE" => Ok(()),
        "FACE" if ev.query("faces").is_empty() => crate::regen::eval::err_at(&["faces"], format!("Select faces to {verb}.")),
        "FACE" => Ok(()),
        _ if ev.query("entities").is_empty() => crate::regen::eval::err_at(&["entities"], format!("Select parts to {verb}.")),
        _ => Ok(()),
    }
}

/// "mirror" for Mirror, else "pattern" (Onshape's error texts).
fn verb(ev: &EvalCtx) -> &'static str {
    if ev.features.iter().any(|f| f.id == ev.feature && f.type_id == "mirror") { "mirror" } else { "pattern" }
}

fn run_pattern(ev: &mut EvalCtx, instances: Vec<Instance>) -> FResult<()> {
    match ev.enum_("patternType") {
        "FEATURE" => feature_pattern(ev, &instances),
        "FACE" => {
            let groups = by_body(ev.resolve_faces(ev.query("faces")));
            if groups.is_empty() {
                let verb = verb(ev);
                return crate::regen::eval::err_at(&["faces"], format!("Select faces to {verb}."));
            }
            if instances.is_empty() {
                return Ok(());
            }
            // Live L43: faces from two parts.
            if groups.len() > 1 {
                return err("Failed to create pattern, check input.");
            }
            let groups: Vec<(BodyId, Vec<usize>)> = groups.into_iter().map(|(bi, fs)| (ev.ctx.bodies[bi].id.clone(), fs)).collect();
            // Onshape's PATTERN_FACE_FAILED / MIRROR_FACE_FAILED: the reason isn't shown.
            let failed = if verb(ev) == "mirror" { "Failed to create face mirror, check input." } else { "Failed to create pattern, check input." };
            for (id, fs) in groups {
                pattern_faces(ev, &id, &fs, &instances, true).map_err(|e| {
                    if e.message == "Cancelled" || e.message == NOT_ON_BODY {
                        return e;
                    }
                    log::debug!("face pattern: {}", e.message);
                    fe(failed)
                })?;
            }
            Ok(())
        }
        _ => part_pattern(ev, &instances),
    }
}

// ------------------------------------------------------------------ part pattern

fn transform_connector(c: &Connector, t: &k::Transform) -> (DVec3, DVec3, DVec3) {
    let m = t.matrix();
    (m.transform_point3(c.origin), m.transform_vector3(c.x).normalize(), m.transform_vector3(c.z).normalize())
}

/// Copy bodies (and connectors) for each instance; returns the copies.
/// Each copy is the body moved (located, sharing its geometry and mesh;
/// a mirror shares one mirrored copy), also for instances that go into a
/// boolean: OCCT intersects congruent pairs of instances once (patch 0001),
/// and the fins ×1000 regenerate faster than with deep copies (9.3 s against
/// 10.5 for the pattern, same 4004 faces).
fn copy_bodies(ev: &EvalCtx, bodies: &[usize], instances: &[Instance]) -> FResult<Vec<(usize, usize, BodyKind, NamedShape)>> {
    let mut out = Vec::new();
    for (ii, inst) in instances.iter().enumerate() {
        if ev.cancelled() {
            return err("Cancelled");
        }
        for (bj, &bi) in bodies.iter().enumerate() {
            let b = &ev.ctx.bodies[bi];
            let res = k::transform(&b.shape, &inst.t, false).map_err(kerr("Pattern failed"))?;
            let names = copy_names(ev, &b.faces, &res, inst.code);
            out.push((ii, bj, b.kind, NamedShape::new(res.shape, names)));
        }
    }
    Ok(out)
}

fn pattern_connectors(ev: &mut EvalCtx, refs: &[Reference], instances: &[Instance]) {
    let conns: Vec<Connector> = refs.iter().filter(|r| matches!(r, Reference::MateConnector { .. })).filter_map(|r| ev.resolve_connector(r)).collect();
    let nc = conns.len() as u32;
    for (ii, inst) in instances.iter().enumerate() {
        for (cj, c) in conns.iter().enumerate() {
            let (origin, x, z) = transform_connector(c, &inst.t);
            ev.ctx.connectors.push(Connector { reference: Reference::MateConnector { feature: ev.feature, index: ii as u32 * nc + cj as u32 }, name: c.name.clone(), origin, x, z });
        }
    }
}

fn part_pattern(ev: &mut EvalCtx, instances: &[Instance]) -> FResult<()> {
    let refs = ev.query("entities");
    let bodies: Vec<usize> = ev.resolve_bodies(refs);
    let has_conn = refs.iter().any(|r| matches!(r, Reference::MateConnector { .. }));
    if bodies.is_empty() && !has_conn {
        return crate::regen::eval::err_at(&["entities"], "Select parts to pattern.");
    }
    if instances.is_empty() {
        return Ok(());
    }
    let op = ev.enum_("operationType").to_string();
    let copies = copy_bodies(ev, &bodies, instances)?;
    pattern_connectors(ev, refs, instances);
    if op == "NEW" || copies.iter().all(|c| c.2 != BodyKind::Solid) {
        // Live (qa5 P1-13): the instances keep the seed's colour.
        let nb = bodies.len().max(1);
        for (ii, bj, kind, ns) in copies {
            let color = ev.ctx.bodies[bodies[bj]].color;
            push_body_colored(ev, BodyId { feature: ev.feature, index: (ii * nb + bj) as u32 }, kind, ns.shape, ns.names.faces, color);
        }
        return Ok(());
    }
    let tools: Vec<NamedShape> = copies.into_iter().filter(|c| c.2 == BodyKind::Solid).map(|c| c.3).collect();
    let scope = super::extrude::scope(ev);
    ev.apply_operation(tools, &op, scope, BodyKind::Solid)
}

// ------------------------------------------------------------------ face / feature pattern

/// A tool solid built from faces of a body.
struct FaceTool {
    shape: Shape,
    names: Vec<Name>,
    add: bool,
}

fn faces_tool(ev: &EvalCtx, body: &Body, faces: &[usize]) -> FResult<FaceTool> {
    let sel: BTreeSet<u32> = faces.iter().map(|f| *f as u32).collect();
    let face_shapes: Vec<Shape> = faces.iter().map(|f| body.shape.sub_shape(ShapeType::Face, *f)).collect::<Result<_, _>>()?;
    let ef = body.shape.edge_faces()?;
    let boundary: Vec<Shape> = ef
        .iter()
        .enumerate()
        .filter(|(_, fs)| fs.iter().any(|f| sel.contains(f)) && fs.iter().any(|f| !sel.contains(f)))
        .filter_map(|(ei, _)| body.shape.sub_shape(ShapeType::Edge, ei).ok())
        .collect();
    let mut caps: Vec<Shape> = Vec::new();
    if !boundary.is_empty() {
        for w in k::edges_to_wires(&Shape::compound(&boundary), 1e-6)? {
            let cap = k::make_face_from_wire(&w, true).map_err(|_| fe("These faces can't be patterned: their boundary is not planar"))?;
            caps.push(cap);
        }
    }
    let mut inputs = face_shapes.clone();
    inputs.extend(caps.iter().cloned());
    let sewn = k::sew(&inputs, 1e-6, true).map_err(kerr("These faces can't be patterned (they don't bound a closed volume)"))?;
    // Separate closed pieces (a Hole at several points) make one tool.
    let solid = match sewn.shape.count(ShapeType::Solid) {
        0 => return err("These faces can't be patterned (they don't bound a closed volume)"),
        1 => sewn.shape.solids().into_iter().next().ok_or_else(|| fe("These faces can't be patterned (they don't bound a closed volume)"))?,
        _ => Shape::compound(&sewn.shape.solids()),
    };
    // Names: selected faces keep their names, caps are new.
    let mut names: Vec<Option<Name>> = vec![None; solid.count(ShapeType::Face)];
    for r in &sewn.history.records {
        if r.input_kind == ShapeType::Face && r.output_kind == ShapeType::Face && matches!(r.relation, k::Relation::Modified | k::Relation::Kept) {
            let Some(out_face) = sewn.shape.sub_shape(ShapeType::Face, r.output_index as usize).ok() else { continue };
            let Some(j) = solid.index_of(&out_face, ShapeType::Face) else { continue };
            let n = match faces.get(r.arg as usize) {
                Some(fi) => body.faces[*fi].clone(),
                None => Arc::new(TopoName::face(ev.feature, Role::New(r.arg - faces.len() as u32))),
            };
            names[j].get_or_insert(n);
        }
    }
    // Geometric fallback for faces without history.
    let all_inputs: Vec<(Shape, Name)> = face_shapes
        .iter()
        .zip(faces)
        .map(|(s, fi)| (s.clone(), body.faces[*fi].clone()))
        .chain(caps.iter().enumerate().map(|(c, s)| (s.clone(), Arc::new(TopoName::face(ev.feature, Role::New(c as u32))))))
        .collect();
    let names: Vec<Name> = solid
        .faces()
        .iter()
        .enumerate()
        .map(|(j, f)| {
            names[j].clone().unwrap_or_else(|| {
                let c = f.centroid().unwrap_or(DVec3::ZERO);
                all_inputs.iter().min_by(|a, b| a.0.centroid().unwrap_or(DVec3::ZERO).distance(c).total_cmp(&b.0.centroid().unwrap_or(DVec3::ZERO).distance(c))).map(|x| x.1.clone()).unwrap_or_else(|| Arc::new(TopoName::face(ev.feature, Role::New(999))))
            })
        })
        .collect();
    // Boss or pocket: probe just outside the body at the first face.
    let f0 = &face_shapes[0];
    let p = point_on_face(f0).ok_or_else(|| fe("Pattern failed: degenerate face"))?;
    let (_, n) = face_normal_near(f0, p).ok_or_else(|| fe("Pattern failed: degenerate face"))?;
    let size = solid.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0);
    let q = p + n.normalize() * (size * 1e-4).max(1e-7);
    let add = match solid.classify_point(q, 1e-9)? {
        k::PointClass::Inside => false,
        k::PointClass::Outside => true,
        _ => return err("Pattern failed: could not tell whether the faces add or remove material"),
    };
    Ok(FaceTool { shape: solid, names, add })
}

/// `strict` (Face pattern / mirror): an instance off the part fails (live
/// L42/L43); a Feature pattern leaves such instances out.
fn pattern_faces(ev: &mut EvalCtx, body_id: &BodyId, faces: &[usize], instances: &[Instance], strict: bool) -> FResult<()> {
    let Some(bi) = index_of_body(ev, body_id) else { return Ok(()) };
    let body = ev.ctx.bodies[bi].clone();
    let t0 = std::time::Instant::now();
    let tool = faces_tool(ev, &body, faces)?;
    let t_tool = t0.elapsed();
    let mut tools: Vec<NamedShape> = Vec::new();
    for inst in instances {
        if ev.cancelled() {
            return err("Cancelled");
        }
        // Moved, not copied (see copy_bodies).
        let res = k::transform(&tool.shape, &inst.t, false).map_err(kerr("Pattern failed"))?;
        let names = copy_names(ev, &tool.names, &res, inst.code);
        tools.push(NamedShape::new(res.shape, names));
    }
    let t_copies = t0.elapsed();
    let r = apply_tools(ev, body_id, tools, tool.add, body.kind, strict);
    if crate::regen::display::perf_enabled() {
        log::info!(target: "perf", "pattern_faces × {}: tool {:.1} ms, copies + names {:.1} ms, apply {:.1} ms", instances.len(), t_tool.as_secs_f64() * 1e3, (t_copies - t_tool).as_secs_f64() * 1e3, (t0.elapsed() - t_copies).as_secs_f64() * 1e3);
    }
    r
}

/// Fuse (boss) or cut (pocket) instance tools into the parts they touch.
fn apply_tools(ev: &mut EvalCtx, seed: &BodyId, tools: Vec<NamedShape>, add: bool, kind: BodyKind, strict: bool) -> FResult<()> {
    if let Some(bi) = index_of_body(ev, seed) {
        let seed_box = ev.ctx.bodies[bi].shape.bounding_box(false).ok();
        let off = |t: &NamedShape| match (&seed_box, t.shape.bounding_box(false)) {
            (Some(a), Ok(b)) => !boxes_meet(a, &b, 1e-7),
            _ => false,
        };
        // Live L42: a face pattern fails on any instance off the part; L40:
        // a Feature pattern skips one and fails when all are off, asking for
        // Reapply features (live "qa5 bracket" †; std patternUtils).
        if strict && tools.iter().any(off) {
            return err(NOT_ON_BODY);
        }
        if !tools.is_empty() && tools.iter().all(off) {
            return crate::regen::eval::err_at(&["fullFeaturePattern"], SWITCH_TO_PER_INSTANCE);
        }
    }
    if add {
        let Some(bi) = index_of_body(ev, seed) else { return Ok(()) };
        let seed_shape = ev.ctx.bodies[bi].shape.clone();
        // Instances whose boxes meet the part go into one fuse (one that
        // turns out not to touch comes out of it as a separate solid, i.e. a
        // new part, as below); an exact distance per instance cost 11 s for
        // ×1000.
        let seed_box = seed_shape.bounding_box(false).ok();
        let near = |t: &NamedShape| match (&seed_box, t.shape.bounding_box(false)) {
            (Some(a), Ok(b)) => boxes_meet(a, &b, 1e-7),
            _ => true,
        };
        let (touch, free): (Vec<NamedShape>, Vec<NamedShape>) = tools.into_iter().partition(near);
        if !touch.is_empty() {
            ev.boolean_into_with(&[bi], touch, k::BooleanOp::Fuse, &EvalCtx::many_tools_options())?;
        }
        for (i, t) in free.into_iter().enumerate() {
            push_body(ev, BodyId { feature: ev.feature, index: 30_000 + i as u32 }, kind, t.shape, t.names.faces);
        }
        return Ok(());
    }
    // Onshape's face pattern only changes the body the faces are on: the
    // copies of a hole on Part 1 never cut a part overlapping it (qa4 F12).
    let Some(bi) = index_of_body(ev, seed) else { return Ok(()) };
    ev.boolean_into_with(&[bi], tools, k::BooleanOp::Cut, &EvalCtx::many_tools_options())
}

/// Whether two boxes meet (within `tol`).
fn boxes_meet(a: &k::BoundingBox, b: &k::BoundingBox, tol: f64) -> bool {
    a.min.x <= b.max.x + tol && b.min.x <= a.max.x + tol && a.min.y <= b.max.y + tol && b.min.y <= a.max.y + tol && a.min.z <= b.max.z + tol && b.min.z <= a.max.z + tol
}

fn feature_pattern(ev: &mut EvalCtx, instances: &[Instance]) -> FResult<()> {
    let mut feats: Vec<FeatureId> = ev.query("instanceFunction").iter().filter_map(|r| if let Reference::Feature(f) = r { Some(*f) } else { None }).collect();
    if feats.is_empty() {
        let verb = verb(ev);
        return crate::regen::eval::err_at(&["instanceFunction"], format!("Select features to {verb}."));
    }
    // Feature-list order (not selection order).
    let order = |f: &FeatureId| ev.features.iter().position(|x| x.id == *f).unwrap_or(usize::MAX);
    feats.sort_by_key(order);
    feats.dedup();
    if instances.is_empty() {
        return Ok(());
    }
    let before = ev.ctx.clone();
    let warnings_before = ev.warnings.len();
    let (sketches, feats): (Vec<FeatureId>, Vec<FeatureId>) = feats.into_iter().partition(|f| ev.features.iter().any(|x| x.id == *f && x.type_id == "sketch"));
    if ev.bool("fullFeaturePattern") && !sketches.is_empty() {
        ev.infos.push(SKETCH_REAPPLY_INFO.into());
    }
    // Live L41: a patterned sketch is copied to every instance (its wire /
    // sheet bodies), with or without Reapply; no part is made.
    copy_sketches(ev, &sketches, instances);
    if feats.is_empty() {
        return Ok(());
    }
    // With Reapply, patternUtils.fs re-runs the features and reports their
    // failure as is (no fallback to the geometry).
    if ev.bool("fullFeaturePattern") {
        return reapply_features(ev, &feats, instances);
    }
    match geometric_feature_pattern(ev, &feats, instances) {
        Ok(()) => Ok(()),
        // Onshape's own failure (live G45): no re-apply.
        // Live L40: every instance off the part fails too.
        Err(e) if e.message == SWITCH_TO_PER_INSTANCE || e.message == NO_GEOMETRY_FROM_FEATURES || e.message == NOT_ON_BODY || ev.cancelled() => Err(e),
        // Faces our face pattern can't make a tool of (Parasolid's can):
        // the same instances by re-applying the features.
        Err(e) => {
            ev.ctx = before;
            ev.warnings.truncate(warnings_before);
            match reapply_features(ev, &feats, instances) {
                Ok(()) => Ok(()),
                Err(_) => Err(e),
            }
        }
    }
}

/// Copies of `sketches` moved to each instance, under ids of their own.
fn copy_sketches(ev: &mut EvalCtx, sketches: &[FeatureId], instances: &[Instance]) {
    for inst in instances {
        let m = inst.t.matrix();
        for f in sketches {
            let Some(src) = ev.ctx.sketches.iter().find(|s| s.feature == *f).cloned() else { continue };
            let id = FeatureId(mix(&[ev.feature.0, f.0, inst.code as u64]) | (1 << 62));
            ev.ctx.sketches.push(Arc::new(SketchResult { feature: id, ..transformed_sketch(&src, &m) }));
            ev.ctx.sketch_copies.push((ev.feature, id));
        }
    }
}

/// Feature pattern through geometry (face / part patterns of what each
/// feature created).
fn geometric_feature_pattern(ev: &mut EvalCtx, feats: &[FeatureId], instances: &[Instance]) -> FResult<()> {
    for &f in feats {
        let mut found = false;
        // Bodies created by the feature: copied as new parts.
        let created: Vec<usize> = (0..ev.ctx.bodies.len()).filter(|i| ev.ctx.bodies[*i].id.feature == f).collect();
        if !created.is_empty() {
            found = true;
            let copies = copy_bodies(ev, &created, instances)?;
            let nb = created.len();
            for (ii, bj, kind, ns) in copies {
                let index = (f.0 as u32 & 0xFFF).wrapping_mul(1 << 20).wrapping_add((ii * nb + bj) as u32);
                let color = ev.ctx.bodies[created[bj]].color;
                push_body_colored(ev, BodyId { feature: ev.feature, index }, kind, ns.shape, ns.names.faces, color);
            }
        }
        // Faces the feature left on other bodies.
        let groups: Vec<(BodyId, Vec<usize>)> = ev
            .ctx
            .bodies
            .iter()
            .filter(|b| b.id.feature != f && b.id.feature != ev.feature)
            .filter_map(|b| {
                let fs: Vec<usize> = b.faces.iter().enumerate().filter(|(_, n)| n.creator() == Some(f)).map(|(i, _)| i).collect();
                (!fs.is_empty()).then(|| (b.id.clone(), fs))
            })
            .collect();
        let targets: Vec<usize> = groups.iter().filter_map(|(id, _)| index_of_body(ev, id)).collect();
        if let Some((id, fs)) = groups.first()
            && let Some(bi) = index_of_body(ev, id)
            && copies_fail(ev, f, &targets, instances)
        {
            let body = ev.ctx.bodies[bi].clone();
            ev.error_shapes = failed_instances(ev, f, &body, fs, instances);
            return crate::regen::eval::err_at(&["fullFeaturePattern"], SWITCH_TO_PER_INSTANCE);
        }
        for (id, fs) in groups {
            found = true;
            // Live "psb flush wall" W1: a wall flush with the part (the
            // feature owns only its caps across the pattern direction) fails
            // from 3 instances; with 2 the copy moves the cap (Parasolid).
            if let Some(bi) = index_of_body(ev, &id)
                && let Some(step) = flush_caps_step(&ev.ctx.bodies[bi], &fs, instances)
            {
                let body = ev.ctx.bodies[bi].clone();
                if instances.len() > 1 {
                    ev.error_shapes = failed_instances(ev, f, &body, &fs, instances);
                    return crate::regen::eval::err_at(&["fullFeaturePattern"], SWITCH_TO_PER_INSTANCE);
                }
                let caps: Vec<Shape> = fs.iter().filter_map(|&i| body.shape.sub_shape(ShapeType::Face, i).ok()).collect();
                let res = k::offset_faces(&body.shape, &caps, step).map_err(|_| fe(SWITCH_TO_PER_INSTANCE))?;
                let offset_role = |n: &Name| Role::Offset(n.clone());
                let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &std::collections::BTreeMap::new(), Some(&offset_role));
                ev.replace_body(bi, res.shape, names);
                continue;
            }
            let merged = index_of_body(ev, &id).and_then(|bi| seed_tool(ev, f).map(|tool| merged_faces(&tool, &ev.ctx.bodies[bi], &fs)));
            if let Some(bi) = index_of_body(ev, &id)
                && face_pattern_fails(&ev.ctx.bodies[bi], &fs, instances, merged.as_deref())
            {
                let body = ev.ctx.bodies[bi].clone();
                ev.error_shapes = failed_instances(ev, f, &body, &fs, instances);
                return crate::regen::eval::err_at(&["fullFeaturePattern"], SWITCH_TO_PER_INSTANCE);
            }
            pattern_faces(ev, &id, &fs, instances, false)?;
        }
        if !found {
            return crate::regen::eval::err_at(&["instanceFunction"], NO_GEOMETRY_FROM_FEATURES);
        }
    }
    Ok(())
}

/// What Onshape shows for a face pattern it can't make (live "lp wall over"
/// 2256–2262, the user's shot of 2026-10-23): a ghost of every instance, the
/// seed's whole tool body moved there, plus the part's faces the seed merged
/// into (a coplanar face of the part running on into the tool is the
/// feature's face too in Parasolid: the user's flush side) moved likewise.
fn failed_instances(ev: &EvalCtx, f: FeatureId, body: &Body, fs: &[usize], instances: &[Instance]) -> Vec<Shape> {
    let mut ghost: Vec<Shape> = Vec::new();
    match seed_tool(ev, f) {
        Some(tool) => {
            ghost.extend(merged_faces(&tool, body, fs).into_iter().filter_map(|i| body.shape.sub_shape(ShapeType::Face, i).ok()));
            ghost.push(tool);
        }
        None => ghost.extend(fs.iter().filter_map(|&i| body.shape.sub_shape(ShapeType::Face, i).ok())),
    }
    let seed = Shape::compound(&ghost);
    // Moved, not copied: the ghosts share the seed's geometry and mesh.
    instances.iter().filter_map(|inst| k::transform(&seed, &inst.t, false).ok().map(|r| r.shape)).collect()
}

/// The body's faces (not the feature's own `fs`) that carry on a face of the
/// seed's tool: in the same plane, facing the same way, reaching past it and
/// sharing part of its outline (Parasolid merges them; the merged face is
/// the feature's too). A tool face wholly inside a face of the part (a rib
/// sketched on the block's bottom plane) leaves that face as it was.
fn merged_faces(tool: &Shape, body: &Body, fs: &[usize]) -> Vec<usize> {
    let tol = 1e-7_f64.max(body.shape.bounding_box(false).map(|b| b.diagonal() * 1e-9).unwrap_or(0.0));
    let tool_planes: Vec<(DVec3, DVec3, DVec3, f64, Vec<DVec3>)> = tool
        .faces()
        .iter()
        .filter_map(|t| {
            let k::Surface::Plane(pl) = t.face_surface().ok()? else { return None };
            let c = t.centroid().ok()?;
            // Points along its outline: line midpoints, other edges' ends.
            let outline = t
                .edges()
                .iter()
                .filter_map(|e| e.edge_info().ok())
                .flat_map(|i| match i.curve {
                    k::Curve::Line { .. } => vec![(i.start + i.end) * 0.5],
                    _ => vec![i.start, i.end],
                })
                .collect();
            Some((pl.origin, t.normal_at_point(c).ok()?.normalize_or_zero(), c, t.area().ok()?, outline))
        })
        .collect();
    (0..body.faces.len())
        .filter(|i| !fs.contains(i))
        .filter(|&i| {
            let Ok(face) = body.shape.sub_shape(ShapeType::Face, i) else { return false };
            let Ok(k::Surface::Plane(_)) = face.face_surface() else { return false };
            let (Ok(c), Ok(area)) = (face.centroid(), face.area()) else { return false };
            let Ok(n) = face.normal_at_point(c) else { return false };
            let n = n.normalize_or_zero();
            let on_edge = |p: &DVec3| k::make_vertex(*p).is_ok_and(|v| face.edges().iter().any(|e| k::distance(e, &v).is_ok_and(|d| d.distance < tol)));
            tool_planes.iter().any(|(o, tn, tc, ta, outline)| {
                tn.dot(n) > 1.0 - 1e-9
                    && (*o - c).dot(n).abs() < tol
                    && area > ta * (1.0 + 1e-6)
                    && k::make_vertex(*tc).and_then(|v| k::distance(&face, &v)).is_ok_and(|d| d.distance < tol)
                    && outline.iter().any(on_edge)
            })
        })
        .collect()
}

/// The solid a feature adds or removes, evaluated as New where it stands.
fn seed_tool(ev: &EvalCtx, f: FeatureId) -> Option<Shape> {
    let seed = ev.features.iter().find(|x| x.id == f)?;
    if crate::custom::is_custom(&seed.type_id) {
        return None;
    }
    let eval_fn = super::evaluator(&seed.type_id)?;
    let tmp = FeatureId(mix(&[ev.feature.0, f.0, u64::from(u32::MAX)]) | (1 << 62));
    let mut params = crate::regen::eval::expand_query_variables(&seed.params, &ev.ctx).unwrap_or_else(|| seed.params.clone());
    params.set("operationType", crate::params::ParamValue::Enum("NEW".into()));
    let mut sub = EvalCtx::new(tmp, &seed.name, &params, ev.units, ev.ctx.clone(), ev.outer_vars);
    sub.features = ev.features;
    sub.scratch = true;
    sub.cancel = ev.cancel.clone();
    eval_fn(&mut sub).ok()?;
    let made: Vec<Shape> = sub.ctx.bodies.iter().filter(|b| b.id.feature == tmp).map(|b| b.shape.clone()).collect();
    (!made.is_empty()).then(|| Shape::compound(&made))
}

/// When a feature's faces on a part are only planar caps across a straight
/// pattern direction (its sides merged into the part's faces: a wall flush
/// with the part), the offset that moves those caps by one step (negative:
/// into the part). None otherwise.
fn flush_caps_step(body: &Body, faces: &[usize], instances: &[Instance]) -> Option<f64> {
    // A pure translation (a curve pattern along a straight edge is one too,
    // live "psb (10-24) flush wall follow-up").
    let m = instances.first()?.t.matrix();
    let rigid = glam::DMat3::from_mat4(m);
    if !rigid.abs_diff_eq(glam::DMat3::IDENTITY, 1e-9) {
        return None;
    }
    let v = m.w_axis.truncate();
    let d = v.try_normalize()?;
    let mut step = None;
    for &i in faces {
        let f = body.shape.sub_shape(ShapeType::Face, i).ok()?;
        let k::Surface::Plane(_) = f.face_surface().ok()? else { return None };
        let n = f.normal_at_point(f.centroid().ok()?).ok()?.normalize_or_zero();
        if n.dot(d).abs() < 1.0 - 1e-9 {
            return None;
        }
        // The cap's outward normal: an offset along it by v·n moves it by v
        // (live: a curve pattern into the part moves the end cap in).
        let s = v.dot(n);
        if step.is_some_and(|x: f64| (x - s).abs() > 1e-12) {
            return None;
        }
        step = Some(s);
    }
    step
}

/// Onshape's PATTERN_SWITCH_TO_PER_INSTANCE: a feature pattern whose face
/// pattern can't make the instances (patternUtils.fs).
const SWITCH_TO_PER_INSTANCE: &str = "Could not create all instances as entered. Try selecting \"Reapply features\" option.";
/// Onshape's PATTERN_NOT_ON_BODY (live L42/L43).
const NOT_ON_BODY: &str = "Pattern could not be created on the same part.";
/// Onshape's info when a Feature pattern reapplies a sketch (live L40/L41).
const SKETCH_REAPPLY_INFO: &str = "Sketch dimensions and constraints using the Origin or the default planes as reference will not be reapplied. Learn more";
/// Onshape's PATTERN_NO_GEOM_FROM_FEATURES.
const NO_GEOMETRY_FROM_FEATURES: &str = "Selected features do not create any geometry that may be patterned.";

/// Whether Onshape's face pattern (Parasolid's) can't place the feature's
/// faces at the instances, so the Feature pattern fails asking for Reapply
/// features (patternUtils.fs: any failure of the face-based pattern):
/// - the faces merged with faces of the body around them (live G45 and
///   "Pattern face merging": a rib as wide as the block, its sides one face
///   with the block's): they then meet the rest of the body along concave
///   edges (where they stand on it) and convex ones (where the merged faces
///   carry on); a boss meets its part along concave edges only, a pocket
///   along convex ones;
/// - an instance's boundary leaves the surfaces it has to attach to (a hole
///   on the block's corner, its edges on the side planes, patterned along
///   them: qa4 F12).
fn face_pattern_fails(body: &Body, faces: &[usize], instances: &[Instance], merged: Option<&[usize]>) -> bool {
    let sel: BTreeSet<u32> = faces.iter().map(|f| *f as u32).collect();
    let Ok(ef) = body.shape.edge_faces() else { return false };
    let probes: Vec<DMat4> = [instances.first(), instances.last()].into_iter().flatten().map(|i| i.t.matrix()).collect();
    let tol = 1e-7_f64.max(body.shape.bounding_box(false).map(|b| b.diagonal() * 1e-9).unwrap_or(0.0));
    // A face the seed merged into is the feature's too, so the copies carry
    // it along: each must land in the plane of another face of the part,
    // facing the same way, to merge into it (qa5 ADV9: a gusset on the
    // bracket's end mirrored to the other end). Onto its own plane (G45's
    // full-width rib patterned along the block, "lp wall over"'s floor,
    // the fins' sides) or into empty space (the fins' front wall moved
    // back) it can't be placed.
    let merged = merged.filter(|m| !m.is_empty());
    let planes: Vec<(usize, DVec3, DVec3)> = match merged {
        Some(_) => (0..body.faces.len())
            .filter_map(|i| {
                let face = body.shape.sub_shape(ShapeType::Face, i).ok()?;
                let k::Surface::Plane(_) = face.face_surface().ok()? else { return None };
                let c = face.centroid().ok()?;
                Some((i, c, face.normal_at_point(c).ok()?.normalize_or_zero()))
            })
            .collect(),
        None => Vec::new(),
    };
    for &m in merged.unwrap_or_default() {
        let Some(&(_, c, n)) = planes.iter().find(|p| p.0 == m) else { continue };
        let lands = |t: &DMat4| {
            let (c2, n2) = (t.transform_point3(c), t.transform_vector3(n).normalize_or_zero());
            planes.iter().any(|&(i, o, nf)| i != m && nf.dot(n2) > 1.0 - 1e-9 && (o - c2).dot(n2).abs() < tol)
        };
        if !probes.iter().all(lands) {
            return true;
        }
    }
    let (mut convex, mut concave) = (false, false);
    for (e, fs) in ef.iter().enumerate() {
        let [a, b] = fs[..] else { continue };
        let (s, o) = match (sel.contains(&a), sel.contains(&b)) {
            (true, false) => (a, b),
            (false, true) => (b, a),
            _ => continue,
        };
        // Merged faces are judged above.
        if merged.is_some_and(|m| m.contains(&(o as usize))) {
            continue;
        }
        // Without the seed's tool, merged faces show as a boss meeting the
        // part along concave edges and its continuations along convex ones.
        if merged.is_none() {
            match edge_convexity(body, e, s as usize, o as usize) {
                1 => convex = true,
                -1 => concave = true,
                _ => {}
            }
            if convex && concave {
                return true;
            }
        }
        // The edge, moved by the instances, still on the other face's surface.
        let (Ok(edge), Ok(other)) = (body.shape.sub_shape(ShapeType::Edge, e), body.shape.sub_shape(ShapeType::Face, o as usize)) else { continue };
        let Ok(info) = edge.edge_info() else { continue };
        let Ok(mid) = edge.point_at((info.first_param + info.last_param) * 0.5) else { continue };
        if probes.iter().any(|m| !on_surface(&other, m.transform_point3(mid), tol)) {
            return true;
        }
    }
    false
}

/// Copies Onshape's face pattern (no Reapply) can't place (live "kern
/// (10-23 22:00) patterns"): copies overlapping each other or the seed (R2:
/// radial holes meeting at the cylinder's axis), an added copy off the part
/// (O3) and, for a mirror, any copy off the part (M2). A removed copy off the
/// part just does nothing (O1). The seed's tool is the feature evaluated as
/// New where it stands.
fn copies_fail(ev: &EvalCtx, f: FeatureId, targets: &[usize], instances: &[Instance]) -> bool {
    let Some(tool) = seed_tool(ev, f) else { return false };
    let add = matches!(ev.features.iter().find(|x| x.id == f).and_then(|x| x.params.get("operationType")), Some(crate::params::ParamValue::Enum(o)) if o == "ADD");
    let mirror = verb(ev) == "mirror";
    let mut tools = vec![tool.clone()];
    // Moved, not copied: only their boxes are compared.
    tools.extend(instances.iter().filter_map(|inst| k::transform(&tool, &inst.t, false).ok().map(|r| r.shape)));
    let size = tool.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0);
    let boxes: Vec<Option<k::BoundingBox>> = tools.iter().map(|t| t.bounding_box(false).ok()).collect();
    for i in 0..tools.len() {
        for j in i + 1..tools.len() {
            let (Some(a), Some(b)) = (&boxes[i], &boxes[j]) else { continue };
            // Copies side by side (boxes sharing a face) don't overlap.
            if !boxes_meet(a, b, -size * 1e-6) {
                continue;
            }
            let common = k::boolean(k::BooleanOp::Common, std::slice::from_ref(&tools[i]), std::slice::from_ref(&tools[j]), &k::BooleanOptions::default());
            if common.is_ok_and(|r| r.shape.volume().is_ok_and(|v| v > (size * 1e-4).powi(3))) {
                return true;
            }
        }
    }
    // Off the part: boxes apart (an exact distance per copy costs seconds
    // for ×1000, see apply_tools). Not where the seed's faces merged into the
    // part's (psb's flush wall, live "psb (10-23) flush wall pattern": a wall
    // covering the block's end face, its sides one face with the block's,
    // patterned ×2 is OK).
    let target_boxes: Vec<k::BoundingBox> = targets.iter().filter_map(|&bi| ev.ctx.bodies[bi].shape.bounding_box(false).ok()).collect();
    (mirror || (add && !merged_into(ev, f, &tool, targets))) && boxes[1..].iter().flatten().any(|b| !target_boxes.iter().any(|a| boxes_meet(a, b, 1e-7)))
}

/// Whether a planar face of `tool` lies on a face of the target parts facing
/// the same way and touching it (the two merged into one face; a face resting
/// on the part faces the other way). The feature `f`'s own faces on the part
/// (the seed itself) don't count.
fn merged_into(ev: &EvalCtx, f: FeatureId, tool: &Shape, targets: &[usize]) -> bool {
    let tol = 1e-9;
    tool.faces().iter().any(|tf| {
        let Ok(Some(tp)) = tf.face_plane() else { return false };
        let Some(c) = point_on_face(tf) else { return false };
        let Some((_, tn)) = face_normal_near(tf, c) else { return false };
        targets.iter().any(|&bi| {
            let body = &ev.ctx.bodies[bi];
            body.shape.faces().iter().enumerate().filter(|(i, _)| body.faces.get(*i).is_none_or(|n| n.creator() != Some(f))).any(|(_, bf)| {
                let Ok(Some(bp)) = bf.face_plane() else { return false };
                if tp.normal.normalize().cross(bp.normal.normalize()).length() > 1e-9 || (tp.origin - bp.origin).dot(bp.normal.normalize()).abs() > tol {
                    return false;
                }
                let Some(bc) = point_on_face(bf) else { return false };
                let Some((_, bn)) = face_normal_near(bf, bc) else { return false };
                tn.dot(bn) > 0.0 && k::distance(tf, bf).is_ok_and(|d| d.distance < tol)
            })
        })
    })
}

/// Whether `p` lies on the (unbounded) surface of `face`.
fn on_surface(face: &Shape, p: DVec3, tol: f64) -> bool {
    let off_axis = |o: DVec3, a: DVec3| {
        let a = a.normalize_or_zero();
        let q = p - o;
        (q.dot(a), (q - a * q.dot(a)).length())
    };
    match face.face_surface() {
        Ok(k::Surface::Plane(pl)) => (p - pl.origin).dot(pl.normal.normalize_or_zero()).abs() < tol,
        Ok(k::Surface::Cylinder { origin, axis, radius, .. }) => (off_axis(origin, axis).1 - radius).abs() < tol,
        Ok(k::Surface::Sphere { center, radius, .. }) => ((p - center).length() - radius).abs() < tol,
        Ok(k::Surface::Cone { axis, half_angle, apex, .. }) => {
            let (h, r) = off_axis(apex, axis);
            (r - h.abs() * half_angle.tan()).abs() < tol
        }
        Ok(k::Surface::Torus { center, axis, major_radius, minor_radius }) => {
            let (h, r) = off_axis(center, axis);
            (((r - major_radius).powi(2) + h * h).sqrt() - minor_radius).abs() < tol
        }
        // Other surfaces: on the face itself.
        _ => face.project_point(p).map(|x| x.distance < tol).unwrap_or(false),
    }
}

/// How faces `fa` and `fb` meet along edge `e`: +1 convex, -1 concave, 0
/// tangent: the sign of (n_a × n_b) · t, t the edge's tangent as face `fa`
/// runs it, n the outward normals.
fn edge_convexity(body: &Body, e: usize, fa: usize, fb: usize) -> i32 {
    let (Ok(edge), Ok(a), Ok(b)) = (body.shape.sub_shape(ShapeType::Edge, e), body.shape.sub_shape(ShapeType::Face, fa), body.shape.sub_shape(ShapeType::Face, fb)) else { return 0 };
    // The edge as face `fa` uses it (its orientation in that face).
    let Some(ea) = a.edges().into_iter().find(|x| x.is_same(&edge)) else { return 0 };
    let Ok(info) = ea.edge_info() else { return 0 };
    let t = (info.first_param + info.last_param) * 0.5;
    let (Ok(p), Ok(tan)) = (ea.point_at(t), ea.tangent_at(t)) else { return 0 };
    let (Ok(na), Ok(nb)) = (a.normal_at_point(p), b.normal_at_point(p)) else { return 0 };
    let c = na.normalize_or_zero().cross(nb.normalize_or_zero()).dot(tan);
    if c > 1e-6 {
        1
    } else if c < -1e-6 {
        -1
    } else {
        0
    }
}

// ------------------------------------------------------------------ reapply

/// Stable 32-bit mix (FNV-1a) for derived ids.
fn mix(parts: &[u64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// Renames the entities a feature created while being re-applied for one
/// instance: the scratch feature id `tmp` becomes the pattern feature.
struct Renamer {
    tmp: FeatureId,
    seed: FeatureId,
    pattern: FeatureId,
    code: u32,
}

impl Renamer {
    fn contains(&self, n: &TopoName) -> bool {
        match n {
            TopoName::Face { feature, role } => *feature == self.tmp || self.role_contains(role),
            TopoName::Split { parent, by, .. } => *by == self.tmp || self.contains(parent),
            TopoName::Edge { faces, .. } => faces.iter().any(|f| self.contains(f)),
            TopoName::Vertex { faces, .. } => faces.iter().any(|f| self.contains(f)),
            TopoName::Wire { feature, source } => *feature == self.tmp || self.source_contains(source),
            TopoName::Boundary { face, .. } => self.contains(face),
        }
    }
    fn role_contains(&self, r: &Role) -> bool {
        match r {
            Role::Side(s) | Role::Profile(s) => self.source_contains(s),
            Role::CapStart(s) | Role::CapEnd(s) => s.as_ref().is_some_and(|s| self.source_contains(s)),
            Role::Blend(n) | Role::Generated(n) | Role::Offset(n) | Role::Copy { of: n, .. } => self.contains(n),
            Role::New(_) | Role::Datum(_) => false,
        }
    }
    fn source_contains(&self, s: &Source) -> bool {
        matches!(s, Source::Topo(n) if self.contains(n))
    }

    /// The name the seed feature would give: `tmp` replaced by the seed id.
    fn to_seed(&self, n: &Name) -> Name {
        if !self.contains(n) {
            return n.clone();
        }
        Arc::new(match &**n {
            TopoName::Face { feature, role } => TopoName::Face { feature: if *feature == self.tmp { self.seed } else { *feature }, role: self.map_role(role, true) },
            TopoName::Split { parent, by, ordinal } => TopoName::Split { parent: self.to_seed(parent), by: if *by == self.tmp { self.seed } else { *by }, ordinal: *ordinal },
            TopoName::Edge { faces: [a, b], ordinal } => TopoName::edge(self.to_seed(a), self.to_seed(b), *ordinal),
            TopoName::Vertex { faces, ordinal } => TopoName::vertex(faces.iter().map(|f| self.to_seed(f)).collect(), *ordinal),
            TopoName::Wire { feature, source } => TopoName::Wire { feature: if *feature == self.tmp { self.seed } else { *feature }, source: self.map_source(source, true) },
            TopoName::Boundary { face, ordinal } => TopoName::Boundary { face: self.to_seed(face), ordinal: *ordinal },
        })
    }

    /// Final instance name: faces created by `tmp` become
    /// `Copy { instance, of: <seed name> }` of the pattern feature.
    fn rename(&self, n: &Name) -> Name {
        if !self.contains(n) {
            return n.clone();
        }
        Arc::new(match &**n {
            TopoName::Face { feature, role } if *feature == self.tmp => {
                let of = Arc::new(TopoName::Face { feature: self.seed, role: self.map_role(role, true) });
                TopoName::Face { feature: self.pattern, role: Role::Copy { instance: self.code, of } }
            }
            TopoName::Face { feature, role } => TopoName::Face { feature: *feature, role: self.map_role(role, false) },
            TopoName::Split { parent, by, ordinal } => TopoName::Split { parent: self.rename(parent), by: if *by == self.tmp { self.pattern } else { *by }, ordinal: *ordinal },
            TopoName::Edge { faces: [a, b], ordinal } => TopoName::edge(self.rename(a), self.rename(b), *ordinal),
            TopoName::Vertex { faces, ordinal } => TopoName::vertex(faces.iter().map(|f| self.rename(f)).collect(), *ordinal),
            TopoName::Wire { feature, source } => TopoName::Wire { feature: if *feature == self.tmp { self.pattern } else { *feature }, source: self.map_source(source, false) },
            TopoName::Boundary { face, ordinal } => TopoName::Boundary { face: self.rename(face), ordinal: *ordinal },
        })
    }

    fn map_name(&self, n: &Name, seed: bool) -> Name {
        if seed { self.to_seed(n) } else { self.rename(n) }
    }
    fn map_source(&self, s: &Source, seed: bool) -> Source {
        match s {
            Source::Topo(n) => Source::Topo(self.map_name(n, seed)),
            other => other.clone(),
        }
    }
    fn map_role(&self, r: &Role, seed: bool) -> Role {
        match r {
            Role::Side(s) => Role::Side(self.map_source(s, seed)),
            Role::Profile(s) => Role::Profile(self.map_source(s, seed)),
            Role::CapStart(s) => Role::CapStart(s.as_ref().map(|s| self.map_source(s, seed))),
            Role::CapEnd(s) => Role::CapEnd(s.as_ref().map(|s| self.map_source(s, seed))),
            Role::Blend(n) => Role::Blend(self.map_name(n, seed)),
            Role::Generated(n) => Role::Generated(self.map_name(n, seed)),
            Role::Offset(n) => Role::Offset(self.map_name(n, seed)),
            Role::Copy { instance, of } => Role::Copy { instance: *instance, of: self.map_name(of, seed) },
            other => other.clone(),
        }
    }

    fn body_id(&self, id: &BodyId) -> BodyId {
        if id.feature != self.tmp {
            return id.clone();
        }
        let index = (mix(&[self.seed.0, self.code as u64, id.index as u64]) & 0x7FFF_FFFF) as u32 | 0x8000_0000;
        BodyId { feature: self.pattern, index }
    }
}

/// Sketches referenced (directly or through implicit connectors) by a
/// feature's parameters.
fn referenced_sketches(p: &crate::params::ParamValues, out: &mut BTreeSet<FeatureId>) {
    fn of_ref(r: &Reference, out: &mut BTreeSet<FeatureId>) {
        match r {
            Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } | Reference::Sketch(sketch) => {
                out.insert(*sketch);
            }
            Reference::Implicit { on, .. } => of_ref(on, out),
            _ => {}
        }
    }
    for v in p.0.values() {
        match v {
            crate::params::ParamValue::Query(q) => q.iter().for_each(|r| of_ref(r, out)),
            crate::params::ParamValue::Array(items) => items.iter().for_each(|i| referenced_sketches(i, out)),
            _ => {}
        }
    }
}

/// Sketch moved by an instance transform (frame mapped; 2D data unchanged).
fn transformed_sketch(s: &SketchResult, m: &DMat4) -> SketchResult {
    SketchResult {
        feature: s.feature,
        name: s.name.clone(),
        frame: transform_frame(&s.frame, m),
        sketch: s.sketch.clone(),
        report: s.report.clone(),
        regions: s.regions.clone(),
        wires: s.wires.clone(),
        dimension_text: s.dimension_text.clone(),
        images: s.images.clone(),
    }
}

/// Re-run the selected features for every instance (see the module docs).
fn reapply_features(ev: &mut EvalCtx, feats: &[FeatureId], instances: &[Instance]) -> FResult<()> {
    if ev.features.is_empty() {
        return err("The feature list is not available");
    }
    let me = ev.features.iter().position(|f| f.id == ev.feature);
    let mut seeds: Vec<&EffectiveFeature> = Vec::new();
    for f in feats {
        let Some(i) = ev.features.iter().position(|x| x.id == *f) else { return err("A selected feature no longer exists") };
        if me.is_some_and(|m| i >= m) {
            return err("Only features before the pattern can be patterned");
        }
        let fd = &ev.features[i];
        if fd.suppressed || fd.type_id == "sketch" {
            continue;
        }
        if super::evaluator(&fd.type_id).is_none() {
            return err(format!("{} can't be re-applied", fd.name));
        }
        seeds.push(fd);
    }
    if seeds.is_empty() {
        return err("None of the selected features can be re-applied");
    }
    // Add / Remove features: each instance's tool is made against the model
    // before the pattern (as New), then all go into one boolean. Linear in
    // the instance count; re-applying into the growing part was quadratic
    // (a ×200 rib pattern took 167 s).
    let ops: Option<Vec<String>> = seeds
        .iter()
        .map(|s| match s.params.get("operationType") {
            Some(crate::params::ParamValue::Enum(o)) if o == "ADD" || o == "REMOVE" => Some(o.clone()),
            _ => None,
        })
        .collect();
    if let Some(ops) = ops {
        return reapply_as_tools(ev, &seeds, &ops, instances);
    }
    let features = ev.features;
    let mut ok = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for inst in instances {
        if ev.cancelled() {
            return err("Cancelled");
        }
        let m = inst.t.matrix();
        let total = match &ev.xform {
            Some(outer) => m * *outer,
            None => m,
        };
        for seed in &seeds {
            let tmp = FeatureId(mix(&[ev.feature.0, seed.id.0, inst.code as u64]) | (1 << 62));
            let expanded = crate::regen::eval::expand_query_variables(&seed.params, &ev.ctx);
            let params = expanded.as_ref().unwrap_or(&seed.params);
            // Scratch context: the seed's sketches moved to the instance.
            let mut scratch = ev.ctx.clone();
            let mut sk = BTreeSet::new();
            referenced_sketches(params, &mut sk);
            for s in scratch.sketches.iter_mut() {
                if sk.contains(&s.feature) {
                    *s = Arc::new(transformed_sketch(s, &m));
                }
            }
            let mut sub = EvalCtx::new(tmp, &seed.name, params, ev.units, scratch, ev.outer_vars);
            sub.features = features;
            sub.scratch = true;
            sub.cancel = ev.cancel.clone();
            sub.xform = Some(total);
            sub.custom = ev.custom.clone();
            sub.custom_type = crate::custom::is_custom(&seed.type_id).then(|| seed.type_id.clone());
            let eval_fn = super::evaluator(&seed.type_id).expect("checked above");
            match eval_fn(&mut sub) {
                Ok(()) => {
                    ok += 1;
                    for w in sub.warnings {
                        if !ev.warnings.contains(&w) {
                            ev.warnings.push(w);
                        }
                    }
                    let rn = Renamer { tmp, seed: seed.id, pattern: ev.feature, code: inst.code };
                    merge_instance(ev, sub.ctx, &rn);
                }
                Err(e) => failures.push(e.message),
            }
        }
    }
    // patternUtils.fs: instances that fail are skipped silently; only when
    // none works the feature fails.
    if ok == 0 {
        if let Some(first) = failures.first() {
            log::debug!("feature pattern: every instance failed ({first})");
        }
        return crate::regen::eval::err_at(&["instanceFunction"], "Could not pattern selected features.");
    }
    Ok(())
}

/// [`reapply_features`] for Add / Remove seeds: every instance evaluated as
/// New against the model before the pattern; their solids, renamed like
/// re-applied instances, are then added or removed in one boolean per seed
/// (in the seed's merge scope).
fn reapply_as_tools(ev: &mut EvalCtx, seeds: &[&EffectiveFeature], ops: &[String], instances: &[Instance]) -> FResult<()> {
    let features = ev.features;
    let base = ev.ctx.clone();
    let (feature, units, outer_vars, xform, custom) = (ev.feature, ev.units, ev.outer_vars, ev.xform, ev.custom.clone());
    let cancel = ev.cancel.clone();
    // Every seed evaluated as New for one instance: per seed, its warnings
    // and the bodies it made (renamed), or its error.
    type Made = Vec<(BodyId, BodyKind, Shape, Vec<Name>)>;
    type PerSeed = Vec<Result<(Vec<String>, Made), String>>;
    let instance_body = |inst: &Instance| -> Option<PerSeed> {
        let m = inst.t.matrix();
        let total = match &xform {
            Some(outer) => m * *outer,
            None => m,
        };
        let mut out = Vec::with_capacity(seeds.len());
        for seed in seeds {
            let tmp = FeatureId(mix(&[feature.0, seed.id.0, inst.code as u64]) | (1 << 62));
            let mut params = crate::regen::eval::expand_query_variables(&seed.params, &base).unwrap_or_else(|| seed.params.clone());
            params.set("operationType", crate::params::ParamValue::Enum("NEW".into()));
            let mut scratch = base.clone();
            let mut sk = BTreeSet::new();
            referenced_sketches(&params, &mut sk);
            for s in scratch.sketches.iter_mut() {
                if sk.contains(&s.feature) {
                    *s = Arc::new(transformed_sketch(s, &m));
                }
            }
            let mut sub = EvalCtx::new(tmp, &seed.name, &params, units, scratch, outer_vars);
            sub.features = features;
            sub.scratch = true;
            sub.cancel = cancel.clone();
            sub.xform = Some(total);
            sub.custom = custom.clone();
            sub.custom_type = crate::custom::is_custom(&seed.type_id).then(|| seed.type_id.clone());
            let eval_fn = super::evaluator(&seed.type_id).expect("checked by the caller");
            out.push(match eval_fn(&mut sub) {
                Ok(()) => {
                    let rn = Renamer { tmp, seed: seed.id, pattern: feature, code: inst.code };
                    let made = sub.ctx.bodies.iter().filter(|b| b.id.feature == tmp).map(|b| (rn.body_id(&b.id), b.kind, b.shape.clone(), b.faces.iter().map(|n| rn.rename(n)).collect())).collect();
                    Ok((sub.warnings, made))
                }
                Err(e) => Err(e.message),
            });
        }
        Some(out)
    };
    let instance = |inst: &Instance| -> Option<PerSeed> {
        if cancel.as_ref().is_some_and(crate::regen::RegenCancel::aborted) {
            return None;
        }
        // On a pool thread: its booleans see the abort too.
        if let Some(c) = &cancel {
            return c.scope(|| instance_body(inst));
        }
        instance_body(inst)
    };
    // In parallel (each instance is independent; built-in features only:
    // FeatureScript evaluation stays on this thread), in instance order.
    let outs: Vec<Option<_>> = if seeds.iter().any(|s| crate::custom::is_custom(&s.type_id)) {
        instances.iter().map(instance).collect()
    } else {
        use rayon::prelude::*;
        instances.par_iter().map(instance).collect()
    };
    let mut tools: Vec<Vec<NamedShape>> = vec![Vec::new(); seeds.len()];
    let mut ok = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for out in outs {
        let Some(out) = out else { return err("Cancelled") };
        for (si, r) in out.into_iter().enumerate() {
            match r {
                Ok((warnings, made)) => {
                    ok += 1;
                    for w in warnings {
                        if !ev.warnings.contains(&w) {
                            ev.warnings.push(w);
                        }
                    }
                    for (id, kind, shape, faces) in made {
                        if kind == BodyKind::Solid {
                            tools[si].push(NamedShape::new(shape, faces));
                        } else {
                            push_body(ev, id, kind, shape, faces);
                        }
                    }
                }
                Err(e) => failures.push(e),
            }
        }
    }
    if ok == 0 {
        if let Some(first) = failures.first() {
            log::debug!("feature pattern: every instance failed ({first})");
        }
        return crate::regen::eval::err_at(&["instanceFunction"], "Could not pattern selected features.");
    }
    for (si, t) in tools.into_iter().enumerate() {
        if t.is_empty() {
            continue;
        }
        let scope = {
            let sc = EvalCtx::new(ev.feature, &seeds[si].name, &seeds[si].params, ev.units, ev.ctx.clone(), ev.outer_vars);
            super::extrude::scope(&sc)
        };
        // Instances that miss the part leave the pattern OK, without the
        // Add / Remove infos (live "kern (10-23 22:00) patterns": O3, M2).
        let n = ev.infos.len();
        ev.apply_operation(t, &ops[si], scope, BodyKind::Solid)?;
        let mut i = n;
        while i < ev.infos.len() {
            if ev.infos[i] == crate::regen::eval::NO_GEOMETRY_CHANGE || ev.infos[i] == crate::regen::eval::TOOLS_MISS_TARGETS {
                ev.infos.remove(i);
            } else {
                i += 1;
            }
        }
    }
    // Instances that failed are skipped silently (patternUtils.fs).
    if !failures.is_empty() {
        log::debug!("feature pattern: {} instance(s) failed", failures.len());
    }
    Ok(())
}

/// Take the bodies (and new mate connectors) of an instance evaluation,
/// renaming what the scratch feature created.
fn merge_instance(ev: &mut EvalCtx, sub: crate::regen::context::Context, rn: &Renamer) {
    let mut bodies = Vec::with_capacity(sub.bodies.len());
    for b in sub.bodies {
        let unchanged = ev.ctx.bodies.iter().any(|o| Arc::ptr_eq(o, &b));
        if unchanged || (b.id.feature != rn.tmp && !b.faces.iter().any(|n| rn.contains(n))) {
            bodies.push(b);
            continue;
        }
        let faces: Vec<Name> = b.faces.iter().map(|n| rn.rename(n)).collect();
        let faces = if faces.is_empty() { faces } else { dedup_names(ev, faces) };
        let nb = Body::new(rn.body_id(&b.id), b.name.clone(), b.kind, b.shape.clone(), faces, b.color, ev.feature);
        bodies.push(Arc::new(nb));
    }
    ev.ctx.bodies = bodies;
    ev.ctx.next_part = sub.next_part;
    ev.ctx.next_surface = sub.next_surface;
    ev.ctx.next_curve = sub.next_curve;
    ev.ctx.next_color = sub.next_color;
    for c in sub.connectors {
        if let Reference::MateConnector { feature, index } = &c.reference
            && *feature == rn.tmp
        {
            let idx = (mix(&[rn.seed.0, rn.code as u64, *index as u64]) & 0x7FFF_FFFF) as u32 | 0x8000_0000;
            ev.ctx.connectors.push(Connector { reference: Reference::MateConnector { feature: ev.feature, index: idx }, ..c });
        }
    }
}

// ------------------------------------------------------------------ transform / copy

fn mc_frame(c: &Connector) -> DMat4 {
    let z = c.z.normalize();
    let x = (c.x - z * c.x.dot(z)).normalize();
    let y = z.cross(x);
    DMat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), DVec4::new(c.origin.x, c.origin.y, c.origin.z, 1.0))
}

fn transform_of(ev: &EvalCtx) -> FResult<Option<k::Transform>> {
    let ty = ev.enum_("transformType").to_string();
    Ok(Some(match ty.as_str() {
        // std transformCopy.fs: two vertices, else one edge (start to end),
        // else (by distance) a face's or mate connector's direction.
        "TRANSLATION_ENTITY" | "TRANSLATION_DISTANCE" => {
            let by_distance = ty == "TRANSLATION_DISTANCE";
            let field: &'static str = if by_distance { "transformDirection" } else { "transformLine" };
            let refs = ev.query(field);
            let is_vertex = |r: &&Reference| matches!(r, Reference::Vertex(_)) || matches!(r, Reference::SketchEntity { .. } if ev.resolve_vertex_point(r).is_some());
            let pts: Vec<DVec3> = refs.iter().filter(is_vertex).filter_map(|r| ev.resolve_vertex_point(r)).collect();
            let edges: Vec<&Reference> = refs.iter().filter(|r| !is_vertex(r) && matches!(r, Reference::Edge(_) | Reference::SketchEntity { .. })).collect();
            let coincident = |v: DVec3| if v.length() <= 1e-8 { err("Two or more points are coincident.") } else { Ok(v) };
            let mut v = if pts.len() >= 2 {
                coincident(pts[1] - pts[0])?
            } else if edges.len() > 1 {
                return err_at(&[field], "Too many entities are selected.");
            } else if let Some(e) = edges.first() {
                let Some(c) = resolve_curves(ev, std::slice::from_ref(*e))?.into_iter().next() else { return err_at(&[field], "Cannot resolve entities.") };
                let (a, b) = c.shape.endpoints()?;
                coincident(b - a)?
            } else if by_distance && let Some(d) = refs.iter().find(|r| matches!(r, Reference::Face(_) | Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::MateConnector { .. } | Reference::Implicit { .. })).map(|r| ev.resolve_direction(r)) {
                d.ok_or_else(|| FeatureError { faulty: vec![field], ..fe("Translation by distance requires two vertices, an edge, a plane, or a cylindrical face.") })?
            } else if by_distance {
                return err_at(&[field], "Translation by distance requires two vertices, an edge, a plane, or a cylindrical face.");
            } else {
                return err_at(&[field], "Translation requires two vertices or an edge.");
            };
            if by_distance {
                let Some(d) = v.try_normalize() else { return err_at(&[field], "Cannot determine translation direction.") };
                let dist = ev.length("distance")?;
                k::Transform::Translate(if ev.bool("oppositeDirection") { -d * dist } else { d * dist })
            } else {
                if ev.bool("oppositeDirectionEntity") {
                    v = -v;
                }
                k::Transform::Translate(v)
            }
        }
        "TRANSLATION_3D" => k::Transform::Translate(DVec3::new(ev.length("dx")?, ev.length("dy")?, ev.length("dz")?)),
        "TRANSFORM_MATE_CONNECTORS" => {
            const NEEDS_TWO: &str = "Transform by mate connectors requires two mate connectors.";
            let base = ev.query("baseConnector").first().and_then(|r| ev.resolve_connector(r)).ok_or_else(|| fe(NEEDS_TWO))?;
            let dest = ev.query("destinationConnector").first().and_then(|r| ev.resolve_connector(r)).ok_or_else(|| fe(NEEDS_TWO))?;
            let mut to = mc_frame(&dest);
            if ev.bool("oppositeDirectionMateAxis") {
                to *= DMat4::from_rotation_x(std::f64::consts::PI);
            }
            let quarter = match ev.enum_("secondaryAxisType") {
                "PLUS_Y" => 1.0,
                "MINUS_X" => 2.0,
                "MINUS_Y" => 3.0,
                _ => 0.0,
            };
            to *= DMat4::from_rotation_z(quarter * std::f64::consts::FRAC_PI_2);
            k::Transform::Matrix(to * mc_frame(&base).inverse())
        }
        "ROTATION" => {
            // std: evAxis's own errors (its field "axis" isn't the feature's).
            let Some(r) = ev.query("transformAxis").first() else { return err("Cannot resolve entities.") };
            let (origin, axis) = ev.resolve_axis(r).ok_or_else(|| fe("Cannot determine rotation axis."))?;
            let a = ev.angle("angle")?;
            k::Transform::Rotate { origin, axis, angle: if ev.bool("oppositeDirection") { -a } else { a } }
        }
        "COPY" => return Ok(None),
        "SCALE_UNIFORMLY" => {
            let Some(r) = ev.query("scalePoint").first() else { return err_at(&["scalePoint"], "Scale requires a vertex or a mate connector.") };
            let conn = Some(r).filter(|r| matches!(r, Reference::MateConnector { .. } | Reference::Implicit { .. })).and_then(|r| ev.resolve_connector(r));
            let center = ev.resolve_vertex_point(r).ok_or_else(|| FeatureError { faulty: vec!["scalePoint"], ..fe("Scale requires a vertex or a mate connector.") })?;
            if ev.bool("uniform") {
                let s = ev.real("scale")?;
                if s <= 0.0 {
                    return err("Scale must be positive");
                }
                k::Transform::Scale { center, factor: s }
            } else {
                let f = DVec3::new(ev.real("scaleX")?, ev.real("scaleY")?, ev.real("scaleZ")?);
                if f.min_element() <= 0.0 {
                    return err("Scale factors must be positive");
                }
                let frame = conn.as_ref().map(mc_frame).unwrap_or_else(|| DMat4::from_translation(center));
                k::Transform::Matrix(frame * DMat4::from_scale(f) * frame.inverse())
            }
        }
        other => return err(format!("Unknown transform type {other}")),
    }))
}

pub fn transform(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("entities");
    let bodies = ev.resolve_bodies(refs);
    let conn_refs: Vec<Reference> = refs.iter().filter(|r| matches!(r, Reference::MateConnector { .. })).cloned().collect();
    if bodies.is_empty() && conn_refs.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Cannot resolve entities.");
    }
    let t = transform_of(ev)?;
    let copy = t.is_none() || ev.bool("makeCopy");
    let t = t.unwrap_or(k::Transform::Translate(DVec3::ZERO));
    let inst = [Instance { code: 1, t }];
    if copy {
        let copies = copy_bodies(ev, &bodies, &inst)?;
        pattern_connectors(ev, refs, &inst);
        // Live ("qa5 booleans", Copy here…): a copy keeps its source's
        // colour (a Part pattern's copies take the next colours instead).
        for (_, bj, kind, ns) in copies {
            let color = ev.ctx.bodies[bodies[bj]].color;
            let name = ev.ctx.alloc_part_name(kind);
            let f = ev.feature;
            ev.ctx.bodies.push(Arc::new(Body::new(BodyId { feature: f, index: bj as u32 }, name, kind, ns.shape, ns.names.faces, color, f)));
        }
        return Ok(());
    }
    // Mate connectors owned by a moved part move with it.
    let moved: Vec<BodyId> = bodies.iter().map(|&bi| ev.ctx.bodies[bi].id.clone()).collect();
    let mut conn_refs = conn_refs;
    for (c, owner) in &ev.ctx.connector_owners {
        if moved.contains(owner) && !conn_refs.contains(c) {
            conn_refs.push(c.clone());
        }
    }
    // Move in place: bodies keep their ids, names and face names.
    for bi in bodies {
        let b = ev.ctx.bodies[bi].clone();
        // Moved, not copied: the body keeps its geometry at a new location.
        let res = k::transform(&b.shape, &t, false).map_err(kerr("Transform failed"))?;
        let mut faces: Vec<Option<Name>> = vec![None; res.shape.count(ShapeType::Face)];
        for r in &res.history.records {
            if r.arg == 0 && r.input_kind == ShapeType::Face && r.output_kind == ShapeType::Face
                && let (Some(slot), Some(n)) = (faces.get_mut(r.output_index as usize), b.faces.get(r.input_index as usize))
            {
                slot.get_or_insert(n.clone());
            }
        }
        let faces: Vec<Name> = faces.into_iter().enumerate().map(|(j, n)| n.or_else(|| b.faces.get(j).cloned()).unwrap_or_else(|| Arc::new(TopoName::face(ev.feature, Role::New(j as u32))))).collect();
        let nb = Body::new(b.id.clone(), b.name.clone(), b.kind, res.shape, faces, b.color, ev.feature);
        ev.ctx.bodies[bi] = Arc::new(nb);
    }
    for r in conn_refs {
        if let Some(c) = ev.ctx.connectors.iter_mut().find(|c| c.reference == r) {
            let (o, x, z) = transform_connector(c, &t);
            c.origin = o;
            c.x = x;
            c.z = z;
        }
    }
    Ok(())
}

pub fn copy_part(ev: &mut EvalCtx) -> FResult<()> {
    let bodies = ev.resolve_bodies(ev.query("entities"));
    if bodies.is_empty() {
        return err("Select parts to copy");
    }
    let inst = [Instance { code: 1, t: k::Transform::Translate(DVec3::ZERO) }];
    for (_, bj, kind, ns) in copy_bodies(ev, &bodies, &inst)? {
        let color = ev.ctx.bodies[bodies[bj]].color;
        push_body_colored(ev, BodyId { feature: ev.feature, index: bj as u32 }, kind, ns.shape, ns.names.faces, color);
    }
    Ok(())
}
