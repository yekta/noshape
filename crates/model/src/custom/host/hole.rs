//! `@opHole` (std geomOperations.fs opHole): revolved hole tools built from
//! a `HoleDefinition`'s profiles along each axis, placed against where the
//! hole cylinder enters and leaves the targets, and subtracted from them;
//! plus the `qOpHoleFace` / `qOpHoleProfile` queries over what it made.
//!
//! The hole cylinder's entrances and exits are found by casting rays along
//! the axis and around the cylinder of the largest profile radius: the
//! first and last ray to enter a target give its `firstEntrance` /
//! `fullEntrance`, and likewise for the exits.

use super::query::{Ent, QueryCx, evaluate_refs};
use super::{context_arg, convert::*, run_op};
use crate::display::BodyKind;
use crate::reference::{BodyId, Reference, Role, Source, TopoName};
use crate::regen::eval::{EvalCtx, FResult, NamedShape};
use crate::regen::features::helpers_a::{perp, push_body, ray_intervals};
use glam::{DVec2, DVec3};
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::Value;
use noshape_kernel as k;
use std::collections::BTreeMap;
use std::sync::Arc;

pub const NATIVES: &[(&str, NativeFn)] = &[("opHole", op_hole)];

const TOL: f64 = 1e-8;
/// Rays around the hole cylinder (plus one on the axis).
const RAYS: usize = 24;

/// One hole an `opHole` built, kept for `qOpHoleFace` / `qOpHoleProfile`.
#[derive(Clone, Debug)]
pub struct HoleRecord {
    /// Index of the axis (the faces' names carry it).
    pub index: u32,
    /// The `identities` entry of the hole.
    pub identity: Vec<Reference>,
    pub origin: DVec3,
    pub dir: DVec3,
    /// Profiles as placed (collapsed ones removed).
    pub profiles: Vec<Placed>,
    /// `HoleDefinition.faceNames`, by profile index.
    pub face_names: Vec<Option<Arc<str>>>,
}

#[derive(Clone, Debug)]
pub struct Placed {
    /// Index in `HoleDefinition.profiles`.
    pub index: usize,
    /// Distance from the axis point along the axis.
    pub position: f64,
    pub radius: f64,
    pub name: Option<Arc<str>>,
}

#[derive(Clone, Debug)]
struct Profile {
    reference: Arc<str>,
    before: bool,
    matched: bool,
    position: f64,
    radius: f64,
    must_differ: bool,
    not_first: bool,
    name: Option<Arc<str>>,
}

fn profiles_of(i: &mut Interp, def: &Value) -> R<Vec<Profile>> {
    let Some(arr) = def.get("holeDefinition").get("profiles").as_array() else { return Err(i.error("opHole: holeDefinition.profiles should be an array")) };
    let mut out = Vec::new();
    for p in arr.iter() {
        let reference = def_enum(p, "positionReference").unwrap_or_else(|| "AXIS_POINT".into());
        let matched = def_enum(p, "profileType").is_some_and(|t| &*t == "MATCHED");
        out.push(Profile {
            reference,
            before: def_bool(p, "beforeReference", false),
            matched,
            position: length_of(p.get("position")).unwrap_or(0.0),
            radius: length_of(p.get("radius")).ok_or_else(|| i.error("opHole: a profile radius should be a length"))?,
            must_differ: def_bool(p, "targetMustDifferFromPrevious", false),
            not_first: def_bool(p, "notApplicableForFirstTarget", false),
            name: p.get("name").as_str(),
        });
    }
    if out.is_empty() {
        return Err(i.error("opHole: holeDefinition has no profiles"));
    }
    Ok(out)
}

/// Where the hole cylinder meets one target (distances along the axis).
#[derive(Clone, Debug)]
struct Extremes {
    body: BodyId,
    first_entrance: f64,
    full_entrance: f64,
    first_exit: f64,
    full_exit: f64,
}

fn extremes(ev: &EvalCtx, targets: &[usize], o: DVec3, d: DVec3, radius: f64) -> Vec<Extremes> {
    let len = ev.model_extent() * 2.0 + o.length() + 1.0;
    let start = o - d * len;
    let x = perp(d).normalize();
    let y = d.cross(x);
    let mut offsets = vec![DVec3::ZERO];
    if radius > TOL {
        let r = radius * (1.0 - 1e-7);
        offsets.extend((0..RAYS).map(|n| {
            let a = std::f64::consts::TAU * n as f64 / RAYS as f64;
            (x * a.cos() + y * a.sin()) * r
        }));
    }
    let mut out = Vec::new();
    for &bi in targets {
        let shape = &ev.ctx.bodies[bi].shape;
        let (mut entr, mut exits) = (Vec::new(), Vec::new());
        for off in &offsets {
            // Intersections fully behind the axis point don't count.
            let iv: Vec<(f64, f64)> = ray_intervals(shape, start + *off, d, len * 2.0).into_iter().map(|(a, b)| (a - len, b - len)).filter(|(_, b)| *b > TOL).collect();
            if let (Some(f), Some(l)) = (iv.first(), iv.last()) {
                // Live: an axis point inside the part enters at 0.
                entr.push(f.0.max(0.0));
                exits.push(l.1);
            }
        }
        if entr.is_empty() {
            continue;
        }
        let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
        let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        out.push(Extremes { body: ev.ctx.bodies[bi].id.clone(), first_entrance: min(&entr), full_entrance: max(&entr), first_exit: min(&exits), full_exit: max(&exits) });
    }
    out.sort_by(|a, b| a.first_entrance.total_cmp(&b.first_entrance));
    out
}

/// A position reference's root range and target.
#[derive(Clone, Debug, PartialEq)]
struct Root {
    start: f64,
    end: f64,
    target: Option<BodyId>,
}

fn root_of(reference: &str, ex: &[Extremes], end_bound: Option<(f64, f64)>) -> Option<Root> {
    let first = ex.first();
    let at = |e: &Extremes, entrance: bool| {
        let (start, end) = if entrance { (e.first_entrance, e.full_entrance) } else { (e.first_exit, e.full_exit) };
        Root { start, end, target: Some(e.body.clone()) }
    };
    match reference {
        "AXIS_POINT" => Some(Root { start: 0.0, end: 0.0, target: None }),
        "TARGET_START" => first.map(|e| at(e, true)),
        "LAST_TARGET_START" => ex.iter().max_by(|a, b| a.first_entrance.total_cmp(&b.first_entrance)).map(|e| at(e, true)),
        "LAST_TARGET_END" => ex.iter().max_by(|a, b| a.full_exit.total_cmp(&b.full_exit)).map(|e| at(e, false)),
        "UP_TO_NEXT" => first.map(|e| at(e, false)),
        "UP_TO_ENTITY" => end_bound.map(|(start, end)| Root { start, end, target: None }),
        _ => None,
    }
}

/// Distance range along the axis where the hole cylinder meets the end bound.
fn end_bound_range(ev: &EvalCtx, refs: &[Reference], o: DVec3, d: DVec3, radius: f64) -> FResult<Option<(f64, f64)>> {
    let Some(r) = refs.first() else { return Ok(None) };
    if let Some(p) = ev.resolve_vertex_point(r) {
        let t = (p - o).dot(d);
        return Ok(Some((t, t)));
    }
    if let Some(f) = ev.resolve_plane(r) {
        let den = d.dot(f.normal);
        if den.abs() < 1e-12 {
            return Ok(None);
        }
        let t = (f.origin - o).dot(f.normal) / den;
        let spread = radius * (1.0 - den * den).max(0.0).sqrt() / den.abs();
        return Ok(Some((t - spread, t + spread)));
    }
    if let Reference::Face(_) = r
        && let Some((bi, fi)) = ev.resolve_faces(std::slice::from_ref(r)).first().copied()
    {
        let face = ev.face_shape(bi, fi)?;
        let len = ev.model_extent() * 2.0 + o.length() + 1.0;
        let line = k::make_line(o - d * len, o + d * len).map_err(|e| crate::regen::eval::FeatureError::new(e.to_string()))?;
        let sec = k::section(&line, &face).map_err(|e| crate::regen::eval::FeatureError::new(e.to_string()))?;
        let ts: Vec<f64> = sec.shape.vertices().iter().filter_map(|v| v.vertex_point().ok()).map(|p| (p - o).dot(d)).collect();
        let t = ts.iter().copied().filter(|t| *t > -TOL).fold(f64::INFINITY, f64::min);
        if t.is_finite() {
            return Ok(Some((t, t)));
        }
    }
    Ok(None)
}

/// Why a hole was not built.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Skip {
    NoTargets,
    TargetsDoNotDiffer,
    Construction,
}

/// What one hole produced.
struct Built {
    tool: NamedShape,
    record: HoleRecord,
    roots: BTreeMap<Arc<str>, Root>,
    extremes: Vec<Extremes>,
    depth: f64,
    /// Targets the tool reaches.
    cuts: Vec<BodyId>,
}

/// What every hole of one `opHole` shares.
struct HoleInput<'a> {
    profiles: &'a [Profile],
    face_names: &'a [Option<Arc<str>>],
    targets: &'a [usize],
    end_refs: &'a [Reference],
}

/// A profile placed on the axis, with the target of its reference.
struct Slot {
    index: usize,
    position: f64,
    radius: f64,
    name: Option<Arc<str>>,
    target: Option<BodyId>,
}

fn build_hole(ev: &EvalCtx, input: &HoleInput, index: u32, (o, d): (DVec3, DVec3), identity: Vec<Reference>) -> FResult<Result<Built, Skip>> {
    let HoleInput { profiles, face_names, targets, end_refs } = *input;
    // Live (dom/fs-ophole.txt, slanted entrance): the depth extremes are
    // those of the smallest (shaft) cylinder; each reference's root range
    // is measured with the largest radius of the profiles placed on it.
    let shaft = profiles.iter().map(|p| p.radius).filter(|r| *r > TOL).fold(f64::INFINITY, f64::min);
    let shaft = if shaft.is_finite() { shaft } else { 0.0 };
    let ex = extremes(ev, targets, o, d, shaft);
    let ref_radius = |name: &str| profiles.iter().filter(|p| &*p.reference == name).map(|p| p.radius).fold(0.0, f64::max);
    let mut cache: Vec<(f64, Vec<Extremes>)> = vec![(shaft, ex.clone())];
    let mut extremes_at = |r: f64| -> Vec<Extremes> {
        if let Some((_, e)) = cache.iter().find(|(x, _)| (x - r).abs() < TOL) {
            return e.clone();
        }
        let e = extremes(ev, targets, o, d, r);
        cache.push((r, e.clone()));
        e
    };
    let mut roots: BTreeMap<Arc<str>, Root> = BTreeMap::new();
    let mut placed: Vec<Slot> = Vec::new();
    // Every reference but LAST_TARGET_START_IN_DEPTH first (that one
    // depends on the depth of the others).
    for (n, p) in profiles.iter().enumerate() {
        if &*p.reference == "LAST_TARGET_START_IN_DEPTH" {
            continue;
        }
        let r = ref_radius(&p.reference);
        let end_bound = end_bound_range(ev, end_refs, o, d, r)?;
        let Some(root) = root_of(&p.reference, &extremes_at(r), end_bound) else { return Ok(Err(Skip::NoTargets)) };
        let base = if p.matched {
            if p.reference.ends_with("_END") { root.end } else { root.start }
        } else if p.before {
            root.start
        } else {
            root.end
        };
        placed.push(Slot { index: n, position: base + if p.matched { 0.0 } else { p.position }, radius: p.radius, name: p.name.clone(), target: root.target.clone() });
        roots.insert(p.reference.clone(), root);
    }
    if profiles.iter().any(|p| &*p.reference == "LAST_TARGET_START_IN_DEPTH") {
        let depth = placed.iter().map(|x| x.position).fold(f64::NEG_INFINITY, f64::max);
        let at = extremes_at(ref_radius("LAST_TARGET_START_IN_DEPTH"));
        let root = at.iter().filter(|e| e.first_entrance < depth - TOL).max_by(|a, b| a.first_entrance.total_cmp(&b.first_entrance)).map(|e| Root { start: e.first_entrance, end: e.full_entrance, target: Some(e.body.clone()) });
        let first = ex.first().map(|e| e.body.clone());
        let applicable = |p: &Profile| root.as_ref().is_some_and(|r| !(p.not_first && r.target == first));
        let usable = profiles.iter().filter(|p| &*p.reference == "LAST_TARGET_START_IN_DEPTH").all(applicable);
        if let (Some(root), true) = (root, usable) {
            for (n, p) in profiles.iter().enumerate() {
                if &*p.reference != "LAST_TARGET_START_IN_DEPTH" {
                    continue;
                }
                let base = if p.matched || p.before { root.start } else { root.end };
                placed.push(Slot { index: n, position: base + if p.matched { 0.0 } else { p.position }, radius: p.radius, name: p.name.clone(), target: root.target.clone() });
            }
            roots.insert("LAST_TARGET_START_IN_DEPTH".into(), root);
        }
        placed.sort_by_key(|x| x.index);
    }
    // targetMustDifferFromPrevious: the previous profile's reference must
    // have another target.
    for w in placed.windows(2) {
        if profiles[w[1].index].must_differ && w[1].target == w[0].target {
            return Ok(Err(Skip::TargetsDoNotDiffer));
        }
    }
    // Identical neighbours collapse into the first.
    let mut kept: Vec<Placed> = Vec::new();
    for slot in placed {
        if let Some(last) = kept.last()
            && (last.position - slot.position).abs() < TOL
            && (last.radius - slot.radius).abs() < TOL
        {
            continue;
        }
        kept.push(Placed { index: slot.index, position: slot.position, radius: slot.radius, name: slot.name });
    }
    // Profile polyline in (radius, depth); segment k ends at profile k.
    let mut pts: Vec<DVec2> = vec![DVec2::new(0.0, kept[0].position)];
    let mut segments: Vec<(DVec2, DVec2, usize)> = Vec::new();
    for (n, p) in kept.iter().enumerate() {
        let q = DVec2::new(p.radius, p.position);
        let prev = *pts.last().unwrap_or(&q);
        if prev.distance(q) > TOL {
            segments.push((prev, q, if n == 0 { 0 } else { p.index }));
            pts.push(q);
        }
    }
    if pts.len() < 3 {
        return Ok(Err(Skip::Construction));
    }
    let x = perp(d).normalize();
    let frame = k::Plane::new(o, -d, x);
    let Ok(shape) = k::revolve_profile_tool(&pts, &frame) else { return Ok(Err(Skip::Construction)) };
    let names: Vec<_> = shape
        .faces()
        .iter()
        .map(|f| {
            let at = |p: DVec2| o + x * p.x + d * p.y;
            let seg = segments
                .iter()
                .map(|(a, b, n)| (f.project_point(at((*a + *b) * 0.5)).map(|pr| (pr.point - at((*a + *b) * 0.5)).length()).unwrap_or(f64::INFINITY), *n))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map_or(0, |x| x.1);
            Arc::new(TopoName::face(ev.feature, Role::Side(Source::Index((index << 16) | seg as u32))))
        })
        .collect();
    let deepest = kept.iter().map(|p| p.position).fold(f64::NEG_INFINITY, f64::max);
    let cuts = ex.iter().filter(|e| e.first_entrance < deepest - TOL).map(|e| e.body.clone()).collect();
    // Live: 0 for holes ending on the axis point or the first target; else
    // from the first entrance to the last profile with a radius.
    let last_ref = profiles.last().map(|p| p.reference.clone()).unwrap_or_else(|| "AXIS_POINT".into());
    let start = ex.iter().map(|e| e.first_entrance).fold(f64::INFINITY, f64::min);
    let depth = if matches!(&*last_ref, "AXIS_POINT" | "TARGET_START") || !start.is_finite() { 0.0 } else { kept.iter().rev().find(|p| p.radius > TOL).map_or(0.0, |p| p.position) - start };
    Ok(Ok(Built {
        tool: NamedShape::new(shape, names),
        record: HoleRecord { index, identity, origin: o, dir: d, profiles: kept, face_names: face_names.to_vec() },
        roots,
        extremes: ex,
        depth,
        cuts,
    }))
}

fn op_hole(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let def = def_arg(i, &a, 2)?;
    let profiles = profiles_of(i, &def)?;
    let face_names: Vec<Option<Arc<str>>> = match def.get("holeDefinition").get("faceNames").as_array() {
        Some(a) => a.iter().map(|v| v.as_str()).collect(),
        None => vec![None; profiles.len()],
    };
    let Some(axes) = def.get("axes").as_array() else { return Err(i.error("opHole: axes should be an array of Lines")) };
    let mut lines = Vec::new();
    for ax in axes.iter() {
        lines.push(line_of(ax).ok_or_else(|| i.error("opHole: axes should be an array of Lines"))?);
    }
    let mut identities: Vec<Vec<Reference>> = Vec::new();
    if let Some(ids) = def.get("identities").as_array() {
        for q in ids.iter() {
            identities.push(evaluate_refs(i, &cx, q)?);
        }
    }
    let target_refs = evaluate_refs(i, &cx, def.get("targets"))?;
    let exclude_refs = evaluate_refs(i, &cx, def.get("targetsToExcludeFromSubtraction"))?;
    let end_refs = evaluate_refs(i, &cx, def.get("endBoundEntity"))?;
    let subtract = def_bool(&def, "subtractFromTargets", true);
    let keep = !subtract || def_bool(&def, "keepTools", false);
    let results = run_op(i, &cx, &id, "HOLE_CUT_FAIL", |ev| {
        let targets: Vec<usize> = ev.resolve_bodies(&target_refs).into_iter().filter(|b| ev.ctx.bodies[*b].kind == BodyKind::Solid).collect();
        let excluded: Vec<BodyId> = ev.resolve_bodies(&exclude_refs).into_iter().map(|b| ev.ctx.bodies[b].id.clone()).collect();
        let input = HoleInput { profiles: &profiles, face_names: &face_names, targets: &targets, end_refs: &end_refs };
        let mut out = Vec::new();
        for (n, axis) in lines.iter().enumerate() {
            out.push(build_hole(ev, &input, n as u32, *axis, identities.get(n).cloned().unwrap_or_default())?);
        }
        if !out.iter().any(|r| r.is_ok()) {
            let all_differ = out.iter().all(|r| matches!(r, Err(Skip::TargetsDoNotDiffer)));
            return Err(super::code_error_faulty(if all_differ { "HOLE_TARGETS_DO_NOT_DIFFER" } else if out.iter().all(|r| matches!(r, Err(Skip::NoTargets))) { "HOLE_NO_HITS" } else { "HOLE_TOOL_CONSTRUCTION_FAILED" }, &["targets"]));
        }
        if subtract {
            let mut groups: BTreeMap<Vec<BodyId>, Vec<NamedShape>> = BTreeMap::new();
            for b in out.iter().flatten() {
                let ids: Vec<BodyId> = b.cuts.iter().filter(|c| !excluded.contains(c)).cloned().collect();
                if !ids.is_empty() {
                    groups.entry(ids).or_default().push(NamedShape { shape: b.tool.shape.clone(), names: b.tool.names.clone() });
                }
            }
            for (ids, tools) in groups {
                let idx: Vec<usize> = ids.iter().filter_map(|id| ev.ctx.bodies.iter().position(|b| &b.id == id)).collect();
                ev.boolean_into(&idx, tools, k::BooleanOp::Cut)?;
            }
        }
        if keep {
            let first = ev.ctx.bodies.iter().filter(|b| b.id.feature == ev.feature).count() as u32;
            for (n, b) in (first..).zip(out.iter().flatten()) {
                push_body(ev, BodyId { feature: ev.feature, index: n }, BodyKind::Solid, b.tool.shape.clone(), b.tool.names.faces.clone());
            }
        }
        Ok(out)
    })
    // std hole.fs catches opHole's failure as an ErrorStringEnum
    // (displayToolErrorEntities, `error == HOLE_TARGETS_DO_NOT_DIFFER`):
    // the member itself, not a regenError map.
    .map_err(|mut e| {
        if e.value.as_map().is_some_and(|m| m.len() == 1) {
            e.value = e.value.get("message").clone();
        }
        e
    })?;
    let mut st = cx.state.lock();
    let mut values = Vec::new();
    let mut records = Vec::new();
    for r in results {
        let Ok(b) = r else {
            values.push(Value::map_from([("success", Value::bool(false))]));
            continue;
        };
        let mut transient = |body: &BodyId| Value::str(st.transient(&Reference::Body(body.clone())));
        let ext: Vec<(String, Value)> = b
            .extremes
            .iter()
            .map(|e| {
                let key = transient(&e.body).as_str().unwrap_or_default().to_string();
                let m = Value::map_from([
                    ("firstEntrance", Value::num(e.first_entrance)),
                    ("fullEntrance", Value::num(e.full_entrance)),
                    ("firstExit", Value::num(e.first_exit)),
                    ("fullExit", Value::num(e.full_exit)),
                ]);
                (key, m)
            })
            .collect();
        let roots: Vec<(String, Value)> = b
            .roots
            .iter()
            .map(|(k, r)| {
                let target = r.target.as_ref().map_or(Value::Undefined, &mut transient);
                (k.to_string(), Value::map_from([("referenceRootStart", Value::num(r.start)), ("referenceRootEnd", Value::num(r.end)), ("target", target)]))
            })
            .collect();
        values.push(Value::map_from([
            ("success", Value::bool(true)),
            ("holeDepth", Value::num(b.depth)),
            ("targetToDepthExtremes", Value::map_from(ext.iter().map(|(k, v)| (k.as_str(), v.clone())))),
            ("positionReferenceInfo", Value::map_from(roots.iter().map(|(k, v)| (k.as_str(), v.clone())))),
        ]));
        records.push(b.record);
    }
    st.holes.insert(id, records);
    Ok(Value::array(values))
}

/// `qOpHoleFace` / `qOpHoleProfile`.
pub fn op_hole_query(i: &mut Interp, q: &QueryCx, query: &Value, faces: bool) -> R<Vec<Ent>> {
    let Some(id) = query.get("featureId").as_array().map(|a| a.iter().filter_map(|c| c.as_str()).collect::<Vec<_>>()) else { return Ok(Vec::new()) };
    let Some(records) = q.st.holes.get(&id) else { return Ok(Vec::new()) };
    let name = query.get("name").as_str();
    let identity: Option<Vec<Reference>> = match query.get("identity") {
        Value::Undefined => None,
        v => Some(super::query::eval(i, q, v)?.iter().map(|e| q.to_ref(e)).collect()),
    };
    let holes: Vec<&HoleRecord> = records.iter().filter(|h| identity.as_ref().is_none_or(|ids| ids.iter().any(|r| h.identity.contains(r)))).collect();
    // Faces of this operation's holes: (hole index, face index).
    let hole_face = |b: usize, f: usize| -> Option<(u32, usize)> {
        let n = &q.st.ctx.bodies[b].faces[f];
        let op = q.st.op_of_face(n)?;
        if op.id != id {
            return None;
        }
        match n.root() {
            TopoName::Face { role: Role::Generated(inner), .. } => match inner.root() {
                TopoName::Face { role: Role::Side(Source::Index(code)), .. } => Some((code >> 16, (code & 0xFFFF) as usize)),
                _ => None,
            },
            _ => None,
        }
    };
    let mut out = Vec::new();
    for (b, body) in q.st.ctx.bodies.iter().enumerate() {
        for f in 0..body.faces.len() {
            let Some((h, n)) = hole_face(b, f) else { continue };
            let Some(hole) = holes.iter().find(|x| x.index == h) else { continue };
            if faces {
                if name.is_none() || hole.face_names.get(n).cloned().flatten() == name {
                    out.push(Ent::Face(b, f));
                }
                continue;
            }
            // Profiles (live): the edge between the hole faces before and
            // after the profile, or the vertex of a radius 0 profile.
            for (k, p) in hole.profiles.iter().enumerate() {
                if name.is_some() && p.name != name {
                    continue;
                }
                let before = if k == 0 { 0 } else { p.index };
                let after = hole.profiles.get(k + 1).map(|x| x.index);
                if p.radius <= TOL {
                    if n != before && Some(n) != after {
                        continue;
                    }
                    let (_, verts) = q.boundary(&Ent::Face(b, f));
                    for v in verts {
                        let on_axis = q.point(&v).is_some_and(|x| {
                            let r = x - hole.origin;
                            let t = r.dot(hole.dir);
                            (t - p.position).abs() < 1e-7 && (r - hole.dir * t).length() < 1e-7
                        });
                        if on_axis && !out.contains(&v) {
                            out.push(v);
                        }
                    }
                    continue;
                }
                let Some(after) = after.filter(|_| n == before) else { continue };
                let (edges, _) = q.boundary(&Ent::Face(b, f));
                for e in edges {
                    let Ent::Edge(eb, ei) = e else { continue };
                    let other = q.faces_of_edge(eb, ei).into_iter().any(|x| matches!(x, Ent::Face(xb, xf) if (xb, xf) != (b, f) && hole_face(xb, xf) == Some((h, after))));
                    if other && !out.contains(&e) {
                        out.push(e);
                    }
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_pick_first_and_last_targets() {
        let b = |n: u32| BodyId { feature: crate::reference::FeatureId(1), index: n };
        let ex = vec![
            Extremes { body: b(0), first_entrance: 0.001, full_entrance: 0.003, first_exit: 0.01, full_exit: 0.01 },
            Extremes { body: b(1), first_entrance: 0.01, full_entrance: 0.01, first_exit: 0.04, full_exit: 0.04 },
        ];
        assert_eq!(root_of("TARGET_START", &ex, None), Some(Root { start: 0.001, end: 0.003, target: Some(b(0)) }));
        assert_eq!(root_of("LAST_TARGET_START", &ex, None).and_then(|r| r.target), Some(b(1)));
        assert_eq!(root_of("LAST_TARGET_END", &ex, None).map(|r| r.end), Some(0.04));
        assert_eq!(root_of("UP_TO_NEXT", &ex, None).map(|r| r.end), Some(0.01));
        assert_eq!(root_of("UP_TO_ENTITY", &ex, None), None);
    }
}
