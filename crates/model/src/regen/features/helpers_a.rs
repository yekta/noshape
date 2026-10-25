//! Shared helpers for the batch A evaluators (solid, hole, detail extras,
//! split, patterns/transform, direct edit): curve resolution, naming of
//! copies and sub-shapes, body bookkeeping and ray queries.

use crate::display::BodyKind;
use crate::naming::{Name, ShapeNames};
use crate::reference::{BodyId, Reference, Role, Source, TopoName};
use crate::regen::context::{Body, SketchResult};
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, Profile, kernel_plane};
use glam::{DVec2, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn fe(m: impl Into<String>) -> FeatureError {
    FeatureError::new(m)
}

/// Kernel errors with a feature-specific prefix.
pub fn kerr(what: &str) -> impl Fn(k::KernelError) -> FeatureError + '_ {
    move |e| FeatureError::new(format!("{what}: {e}"))
}

// ------------------------------------------------------------------ curves

/// Sketch curve -> kernel 2D segment (every curve kind).
pub fn curve_segment(c: &noshape_sketch::Curve2) -> Option<k::Segment2d> {
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
        C::Ellipse { center, major, minor } => {
            k::Segment2d::Ellipse { center: *center, major_radius: major.length(), minor_radius: *minor, rotation: major.y.atan2(major.x) }
        }
        C::EllipticalArc { center, major, minor, start_param, end_param } => k::Segment2d::EllipseArc {
            center: *center,
            major_radius: major.length(),
            minor_radius: *minor,
            rotation: major.y.atan2(major.x),
            start_angle: *start_param,
            end_angle: *end_param,
        },
        C::Nurbs(n) => {
            let mut uk: Vec<f64> = Vec::new();
            let mut mults: Vec<u32> = Vec::new();
            for &kv in &n.knots {
                if let Some(last) = uk.last()
                    && (kv - last).abs() < 1e-12
                {
                    if let Some(m) = mults.last_mut() {
                        *m += 1;
                    }
                    continue;
                }
                uk.push(kv);
                mults.push(1);
            }
            k::Segment2d::BSpline { poles: n.ctrl.clone(), weights: n.weights.clone(), knots: uk, mults, degree: n.degree as u32, periodic: false }
        }
    })
}

/// A 3D edge with the persistent identity it came from.
#[derive(Clone)]
pub struct SourcedEdge {
    pub shape: Shape,
    pub source: Source,
}

fn sketch_entity_edge(s: &SketchResult, entity: u32) -> FResult<Option<Shape>> {
    let id = noshape_sketch::EntityId(entity);
    if s.sketch.entity(id).is_some_and(|e| e.construction) {
        return Ok(None);
    }
    let Some(curve) = s.sketch.curve(id) else { return Ok(None) };
    let Some(seg) = curve_segment(&curve) else { return Ok(None) };
    Ok(Some(k::make_edge_2d(&kernel_plane(&s.frame), &seg)?))
}

/// Resolve curve-like references (sketch curves, whole sketches, body edges,
/// wire bodies) to 3D edges. Points and other references are ignored.
pub fn resolve_curves(ev: &EvalCtx, refs: &[Reference]) -> FResult<Vec<SourcedEdge>> {
    let mut out = Vec::new();
    for r in refs {
        match r {
            Reference::SketchEntity { sketch, entity } => {
                let Some(s) = ev.ctx.sketch(*sketch) else { return Err(fe("A selected sketch no longer exists")) };
                if let Some(e) = sketch_entity_edge(s, *entity)? {
                    out.push(SourcedEdge { shape: e, source: Source::Sketch { sketch: *sketch, entity: *entity, segment: 0 } });
                }
            }
            Reference::Sketch(sketch) => {
                let Some(s) = ev.ctx.sketch(*sketch) else { return Err(fe("A selected sketch no longer exists")) };
                let ids: Vec<u32> = s.sketch.entities().filter(|e| e.kind().is_curve() && !e.construction).map(|e| e.id.0).collect();
                for id in ids {
                    if let Some(e) = sketch_entity_edge(s, id)? {
                        out.push(SourcedEdge { shape: e, source: Source::Sketch { sketch: *sketch, entity: id, segment: 0 } });
                    }
                }
            }
            Reference::Edge(_) => {
                for (bi, ei) in ev.resolve_edges(std::slice::from_ref(r)) {
                    let b = &ev.ctx.bodies[bi];
                    out.push(SourcedEdge { shape: b.shape.sub_shape(ShapeType::Edge, ei)?, source: Source::Topo(b.edges[ei].clone()) });
                }
            }
            Reference::Body(id) => {
                if let Some(b) = ev.ctx.body(id)
                    && matches!(b.kind, BodyKind::Wire)
                {
                    for (i, e) in b.shape.edges().into_iter().enumerate() {
                        out.push(SourcedEdge { shape: e, source: Source::Topo(b.edges[i].clone()) });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Connect edges into wires (each wire is a connected chain).
pub fn wires_from(edges: &[SourcedEdge]) -> FResult<Vec<Shape>> {
    if edges.is_empty() {
        return Ok(Vec::new());
    }
    let shapes: Vec<Shape> = edges.iter().map(|e| e.shape.clone()).collect();
    if shapes.len() == 1 {
        return Ok(vec![k::make_wire(&shapes)?]);
    }
    Ok(k::edges_to_wires(&Shape::compound(&shapes), 1e-6)?)
}

/// Midpoint of an edge (by parameter).
pub fn edge_mid(e: &Shape) -> Option<DVec3> {
    let i = e.edge_info().ok()?;
    e.point_at((i.first_param + i.last_param) * 0.5).ok()
}

/// Sources for each edge of `shape` (indexed like `shape.edges()`), matched
/// to `sources` by edge midpoint; unmatched edges fall back to an index.
pub fn match_edge_sources(shape: &Shape, sources: &[SourcedEdge]) -> Vec<Source> {
    let mids: Vec<Option<DVec3>> = sources.iter().map(|s| edge_mid(&s.shape)).collect();
    shape
        .edges()
        .iter()
        .enumerate()
        .map(|(i, e)| {
            if let Some(src) = sources.iter().find(|s| s.shape.is_same(e)) {
                return src.source.clone();
            }
            let Some(m) = edge_mid(e) else { return Source::Index(i as u32) };
            let best = mids.iter().enumerate().filter_map(|(j, mj)| mj.map(|p| (j, p.distance(m)))).min_by(|a, b| a.1.total_cmp(&b.1));
            match best {
                Some((j, d)) if d < 1e-6 => sources[j].source.clone(),
                _ => Source::Index(i as u32),
            }
        })
        .collect()
}

/// A wire profile (for surface sweeps / lofts), usable with `sweep_names`.
pub fn wire_profile(wire: Shape, sources: &[SourcedEdge], key: Source) -> Profile {
    let edge_sources = match_edge_sources(&wire, sources);
    let frame = crate::display::PlaneFrame::TOP;
    Profile { shape: wire, frame, edge_sources, key, from_face: false }
}

// ------------------------------------------------------------------ naming

/// Names of the faces/edges/vertices of a sub-shape (e.g. a face) of a body.
pub fn sub_names(body: &Body, sub: &Shape) -> ShapeNames {
    let fallback = || Arc::new(TopoName::face(body.id.feature, Role::New(0)));
    let faces = sub.faces().iter().map(|f| body.shape.index_of(f, ShapeType::Face).and_then(|i| body.faces.get(i).cloned()).unwrap_or_else(fallback)).collect();
    let edges = sub.edges().iter().map(|f| body.shape.index_of(f, ShapeType::Edge).and_then(|i| body.edges.get(i).cloned()).unwrap_or_else(fallback)).collect();
    let vertices = sub.vertices().iter().map(|f| body.shape.index_of(f, ShapeType::Vertex).and_then(|i| body.vertices.get(i).cloned()).unwrap_or_else(fallback)).collect();
    ShapeNames { faces, edges, vertices }
}

/// Face names of a copy (transform / pattern instance): output face j is
/// `Copy { instance, of: <name of the input face it came from> }`.
pub fn copy_names(ev: &EvalCtx, src: &[Name], res: &k::OpResult, instance: u32) -> Vec<Name> {
    let n = res.shape.count(ShapeType::Face);
    let mut out: Vec<Option<Name>> = vec![None; n];
    for r in &res.history.records {
        if r.arg == 0
            && r.input_kind == ShapeType::Face
            && r.output_kind == ShapeType::Face
            && matches!(r.relation, k::Relation::Modified | k::Relation::Kept)
            && let (Some(slot), Some(of)) = (out.get_mut(r.output_index as usize), src.get(r.input_index as usize))
            && slot.is_none()
        {
            *slot = Some(Arc::new(TopoName::face(ev.feature, Role::Copy { instance, of: of.clone() })));
        }
    }
    // Same topology order as a fallback (transforms keep MapShapes order).
    out.into_iter()
        .enumerate()
        .map(|(j, n)| {
            n.unwrap_or_else(|| match src.get(j) {
                Some(of) => Arc::new(TopoName::face(ev.feature, Role::Copy { instance, of: of.clone() })),
                None => Arc::new(TopoName::face(ev.feature, Role::Copy { instance, of: Arc::new(TopoName::face(ev.feature, Role::New(j as u32))) })),
            })
        })
        .collect()
}

/// Wrap names created by other features as `Generated(name)` of this
/// feature, so new bodies never share face names with their sources.
pub fn rebrand(ev: &EvalCtx, names: Vec<Name>) -> Vec<Name> {
    let names: Vec<Name> = names
        .into_iter()
        .map(|n| if n.creator() == Some(ev.feature) { n } else { Arc::new(TopoName::face(ev.feature, Role::Generated(n))) })
        .collect();
    dedup_names(ev, names)
}

/// Disambiguate duplicate names with split ordinals (in index order).
pub fn dedup_names(ev: &EvalCtx, mut names: Vec<Name>) -> Vec<Name> {
    let mut groups: BTreeMap<Name, Vec<usize>> = BTreeMap::new();
    for (i, n) in names.iter().enumerate() {
        groups.entry(n.clone()).or_default().push(i);
    }
    for (n, idx) in groups {
        if idx.len() > 1 {
            for (ord, i) in idx.into_iter().enumerate() {
                names[i] = Arc::new(TopoName::Split { parent: n.clone(), by: ev.feature, ordinal: ord as u16 });
            }
        }
    }
    names
}

/// Name faces of a fresh shape by sorting them with `key` and assigning
/// `role(ordinal)`.
pub fn name_sorted(ev: &EvalCtx, shape: &Shape, key: impl Fn(&Shape) -> [f64; 2], role: impl Fn(u32) -> Role) -> Vec<Name> {
    let faces = shape.faces();
    let mut order: Vec<(usize, [f64; 2])> = faces.iter().enumerate().map(|(i, f)| (i, key(f))).collect();
    let q = |x: f64| (x * 1e9).round() as i64;
    order.sort_by(|a, b| q(a.1[0]).cmp(&q(b.1[0])).then(q(a.1[1]).cmp(&q(b.1[1]))).then(a.0.cmp(&b.0)));
    let mut names: Vec<Option<Name>> = vec![None; faces.len()];
    for (ord, (i, _)) in order.into_iter().enumerate() {
        names[i] = Some(Arc::new(TopoName::face(ev.feature, role(ord as u32))));
    }
    names.into_iter().map(|n| n.unwrap_or_else(|| Arc::new(TopoName::face(ev.feature, Role::New(0))))).collect()
}

/// Plain geometric naming (`New(ordinal)`) for a primitive.
pub fn name_new(ev: &EvalCtx, shape: &Shape) -> Vec<Name> {
    let empty = ShapeNames::default();
    ev.name_result(&[&empty], &k::History::default(), shape, &BTreeMap::new(), None)
}

/// Zig-zag encoding of signed instance indices (0, -1, 1, -2, 2 ...).
pub fn zigzag(i: i64) -> u32 {
    if i >= 0 { (2 * i) as u32 } else { (-2 * i - 1) as u32 }
}

// ------------------------------------------------------------------ bodies

pub fn body_named(b: &Body) -> NamedShape {
    NamedShape { shape: b.shape.clone(), names: b.shape_names() }
}

pub fn index_of_body(ev: &EvalCtx, id: &BodyId) -> Option<usize> {
    ev.ctx.bodies.iter().position(|b| &b.id == id)
}

pub fn remove_bodies(ev: &mut EvalCtx, ids: &[BodyId]) {
    ev.ctx.bodies.retain(|b| !ids.contains(&b.id));
}

/// Push a new body with a fresh part name and color.
pub fn push_body(ev: &mut EvalCtx, id: BodyId, kind: BodyKind, shape: Shape, faces: Vec<Name>) {
    let name = ev.ctx.alloc_part_name(kind);
    let color = ev.ctx.alloc_color();
    let f = ev.feature;
    ev.ctx.bodies.push(Arc::new(Body::new(id, name, kind, shape, faces, color, f)));
}

/// Body ids of the result-operation scope (Merge with all / Merge scope).
pub fn scope_ids(ev: &EvalCtx) -> Option<Vec<BodyId>> {
    super::extrude::scope(ev).map(|v| v.into_iter().map(|i| ev.ctx.bodies[i].id.clone()).collect())
}

pub fn ids_to_indices(ev: &EvalCtx, ids: &Option<Vec<BodyId>>) -> Option<Vec<usize>> {
    ids.as_ref().map(|v| v.iter().filter_map(|id| index_of_body(ev, id)).collect())
}

/// Group (body, sub-shape) pairs by body.
pub fn by_body(items: Vec<(usize, usize)>) -> BTreeMap<usize, Vec<usize>> {
    let mut m: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (b, e) in items {
        m.entry(b).or_default().push(e);
    }
    m
}

// ------------------------------------------------------------------ geometry

/// Parameter intervals `[t0, t1]` along `p + t * dir` (0 <= t <= len) where
/// the ray is inside `solid`.
pub fn ray_intervals(solid: &Shape, p: DVec3, dir: DVec3, len: f64) -> Vec<(f64, f64)> {
    if let Some(iv) = ray_crossings(solid, p, dir, len) {
        return iv;
    }
    let Ok(line) = k::make_line(p, p + dir * len) else { return Vec::new() };
    let Ok(res) = k::boolean(k::BooleanOp::Common, &[line], std::slice::from_ref(solid), &k::BooleanOptions::default()) else { return Vec::new() };
    let mut out: Vec<(f64, f64)> = res
        .shape
        .edges()
        .iter()
        .filter_map(|e| e.endpoints().ok())
        .map(|(a, b)| {
            let (ta, tb) = ((a - p).dot(dir), (b - p).dot(dir));
            (ta.min(tb), ta.max(tb))
        })
        .filter(|(a, b)| b - a > 1e-9)
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Merge touching intervals.
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for iv in out {
        match merged.last_mut() {
            Some(last) if iv.0 <= last.1 + 1e-9 => last.1 = last.1.max(iv.1),
            _ => merged.push(iv),
        }
    }
    merged
}

/// [`ray_intervals`] from where the ray crosses the faces of one solid, in
/// and out (REGEN-SPEED: a Common of the line with the part, per ray, cost
/// more than the Hole's cut on live's 30-feature part). None where that isn't
/// clear-cut (a tangent touch, a crossing on a face's edge, crossings that
/// don't alternate): the Common decides then.
fn ray_crossings(solid: &Shape, p: DVec3, dir: DVec3, len: f64) -> Option<Vec<(f64, f64)>> {
    if solid.shape_type() != k::ShapeType::Solid || len <= 0.0 {
        return None;
    }
    let mut hits = solid.line_crossings(p, dir, len).ok()?;
    if hits.iter().any(|&(_, t, on)| on || t == 0) {
        return None;
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    hits.dedup_by(|b, a| (b.0 - a.0).abs() < 1e-9 && b.1 == a.1);
    let mut out: Vec<(f64, f64)> = Vec::new();
    let mut inside: Option<f64> = None;
    for (i, &(t, tr, _)) in hits.iter().enumerate() {
        match (tr, inside) {
            (1, None) => inside = Some(t),
            // The ray starts inside the solid.
            (-1, None) if i == 0 => out.push((0.0, t)),
            (-1, Some(a)) => {
                out.push((a, t));
                inside = None;
            }
            _ => return None,
        }
    }
    if let Some(a) = inside {
        out.push((a, len));
    }
    out.retain(|(a, b)| b - a > 1e-9);
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for iv in out {
        match merged.last_mut() {
            Some(last) if iv.0 <= last.1 + 1e-9 => last.1 = last.1.max(iv.1),
            _ => merged.push(iv),
        }
    }
    Some(merged)
}

/// Unit vector perpendicular to `n`.
pub fn perp(n: DVec3) -> DVec3 {
    n.any_orthonormal_vector()
}

/// Kernel plane from origin + normal.
pub fn plane_from(origin: DVec3, normal: DVec3) -> k::Plane {
    let n = normal.normalize();
    k::Plane::new(origin, n, perp(n))
}

/// A big planar face for a datum plane (split / enclose tools).
pub fn big_plane_face(ev: &EvalCtx, plane: &k::Plane) -> FResult<Shape> {
    let ext = ev.model_extent() * 2.0;
    let c = ev.ctx.bounds().map(|(a, b)| (a + b) * 0.5).unwrap_or(DVec3::ZERO);
    let l = plane.to_local(c);
    Ok(k::make_plane_face(plane, l.x - ext, l.x + ext, l.y - ext, l.y + ext)?)
}

/// Surface normal of a face at the point nearest to `p` (body orientation).
pub fn face_normal_near(face: &Shape, p: DVec3) -> Option<(DVec3, DVec3)> {
    let pr = face.project_point(p).ok()?;
    let n = face.normal_at_uv(pr.u, pr.v).ok()?;
    Some((pr.point, n))
}

/// A point on a face (projection of its centroid).
pub fn point_on_face(face: &Shape) -> Option<DVec3> {
    let c = face.centroid().ok()?;
    face.project_point(c).ok().map(|p| p.point)
}

pub fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
