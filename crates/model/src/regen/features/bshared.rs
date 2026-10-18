//! Shared helpers for the reference / curve / surface / sheet metal / import
//! evaluators: curve selections to kernel edges, wire bodies, surface results
//! (New / Add) and naming helpers.

use crate::display::BodyKind;
use crate::naming::{Name, ShapeNames};
use crate::reference::{BodyId, FeatureId, Reference, Role, Source, TopoName};
use crate::regen::context::Body;
use crate::regen::eval::{EvalCtx, FResult, NamedShape, err, kernel_plane, split_solids};
use glam::{DVec2, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Name of a face this feature creates with a given role.
pub fn face_name(ev: &EvalCtx, role: Role) -> Name {
    Arc::new(TopoName::face(ev.feature, role))
}

/// Names for brand-new faces (no history): `Role::New(i)` ordered by a
/// geometric key, so they are deterministic.
pub fn new_face_names(ev: &EvalCtx, shape: &Shape) -> Vec<Name> {
    ev.name_result(&[&ShapeNames::default()], &k::History::default(), shape, &BTreeMap::new(), None)
}

/// Name output faces from face-to-face history records using `mk(source)`;
/// faces without a face parent become `Role::New`.
pub fn names_from_sources(ev: &EvalCtx, hist: &k::History, out: &Shape, arg: u32, sources: &[Name], mk: impl Fn(&Name) -> Role) -> Vec<Name> {
    let mut roles = BTreeMap::new();
    for r in &hist.records {
        if r.arg == arg
            && r.input_kind == ShapeType::Face
            && r.output_kind == ShapeType::Face
            && matches!(r.relation, k::Relation::Modified | k::Relation::Kept | k::Relation::Generated)
            && let Some(src) = sources.get(r.input_index as usize)
        {
            roles.entry(r.output_index).or_insert_with(|| mk(src));
        }
    }
    ev.name_result(&[&ShapeNames::default()], hist, out, &roles, None)
}

/// A kernel edge for a sketch curve entity (None for points or missing entities).
pub fn sketch_edge(ev: &EvalCtx, sketch: FeatureId, entity: u32) -> FResult<Option<Shape>> {
    let Some(s) = ev.ctx.sketch(sketch) else { return Ok(None) };
    let id = noshape_sketch::EntityId(entity);
    let Some(curve) = s.sketch.curve(id) else { return Ok(None) };
    let seg = match super::extrude::curve_to_segment(&curve) {
        Some(seg) => seg,
        None => {
            let bb = s.sketch.bbox();
            let tol = ((bb.max - bb.min).length() * 1e-4).max(1e-7);
            let pts: Vec<DVec2> = s.sketch.tessellate(id, tol);
            if pts.len() < 2 {
                return Ok(None);
            }
            let closed = (pts[0] - pts[pts.len() - 1]).length() < tol;
            let pts = if closed { pts[..pts.len() - 1].to_vec() } else { pts };
            k::Segment2d::Interpolated { points: pts, periodic: closed }
        }
    };
    Ok(Some(k::make_edge_2d(&kernel_plane(&s.frame), &seg)?))
}

fn is_construction(ev: &EvalCtx, sketch: FeatureId, entity: u32) -> bool {
    ev.ctx.sketch(sketch).and_then(|s| s.sketch.entity(noshape_sketch::EntityId(entity)).map(|e| e.construction)).unwrap_or(false)
}

/// Kernel edges for a curve selection: edges, wire bodies (all edges),
/// surface bodies (their boundary edges), sketch curves and whole sketches.
/// Fails if a selection no longer resolves.
pub fn curve_edges(ev: &EvalCtx, refs: &[Reference]) -> FResult<Vec<Shape>> {
    let mut out: Vec<Shape> = Vec::new();
    for r in refs {
        let one = std::slice::from_ref(r);
        let before = out.len();
        match r {
            Reference::Edge(_) => {
                for (bi, ei) in ev.resolve_edges(one) {
                    out.push(ev.edge_shape(bi, ei)?);
                }
            }
            Reference::Body(_) => {
                for bi in ev.resolve_bodies(one) {
                    let b = &ev.ctx.bodies[bi];
                    match b.kind {
                        BodyKind::Surface => {
                            let ef = b.shape.edge_faces()?;
                            for (i, e) in b.shape.edges().into_iter().enumerate() {
                                if ef.get(i).is_some_and(|f| f.len() == 1) {
                                    out.push(e);
                                }
                            }
                        }
                        _ => out.extend(b.shape.edges()),
                    }
                }
            }
            Reference::SketchEntity { sketch, entity } => {
                if let Some(e) = sketch_edge(ev, *sketch, *entity)? {
                    out.push(e);
                }
            }
            Reference::Sketch(sketch) => {
                if let Some(s) = ev.ctx.sketch(*sketch) {
                    let ids: Vec<u32> = s.sketch.entities().filter(|e| !e.construction).map(|e| e.id.0).collect();
                    for id in ids {
                        if let Some(e) = sketch_edge(ev, *sketch, id)? {
                            out.push(e);
                        }
                    }
                }
            }
            _ => continue,
        }
        if out.len() == before && !matches!(r, Reference::SketchEntity { sketch, entity } if is_construction(ev, *sketch, *entity)) {
            return err(format!("A selected {} no longer exists", r.kind_label().to_lowercase()));
        }
    }
    let mut uniq: Vec<Shape> = Vec::with_capacity(out.len());
    for e in out {
        if !uniq.iter().any(|u| u.is_same(&e)) {
            uniq.push(e);
        }
    }
    Ok(uniq)
}

/// Faces selected directly, via sheet/solid bodies, or as plane references
/// (planes become large planar faces).
pub fn face_shapes(ev: &EvalCtx, refs: &[Reference], with_planes: bool) -> FResult<Vec<Shape>> {
    let mut out = Vec::new();
    for r in refs {
        let one = std::slice::from_ref(r);
        match r {
            Reference::Face(_) => {
                let found = ev.resolve_faces(one);
                if found.is_empty() {
                    return err("A selected face no longer exists");
                }
                for (bi, fi) in found {
                    out.push(ev.face_shape(bi, fi)?);
                }
            }
            Reference::Body(_) => {
                let found = ev.resolve_bodies(one);
                if found.is_empty() {
                    return err("A selected body no longer exists");
                }
                for bi in found {
                    out.extend(ev.ctx.bodies[bi].shape.faces());
                }
            }
            Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::MateConnector { .. } | Reference::Implicit { .. } if with_planes => {
                let Some(f) = ev.resolve_plane(r) else { return err("A selected plane no longer exists") };
                let ext = ev.model_extent() * 2.0;
                out.push(k::make_plane_face(&kernel_plane(&f), -ext, ext, -ext, ext)?);
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Group loose edges into connected wires (one per chain).
pub fn chain_wires(edges: &[Shape]) -> FResult<Vec<Shape>> {
    if edges.is_empty() {
        return Ok(Vec::new());
    }
    let c = Shape::compound(edges);
    Ok(k::edges_to_wires(&c, 1e-6)?)
}

/// Add wire (curve) bodies created by this feature. Edge and vertex names are
/// `Wire { feature, Index(body << 20 | i) }`, unique across the feature's bodies.
pub fn add_wire_bodies(ev: &mut EvalCtx, shapes: Vec<Shape>) -> FResult<()> {
    if shapes.is_empty() {
        return err("No curves were created");
    }
    for (bk, s) in shapes.into_iter().enumerate() {
        add_wire_body(ev, s, bk as u32, BodyKind::Wire);
    }
    Ok(())
}

/// Add one wire or point body with index-based names.
pub fn add_wire_body(ev: &mut EvalCtx, shape: Shape, index: u32, kind: BodyKind) {
    let name = ev.ctx.alloc_part_name(kind);
    let color = ev.ctx.alloc_color();
    let id = BodyId { feature: ev.feature, index };
    let body = wire_body(ev.feature, id, name, kind, shape, color, index);
    ev.ctx.bodies.push(Arc::new(body));
}

/// Build a wire/point body with names unique for (feature, index).
pub fn wire_body(feature: FeatureId, id: BodyId, name: String, kind: BodyKind, shape: Shape, color: usize, index: u32) -> Body {
    let mut body = Body::new(id, name, kind, shape, Vec::new(), color, feature);
    let base = index << 20;
    body.edges = (0..body.edges.len()).map(|i| Arc::new(TopoName::Wire { feature, source: Source::Index(base | i as u32) })).collect();
    body.vertices = (0..body.vertices.len()).map(|i| Arc::new(TopoName::Wire { feature, source: Source::Index(base | 0x8_0000 | i as u32) })).collect();
    body
}

/// Replace the shape of an existing wire body in place (keeps id, name, color).
pub fn replace_wire_body(ev: &mut EvalCtx, bi: usize, shape: Shape) {
    let old = ev.ctx.bodies[bi].clone();
    let index = old.id.index;
    ev.ctx.bodies[bi] = Arc::new(wire_body(ev.feature, old.id.clone(), old.name.clone(), old.kind, shape, old.color, index));
}

/// Result of a surface feature: New, or Add (sew with touching surfaces in
/// the merge scope; a closed result becomes a solid part).
pub fn surface_result(ev: &mut EvalCtx, tool: NamedShape) -> FResult<()> {
    if !ev.is("surfaceOperationType", "ADD") {
        return ev.apply_operation(vec![tool], "NEW", None, BodyKind::Surface);
    }
    let scope: Vec<usize> = if ev.bool("defaultSurfaceScope") {
        (0..ev.ctx.bodies.len()).filter(|&i| ev.ctx.bodies[i].kind == BodyKind::Surface).collect()
    } else {
        ev.resolve_bodies(ev.query("booleanSurfaceScope")).into_iter().filter(|&i| ev.ctx.bodies[i].kind == BodyKind::Surface).collect()
    };
    let targets: Vec<usize> = scope.into_iter().filter(|&i| k::distance(&ev.ctx.bodies[i].shape, &tool.shape).map(|d| d.distance < 1e-6).unwrap_or(false)).collect();
    if targets.is_empty() {
        return ev.apply_operation(vec![tool], "NEW", None, BodyKind::Surface);
    }
    let mut shapes: Vec<Shape> = targets.iter().map(|&i| ev.ctx.bodies[i].shape.clone()).collect();
    shapes.push(tool.shape.clone());
    let res = match k::sew(&shapes, 1e-6, true) {
        Ok(r) if r.shape.count(ShapeType::Solid) > 0 => r,
        _ => k::sew(&shapes, 1e-6, false)?,
    };
    let mut args: Vec<ShapeNames> = targets.iter().map(|&i| ev.ctx.bodies[i].shape_names()).collect();
    args.push(tool.names.clone());
    let arg_refs: Vec<&ShapeNames> = args.iter().collect();
    let names = ev.name_result(&arg_refs, &res.history, &res.shape, &BTreeMap::new(), None);
    let first = ev.ctx.bodies[targets[0]].clone();
    let kind = if res.shape.count(ShapeType::Solid) > 0 { BodyKind::Solid } else { BodyKind::Surface };
    let pieces = split_solids(&res.shape, &names);
    let ids: Vec<BodyId> = targets.iter().map(|&i| ev.ctx.bodies[i].id.clone()).collect();
    let pos = targets[0];
    ev.ctx.bodies.retain(|b| !ids.contains(&b.id));
    for (j, p) in pieces.into_iter().enumerate() {
        let (id, name) = if j == 0 { (first.id.clone(), first.name.clone()) } else { (BodyId { feature: ev.feature, index: 30_000 + j as u32 }, ev.ctx.alloc_part_name(kind)) };
        let body = Body::new(id, name, kind, p.shape, p.names.faces, first.color, ev.feature);
        let at = (pos + j).min(ev.ctx.bodies.len());
        ev.ctx.bodies.insert(at, Arc::new(body));
    }
    Ok(())
}

/// A unit vector perpendicular to `z`, aligned with world X when possible
/// (then Y, then Z), like Onshape's canonical alignment.
pub fn canonical_x(z: DVec3) -> DVec3 {
    for w in [DVec3::X, DVec3::Y, DVec3::Z] {
        let x = w - z * z.dot(w);
        if x.length_squared() > 1e-8 {
            return x.normalize();
        }
    }
    z.any_orthonormal_vector()
}

/// Sample `n + 1` points along an edge (by parameter, following orientation).
pub fn sample_edge(e: &Shape, n: usize) -> FResult<Vec<DVec3>> {
    let info = e.edge_info()?;
    let n = n.max(1);
    let (a, b) = (info.first_param, info.last_param);
    let reversed = matches!(e.orientation(), k::Orientation::Reversed);
    let mut pts = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let t = if reversed { 1.0 - t } else { t };
        pts.push(e.point_at(a + (b - a) * t)?);
    }
    Ok(pts)
}

/// Extent of a face along an axis: (min, max) of the axial coordinate of
/// points sampled on its edges.
pub fn extent_along(face: &Shape, origin: DVec3, axis: DVec3) -> FResult<(f64, f64)> {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for e in face.edges() {
        if e.edge_info().map(|i| i.degenerated).unwrap_or(true) {
            continue;
        }
        for p in sample_edge(&e, 16)? {
            let t = (p - origin).dot(axis);
            lo = lo.min(t);
            hi = hi.max(t);
        }
    }
    if !lo.is_finite() {
        return err("Could not measure the selected face");
    }
    Ok((lo, hi))
}

/// Order edges into a single chain (start to end) by matching endpoints;
/// returns `(edge, reversed)` pairs. Fails on branching or gaps.
pub fn order_chain(edges: &[Shape], tol: f64) -> FResult<Vec<(Shape, bool)>> {
    let ends: Vec<(DVec3, DVec3)> = edges.iter().map(|e| e.endpoints()).collect::<Result<_, _>>()?;
    let n = edges.len();
    if n == 0 {
        return err("Select edges");
    }
    // Start from an edge end that touches no other edge (open chain), else edge 0.
    let touches = |p: DVec3, skip: usize| (0..n).any(|j| j != skip && ((ends[j].0 - p).length() < tol || (ends[j].1 - p).length() < tol));
    let mut start = (0usize, false);
    for (i, (a, b)) in ends.iter().enumerate() {
        if !touches(*a, i) {
            start = (i, false);
            break;
        }
        if !touches(*b, i) {
            start = (i, true);
            break;
        }
    }
    let mut used = vec![false; n];
    let mut out = vec![(edges[start.0].clone(), start.1)];
    used[start.0] = true;
    let mut cur = if start.1 { ends[start.0].0 } else { ends[start.0].1 };
    for _ in 1..n {
        let next = (0..n).find_map(|j| {
            if used[j] {
                None
            } else if (ends[j].0 - cur).length() < tol {
                Some((j, false))
            } else if (ends[j].1 - cur).length() < tol {
                Some((j, true))
            } else {
                None
            }
        });
        let Some((j, rev)) = next else { return err("Selected edges must form one connected chain") };
        used[j] = true;
        cur = if rev { ends[j].0 } else { ends[j].1 };
        out.push((edges[j].clone(), rev));
    }
    Ok(out)
}

/// Points sampled along an ordered chain (joints not duplicated).
pub fn sample_chain(chain: &[(Shape, bool)], per_edge: usize) -> FResult<Vec<DVec3>> {
    let mut pts: Vec<DVec3> = Vec::new();
    for (e, rev) in chain {
        let (a, b) = e.endpoints()?;
        let mut s = sample_edge(e, per_edge)?;
        // sample_edge follows orientation; make it run endpoints().0 -> .1 first.
        if (s[0] - a).length() > (s[0] - b).length() {
            s.reverse();
        }
        if *rev {
            s.reverse();
        }
        if !pts.is_empty() {
            s.remove(0);
        }
        pts.extend(s);
    }
    Ok(pts)
}
