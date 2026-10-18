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
//! * Feature pattern: evaluators only see the current context (not other
//!   features' definitions), so features are patterned through their
//!   geometry: bodies created by the feature are copied as new parts, and
//!   the faces the feature left on other bodies are face-patterned as above.
//!   "Reapply features" is therefore not available (a warning is shown).
//!   Features are processed in selection order.
//!
//! Naming: every face of an instance is `Copy { instance, of }` where
//! `instance` encodes the instance's grid index relative to the seed (zig-zag
//! encoded, second direction in the high 16 bits), so references to an
//! instance survive upstream edits and count changes.

use super::helpers_a::*;
use crate::display::BodyKind;
use crate::naming::Name;
use crate::reference::{BodyId, FeatureId, Reference, Role, TopoName};
use crate::regen::context::{Body, Connector};
use crate::regen::eval::{EvalCtx, FResult, NamedShape, err, kernel_plane};
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
    let Some(r1) = ev.query("directionOne").first() else { return err("Select a pattern direction") };
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
    let Some(r) = ev.query("axis").first() else { return err("Select a pattern axis") };
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
    if orient == "LOCK_FACES" {
        return err("Curve pattern orientation \"Normal to face\" is not supported yet");
    }
    let (p0, t0) = path.at(0.0);
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
    let inst = linear_instances(ev)?;
    run_pattern(ev, inst)
}

pub fn circular_pattern(ev: &mut EvalCtx) -> FResult<()> {
    let inst = circular_instances(ev)?;
    run_pattern(ev, inst)
}

pub fn curve_pattern(ev: &mut EvalCtx) -> FResult<()> {
    let inst = curve_instances(ev)?;
    run_pattern(ev, inst)
}

pub fn mirror(ev: &mut EvalCtx) -> FResult<()> {
    let Some(r) = ev.query("mirrorPlane").first() else { return err("Select a mirror plane") };
    let frame = ev.resolve_plane(r).ok_or_else(|| fe("The mirror plane could not be resolved"))?;
    run_pattern(ev, vec![Instance { code: 1, t: k::Transform::Mirror(kernel_plane(&frame)) }])
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
        return err("Select parts to pattern");
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
    let feats: Vec<FeatureId> = ev.query("instanceFunction").iter().filter_map(|r| if let Reference::Feature(f) = r { Some(*f) } else { None }).collect();
    if feats.is_empty() {
        return err("Select features to pattern");
    }
    if ev.bool("fullFeaturePattern") {
        ev.warnings.push("Reapply features is not supported yet; the features' geometry was patterned instead".into());
    }
    if instances.is_empty() {
        return Ok(());
    }
    for f in feats {
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
        return err("Select parts or mate connectors to transform");
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
