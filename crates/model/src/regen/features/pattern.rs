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
use crate::regen::eval::{EffectiveFeature, EvalCtx, FResult, NamedShape, err, kernel_plane, transform_frame};
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

fn index_range(n: i64, centered: bool) -> std::ops::RangeInclusive<i64> {
    if centered { -(n - 1)..=(n - 1) } else { 0..=(n - 1) }
}

fn code2(i: i64, j: i64) -> u32 {
    zigzag(i) | (zigzag(j) << 16)
}

fn linear_instances(ev: &EvalCtx) -> FResult<Vec<Instance>> {
    let Some(r1) = ev.query("directionOne").first() else { return crate::regen::eval::err_at(&["directionOne"], "Select a pattern direction.") };
    let mut d1 = ev.resolve_direction(r1).ok_or_else(|| fe("The pattern direction could not be resolved"))?.normalize();
    if ev.bool("oppositeDirection") {
        d1 = -d1;
    }
    let s1 = ev.length("distance")?;
    let n1 = ev.integer("instanceCount")?.max(1);
    let (d2, s2, n2, c2) = if ev.bool("hasSecondDir") {
        let Some(r2) = ev.query("directionTwo").first() else { return err("Select the second pattern direction") };
        let mut d = ev.resolve_direction(r2).ok_or_else(|| fe("The second pattern direction could not be resolved"))?.normalize();
        if ev.bool("oppositeDirectionTwo") {
            d = -d;
        }
        (d, ev.length("distanceTwo")?, ev.integer("instanceCountTwo")?.max(1), ev.bool("isCenteredTwo"))
    } else {
        (DVec3::ZERO, 0.0, 1, false)
    };
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
    let (origin, mut axis) = ev.resolve_axis(r).ok_or_else(|| fe("The pattern axis could not be resolved"))?;
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
    let mut cum = vec![0.0];
    for w in chain.windows(2) {
        let l = cum.last().copied().unwrap_or(0.0) + w[0].distance(w[1]);
        cum.push(l);
    }
    if chain.len() < 2 {
        return err("The pattern path is degenerate");
    }
    Ok(Path { pts: chain, cum })
}

fn curve_instances(ev: &EvalCtx) -> FResult<Vec<Instance>> {
    let edges = resolve_curves(ev, ev.query("edges"))?;
    if edges.is_empty() {
        return err("Select a path to pattern along");
    }
    let path = chain_path(&edges)?;
    let len = path.length();
    let n = ev.integer("instanceCount")?.max(1);
    let closed = path.pts.first().zip(path.pts.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-6);
    let spacing = if ev.is("spacingType", "DISTANCE") {
        let d = ev.length("distance")?;
        if d * (n - 1) as f64 > len + 1e-9 {
            return err("The path is too short for the instance count and distance");
        }
        d
    } else if closed {
        len / n as f64
    } else if n > 1 {
        len / (n - 1) as f64
    } else {
        0.0
    };
    let orient = ev.enum_("orientationType").to_string();
    // Normal to face: each instance's frame is (curve tangent, the faces' normal
    // made perpendicular to it), mapped from the frame at the seed.
    let faces: Vec<k::Shape> = if orient == "LOCK_FACES" {
        let f: Vec<k::Shape> = ev.resolve_faces(ev.query("lockedFaces")).into_iter().filter_map(|(bi, fi)| ev.ctx.bodies[bi].shape.sub_shape(ShapeType::Face, fi).ok()).collect();
        if f.is_empty() {
            return err("Select faces to be normal to");
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
    Ok(out)
}

// ------------------------------------------------------------------ entry points

pub fn linear_pattern(ev: &mut EvalCtx) -> FResult<()> {
    check_seeds(ev, "pattern")?;
    let inst = linear_instances(ev)?;
    run_pattern(ev, inst)
}

pub fn circular_pattern(ev: &mut EvalCtx) -> FResult<()> {
    check_seeds(ev, "pattern")?;
    let inst = circular_instances(ev)?;
    run_pattern(ev, inst)
}

pub fn curve_pattern(ev: &mut EvalCtx) -> FResult<()> {
    let inst = curve_instances(ev)?;
    run_pattern(ev, inst)
}

pub fn mirror(ev: &mut EvalCtx) -> FResult<()> {
    check_seeds(ev, "mirror")?;
    let Some(r) = ev.query("mirrorPlane").first() else { return crate::regen::eval::err_at(&["mirrorPlane"], "Select a mirror plane.") };
    let frame = ev.resolve_plane(r).ok_or_else(|| fe("The mirror plane could not be resolved"))?;
    run_pattern(ev, vec![Instance { code: 1, t: k::Transform::Mirror(kernel_plane(&frame)) }])
}

/// Onshape checks what is patterned before the direction / axis / plane:
/// an empty dialog says "Select parts to pattern." (or mirror).
fn check_seeds(ev: &EvalCtx, verb: &str) -> FResult<()> {
    match ev.enum_("patternType") {
        "FEATURE" => Ok(()),
        "FACE" if ev.query("faces").is_empty() => crate::regen::eval::err_at(&["faces"], format!("Select faces to {verb}.")),
        "FACE" => Ok(()),
        _ if ev.query("entities").is_empty() => crate::regen::eval::err_at(&["entities"], format!("Select parts to {verb}.")),
        _ => Ok(()),
    }
}

fn run_pattern(ev: &mut EvalCtx, instances: Vec<Instance>) -> FResult<()> {
    match ev.enum_("patternType") {
        "FEATURE" => feature_pattern(ev, &instances),
        "FACE" => {
            let groups = by_body(ev.resolve_faces(ev.query("faces")));
            if groups.is_empty() {
                return err("Select faces to pattern");
            }
            if instances.is_empty() {
                return Ok(());
            }
            let groups: Vec<(BodyId, Vec<usize>)> = groups.into_iter().map(|(bi, fs)| (ev.ctx.bodies[bi].id.clone(), fs)).collect();
            for (id, fs) in groups {
                pattern_faces(ev, &id, &fs, &instances)?;
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
fn copy_bodies(ev: &EvalCtx, bodies: &[usize], instances: &[Instance]) -> FResult<Vec<(usize, usize, BodyKind, NamedShape)>> {
    let mut out = Vec::new();
    for (ii, inst) in instances.iter().enumerate() {
        for (bj, &bi) in bodies.iter().enumerate() {
            let b = &ev.ctx.bodies[bi];
            let res = k::transform(&b.shape, &inst.t, true).map_err(kerr("Pattern failed"))?;
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
    let copies = copy_bodies(ev, &bodies, instances)?;
    pattern_connectors(ev, refs, instances);
    let op = ev.enum_("operationType").to_string();
    if op == "NEW" || copies.iter().all(|c| c.2 != BodyKind::Solid) {
        let nb = bodies.len().max(1);
        for (ii, bj, kind, ns) in copies {
            push_body(ev, BodyId { feature: ev.feature, index: (ii * nb + bj) as u32 }, kind, ns.shape, ns.names.faces);
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
    let solid = match sewn.shape.solids().into_iter().next() {
        Some(s) if sewn.shape.count(ShapeType::Solid) == 1 => s,
        _ => return err("These faces can't be patterned (they don't bound a closed volume)"),
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

fn pattern_faces(ev: &mut EvalCtx, body_id: &BodyId, faces: &[usize], instances: &[Instance]) -> FResult<()> {
    let Some(bi) = index_of_body(ev, body_id) else { return Ok(()) };
    let body = ev.ctx.bodies[bi].clone();
    let tool = faces_tool(ev, &body, faces)?;
    let mut tools: Vec<NamedShape> = Vec::new();
    for inst in instances {
        let res = k::transform(&tool.shape, &inst.t, true).map_err(kerr("Pattern failed"))?;
        let names = copy_names(ev, &tool.names, &res, inst.code);
        tools.push(NamedShape::new(res.shape, names));
    }
    apply_tools(ev, body_id, tools, tool.add, body.kind)
}

/// Fuse (boss) or cut (pocket) instance tools into the parts they touch.
fn apply_tools(ev: &mut EvalCtx, seed: &BodyId, tools: Vec<NamedShape>, add: bool, kind: BodyKind) -> FResult<()> {
    if add {
        let Some(bi) = index_of_body(ev, seed) else { return Ok(()) };
        let seed_shape = ev.ctx.bodies[bi].shape.clone();
        let (touch, free): (Vec<NamedShape>, Vec<NamedShape>) = tools.into_iter().partition(|t| k::distance(&seed_shape, &t.shape).map(|d| d.distance < 1e-7).unwrap_or(false));
        if !touch.is_empty() {
            ev.boolean_into(&[bi], touch, k::BooleanOp::Fuse)?;
        }
        for (i, t) in free.into_iter().enumerate() {
            push_body(ev, BodyId { feature: ev.feature, index: 30_000 + i as u32 }, kind, t.shape, t.names.faces);
        }
        return Ok(());
    }
    let mut targets = ev.auto_scope(&tools);
    if let Some(bi) = index_of_body(ev, seed)
        && !targets.contains(&bi)
    {
        targets.push(bi);
    }
    targets.sort_unstable();
    ev.boolean_into(&targets, tools, k::BooleanOp::Cut)
}

fn feature_pattern(ev: &mut EvalCtx, instances: &[Instance]) -> FResult<()> {
    let mut feats: Vec<FeatureId> = ev.query("instanceFunction").iter().filter_map(|r| if let Reference::Feature(f) = r { Some(*f) } else { None }).collect();
    if feats.is_empty() {
        return err("Select features to pattern");
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
    if ev.bool("fullFeaturePattern") {
        match reapply_features(ev, &feats, instances) {
            Ok(()) => return Ok(()),
            Err(e) => {
                ev.ctx = before.clone();
                ev.warnings.truncate(warnings_before);
                let r = geometric_feature_pattern(ev, &feats, instances);
                if r.is_err() {
                    return Err(e);
                }
                ev.warnings.push(format!("Reapply features failed ({}); the features' geometry was patterned instead", e.message));
                return r;
            }
        }
    }
    match geometric_feature_pattern(ev, &feats, instances) {
        Ok(()) => Ok(()),
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
                push_body(ev, BodyId { feature: ev.feature, index }, kind, ns.shape, ns.names.faces);
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
        for (id, fs) in groups {
            found = true;
            pattern_faces(ev, &id, &fs, instances)?;
        }
        if !found {
            return err("A selected feature has no geometry to pattern");
        }
    }
    Ok(())
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
    let features = ev.features;
    let mut ok = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for inst in instances {
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
    if ok == 0 {
        return Err(fe(failures.into_iter().next().unwrap_or_else(|| "Pattern failed".into())));
    }
    if !failures.is_empty() {
        let n = failures.len();
        let first = failures.into_iter().next().unwrap_or_default();
        ev.warnings.push(format!("{n} pattern instance(s) failed: {first}"));
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
        "TRANSLATION_ENTITY" => {
            let refs = ev.query("transformLine");
            let pts: Vec<DVec3> = refs.iter().filter_map(|r| ev.resolve_vertex_point(r)).collect();
            let mut v = if pts.len() >= 2 {
                pts[1] - pts[0]
            } else if let Some(e) = resolve_curves(ev, refs)?.first() {
                let (a, b) = e.shape.endpoints()?;
                b - a
            } else {
                return err("Select a line or two points");
            };
            if ev.bool("oppositeDirectionEntity") {
                v = -v;
            }
            if v.length() < 1e-12 {
                return err("The points are coincident");
            }
            k::Transform::Translate(v)
        }
        "TRANSLATION_DISTANCE" => {
            let refs = ev.query("transformDirection");
            let pts: Vec<DVec3> = refs.iter().filter(|r| matches!(r, Reference::Vertex(_) | Reference::SketchEntity { .. })).filter_map(|r| ev.resolve_vertex_point(r)).collect();
            let d = if pts.len() >= 2 {
                pts[1] - pts[0]
            } else {
                let Some(r) = refs.first() else { return err("Select a direction") };
                ev.resolve_direction(r).ok_or_else(|| fe("The direction could not be resolved"))?
            };
            let Some(d) = d.try_normalize() else { return err("The points are coincident") };
            let dist = ev.length("distance")?;
            k::Transform::Translate(if ev.bool("oppositeDirection") { -d * dist } else { d * dist })
        }
        "TRANSLATION_3D" => k::Transform::Translate(DVec3::new(ev.length("dx")?, ev.length("dy")?, ev.length("dz")?)),
        "TRANSFORM_MATE_CONNECTORS" => {
            let base = ev.query("baseConnector").first().and_then(|r| ev.resolve_connector(r)).ok_or_else(|| fe("Select the mate connector to transform from"))?;
            let dest = ev.query("destinationConnector").first().and_then(|r| ev.resolve_connector(r)).ok_or_else(|| fe("Select the mate connector to transform to"))?;
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
            let Some(r) = ev.query("transformAxis").first() else { return err("Select a rotation axis") };
            let (origin, axis) = ev.resolve_axis(r).ok_or_else(|| fe("The rotation axis could not be resolved"))?;
            let a = ev.angle("angle")?;
            k::Transform::Rotate { origin, axis, angle: if ev.bool("oppositeDirection") { -a } else { a } }
        }
        "COPY" => return Ok(None),
        "SCALE_UNIFORMLY" => {
            let pr = ev.query("scalePoint").first();
            let conn = pr.filter(|r| matches!(r, Reference::MateConnector { .. } | Reference::Implicit { .. })).and_then(|r| ev.resolve_connector(r));
            let center = match pr {
                Some(r) => ev.resolve_vertex_point(r).ok_or_else(|| fe("The scale point could not be resolved"))?,
                None => DVec3::ZERO,
            };
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
        for (_, bj, kind, ns) in copies {
            push_body(ev, BodyId { feature: ev.feature, index: bj as u32 }, kind, ns.shape, ns.names.faces);
        }
        return Ok(());
    }
    // Move in place: bodies keep their ids, names and face names.
    for bi in bodies {
        let b = ev.ctx.bodies[bi].clone();
        let res = k::transform(&b.shape, &t, true).map_err(kerr("Transform failed"))?;
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
        push_body(ev, BodyId { feature: ev.feature, index: bj as u32 }, kind, ns.shape, ns.names.faces);
    }
    Ok(())
}
