//! Surface features: Offset surface, Fill, Move boundary, Boundary surface,
//! Ruled surface, Mutual trim; Constrained surface reports a clear error.

use super::bshared::{chain_wires, curve_edges, face_name, names_from_sources, new_face_names, order_chain, sample_chain, surface_result};
use crate::display::BodyKind;
use crate::naming::Name;
use crate::reference::{BodyId, Reference, Role, Source, TopoName};
use crate::regen::context::Body;
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, err, names_for};
use glam::{DQuat, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::sync::Arc;

// ------------------------------------------------------------------ offset surface

pub fn offset_surface(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("surfacesAndFaces");
    if refs.is_empty() {
        return crate::regen::eval::err_at(&["surfacesAndFaces"], "Select faces, surfaces, or sketch regions to offset from.");
    }
    // (face, source name) per group: each picked face or region is its own
    // surface (live L26: even two adjacent faces give two), a picked surface
    // stays one.
    let mut groups: Vec<Vec<(Shape, Name)>> = Vec::new();
    for r in refs {
        let one = std::slice::from_ref(r);
        match r {
            Reference::Face(_) => {
                // Faces that no longer resolve are left out (offsetSurface.fs).
                for (bi, fi) in ev.resolve_faces(one) {
                    groups.push(vec![(ev.face_shape(bi, fi)?, ev.ctx.bodies[bi].faces[fi].clone())]);
                }
            }
            Reference::Body(_) => {
                for bi in ev.resolve_bodies(one) {
                    let b = &ev.ctx.bodies[bi];
                    groups.push(b.shape.faces().into_iter().enumerate().map(|(fi, f)| (f, b.faces[fi].clone())).collect());
                }
            }
            Reference::SketchRegion { .. } => {
                for p in ev.profiles(one)? {
                    let name = face_name(ev, Role::Profile(p.key));
                    groups.push(vec![(p.shape, name)]);
                }
            }
            _ => {}
        }
    }
    groups.retain(|g| !g.is_empty());
    if groups.is_empty() {
        return crate::regen::eval::err_at(&["surfacesAndFaces"], "Select faces, surfaces, or sketch regions to offset from.");
    }
    let d = ev.length("offset")? * if ev.bool("oppositeDirection") { -1.0 } else { 1.0 };
    let mut tools = Vec::new();
    for faces in groups {
        let shapes: Vec<Shape> = faces.iter().map(|f| f.0.clone()).collect();
        let sources: Vec<Name> = faces.iter().map(|f| f.1.clone()).collect();
        let base = if shapes.len() > 1 { k::sew(&shapes, 1e-6, false).map(|r| r.shape).unwrap_or_else(|_| Shape::compound(&shapes)) } else { shapes[0].clone() };
        // Source name of each face of `base` (by geometric identity with the inputs).
        let base_sources: Vec<Name> = base
            .faces()
            .iter()
            .enumerate()
            .map(|(i, f)| {
                shapes
                    .iter()
                    .position(|s| s.is_same(f))
                    .or_else(|| {
                        let c = f.centroid().ok()?;
                        shapes.iter().position(|s| s.centroid().map(|d| (d - c).length() < 1e-7).unwrap_or(false))
                    })
                    .and_then(|j| sources.get(j).cloned())
                    .unwrap_or_else(|| face_name(ev, Role::New(i as u32)))
            })
            .collect();
        let (shape, names) = if d.abs() < 1e-12 {
            let copy = base.transformed(&k::Transform::Translate(DVec3::ZERO))?;
            let names: Vec<Name> = base_sources.iter().map(|s| face_name(ev, Role::Offset(s.clone()))).collect();
            (copy, names)
        } else {
            // Live L25: e.g. a concave fillet offset by more than its radius.
            let res = k::offset_shape(&base, d, k::JoinType::Arc).map_err(|_| FeatureError::new("Could not offset selected faces as requested."))?;
            let mut names = names_from_sources(ev, &res.history, &res.shape, 0, &base_sources, |s| Role::Offset(s.clone()));
            // Without usable history, faces correspond in order when counts match.
            if res.history.records.is_empty() && res.shape.count(ShapeType::Face) == base_sources.len() {
                names = base_sources.iter().map(|s| face_name(ev, Role::Offset(s.clone()))).collect();
            }
            (res.shape, names)
        };
        tools.push(NamedShape::new(shape, names));
    }
    ev.apply_operation(tools, "NEW", None, BodyKind::Surface)
}

// ------------------------------------------------------------------ fill

/// The face adjacent to `edge` in the body that owns it (for G1/G2 boundaries).
/// How many faces of its body an edge borders (0 for a sketch curve).
fn edge_face_count(ev: &EvalCtx, edge: &Shape) -> usize {
    ev.ctx.bodies.iter().filter(|b| b.kind != BodyKind::Wire).find_map(|b| {
        let ei = b.shape.index_of(edge, ShapeType::Edge)?;
        Some(b.shape.edge_faces().ok()?.get(ei)?.len())
    }).unwrap_or(0)
}

fn adjacent_face(ev: &EvalCtx, edge: &Shape) -> Option<Shape> {
    for b in &ev.ctx.bodies {
        if b.kind == BodyKind::Wire {
            continue;
        }
        if let Some(ei) = b.shape.index_of(edge, ShapeType::Edge) {
            let ef = b.shape.edge_faces().ok()?;
            let fi = *ef.get(ei)?.first()?;
            return b.shape.sub_shape(ShapeType::Face, fi as usize).ok();
        }
    }
    None
}

/// Live L27: a tangency row on a sketch curve.
const FILL_TANGENCY: &str = "Could not meet tangency constraints at highlighted points.";

/// Onshape's BSURF_FAILED.
const BSURF_FAILED: &str = "Could not create boundary surface with given information. Check profile order, intersections or end conditions.";

pub fn fill(ev: &mut EvalCtx) -> FResult<()> {
    let rows = ev.params.array("edges");
    let mut boundary = Vec::new();
    let mut rows_refs: Vec<&[Reference]> = rows.iter().map(|r| r.query("entities")).collect();
    let pre = ev.query("preselectedEntities");
    if rows.is_empty() && !pre.is_empty() {
        rows_refs.push(pre);
    }
    if rows_refs.iter().all(|r| r.is_empty()) {
        return crate::regen::eval::err_at(&["edges"], "Select laminar edges or curves as fill surface boundary.");
    }
    for (i, refs) in rows_refs.iter().enumerate() {
        let row = rows.get(i);
        let continuity = match row.map(|r| r.enum_("continuity")) {
            Some("G1") => k::Continuity::Tangent,
            Some("G2") => k::Continuity::Curvature,
            _ => k::Continuity::Position,
        };
        let adj = match row {
            Some(r) if continuity != k::Continuity::Position => ev.resolve_faces(r.query("adjacentFaces")).first().map(|&(bi, fi)| ev.face_shape(bi, fi)).transpose()?,
            _ => None,
        };
        for e in curve_edges(ev, refs)? {
            if boundary.iter().any(|b: &k::FillEdge| b.edge.is_same(&e)) {
                return err("A boundary edge can have only one type of continuity constraint.");
            }
            // Live L27: an internal edge (two faces) can't take tangency; a
            // sketch curve has nothing to be tangent to, so the fill is built
            // at position with a warning.
            if continuity != k::Continuity::Position && adj.is_none() && edge_face_count(ev, &e) >= 2 {
                return err("Cannot create tangency or curvature constraint for an internal edge.");
            }
            let support = if continuity == k::Continuity::Position { None } else { adj.clone().or_else(|| adjacent_face(ev, &e)) };
            let continuity = if continuity != k::Continuity::Position && support.is_none() {
                if !ev.warnings.iter().any(|w| w == FILL_TANGENCY) {
                    ev.warnings.push(FILL_TANGENCY.into());
                }
                k::Continuity::Position
            } else {
                continuity
            };
            boundary.push(k::FillEdge { edge: e, support, continuity });
        }
    }
    // The boundary must be one closed loop.
    let edges: Vec<Shape> = boundary.iter().map(|b| b.edge.clone()).collect();
    let wires = chain_wires(&edges)?;
    if wires.len() > 1 {
        return err("Cannot create fill surface between multiple loops.");
    }
    if let Some(w) = wires.first()
        && !w.is_closed().unwrap_or(true)
    {
        let pts = sample_chain(&order_chain(&edges, 1e-6)?, 1)?;
        if pts.len() < 2 || (pts[0] - pts[pts.len() - 1]).length() > 1e-6 {
            return err("Boundary edges selected do not form a closed loop.");
        }
    }
    let mut points = Vec::new();
    let mut curves = Vec::new();
    if ev.bool("addGuides") {
        // Live "psb (10-24 d)": Guides on with nothing picked is a plain fill.
        let refs = ev.query("guideEntities");
        let (mut verts, mut guides) = (Vec::new(), Vec::new());
        for r in refs {
            if let (Reference::Vertex(_) | Reference::SketchEntity { .. }, Some(p)) = (r, ev.resolve_vertex_point(r)) {
                verts.push(p);
                continue;
            }
            guides.extend(curve_edges(ev, std::slice::from_ref(r))?);
        }
        if ev.is("constraintMode", "SAMPLED") {
            let n = ev.integer("sampleSize")?.max(1) as usize;
            points = verts;
            for e in &guides {
                let pts = super::bshared::sample_edge(e, n + 1)?;
                // Interior samples (end points touch the boundary).
                points.extend(pts[1..pts.len() - 1].iter().copied());
            }
        } else if guides.is_empty() {
            points = verts;
        } else {
            // Precise: the surface passes through the guide curves, each
            // reaching the boundary at an end at least (live "psb (10-24 d)":
            // one end is enough, neither fails, no field marked); points
            // can't be mixed in.
            if !verts.is_empty() {
                ev.warnings.push(crate::regen::custom_features::error_text("FILL_CURVE_OR_POINT_CONSTRAINTS"));
            }
            let loop_ = Shape::compound(&edges);
            for w in chain_wires(&guides)? {
                let touches = w.vertices().iter().any(|v| k::distance(v, &loop_).is_ok_and(|d| d.distance < 1e-6));
                if !touches {
                    return err(crate::regen::custom_features::error_text("FILL_SURFACE_BAD_SUPPORT"));
                }
            }
            curves = guides;
        }
    }
    // Onshape's FILL_SURFACE_FAIL text, not the kernel's (qa5 SF5).
    let res = k::fill_surface_guided(&boundary, &points, &curves).map_err(|_| FeatureError::new(crate::regen::custom_features::error_text("FILL_SURFACE_FAIL")))?;
    let names = new_face_names(ev, &res.shape);
    surface_result(ev, NamedShape::new(res.shape, names))
}

// ------------------------------------------------------------------ move boundary

pub fn extend_surface(ev: &mut EvalCtx) -> FResult<()> {
    if !ev.is("endCondition", "BLIND") {
        return err("Move boundary currently supports only the Blind end condition");
    }
    let refs = ev.query("entities");
    let mut bodies: Vec<usize> = Vec::new();
    // Edges picked per body (a whole body picked: all its boundary edges).
    let mut picked: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
    let mut whole: Vec<usize> = Vec::new();
    for r in refs {
        let one = std::slice::from_ref(r);
        match r {
            Reference::Body(_) => {
                let bs = ev.resolve_bodies(one);
                whole.extend(bs.iter().copied());
                bodies.extend(bs);
            }
            Reference::Edge(_) => {
                for (bi, ei) in ev.resolve_edges(one) {
                    bodies.push(bi);
                    picked.entry(bi).or_default().push(ei);
                }
            }
            _ => {}
        }
    }
    bodies.sort_unstable();
    bodies.dedup();
    if bodies.is_empty() {
        return err("Select a surface or its boundary edges");
    }
    let d = ev.length("extendDistance")? * if ev.bool("oppositeDirection") { -1.0 } else { 1.0 };
    for bi in bodies {
        let b = ev.ctx.bodies[bi].clone();
        if b.kind != BodyKind::Surface {
            return err("Move boundary works on surfaces only");
        }
        let faces = b.shape.faces();
        if faces.len() != 1 || faces[0].face_plane()?.is_none() {
            return err("Move boundary currently supports single planar surfaces only (all boundary edges moved)");
        }
        let face = &faces[0];
        let outer = face.wires().into_iter().next().ok_or_else(|| FeatureError::new("Surface has no boundary"))?;
        // A kernel failure reads as Onshape's EXTEND_FAILED ("Unable to move
        // selected entities."), not the kernel's own text (qa5 SF5).
        let failed = |_| FeatureError::new(crate::regen::custom_features::error_text("EXTEND_FAILED"));
        // Only some edges picked (qa5 SF5: one edge of a 40 × 20 fill, 10 mm:
        // only that edge moves, 1000 mm²): the picked straight edges move out
        // in the plane, their neighbours run on to meet them.
        let some = !whole.contains(&bi) && picked.get(&bi).is_some_and(|es| es.len() < outer.edges().len());
        let (new_face, from) = if some {
            let chosen: Vec<Shape> = picked[&bi].iter().filter_map(|&ei| b.shape.sub_shape(ShapeType::Edge, ei).ok()).collect();
            let (f, from) = move_polygon_edges(face, &outer, &chosen, d, ev.bool("maintainCurvature"))?.ok_or_else(|| FeatureError::new("Move boundary currently supports straight edges of planar surfaces only"))?;
            (f, Some(from))
        } else {
            let moved = k::offset_wire_2d(&outer, d, k::JoinType::Intersection, false).map_err(failed)?;
            let wire = moved.wires().into_iter().next().unwrap_or(moved);
            (k::make_face_from_wire(&wire, true).map_err(failed)?, None)
        };
        let names = vec![b.faces[0].clone()];
        let mut body = Body::new(b.id.clone(), b.name.clone(), b.kind, new_face, names, b.color, ev.feature);
        if let Some(from) = from {
            keep_edge_names(&mut body, &b, &from)?;
        }
        ev.ctx.bodies[bi] = Arc::new(body);
    }
    Ok(())
}

/// A boundary edge of a planar face, in the boundary's direction.
enum Side {
    /// Through the point, along the unit direction.
    Line(DVec3, DVec3),
    /// On the circle (centre, radius), turning by `turn` (+1 / -1) about the
    /// plane normal from its start to its end.
    Arc { c: DVec3, r: f64, turn: f64 },
}

/// A planar face bounded by straight edges and circular arcs with `chosen`
/// straight edges moved out by `d` in its plane; their neighbours run on (or
/// back) to meet them: lines along themselves, arcs along their tangent
/// (along their circle with `curvature`, and when trimmed) (qa5 SF5, SF6: a
/// boundary surface's straight B-spline edge between two arcs). None when the face
/// has another kind of boundary edge or a chosen edge isn't straight.
/// The new face, and per new edge its middle and the boundary edge it comes
/// from (None for an arc's tangent extension).
type MovedFace = (Shape, Vec<(DVec3, Option<Shape>)>);

fn move_polygon_edges(face: &Shape, outer: &Shape, chosen: &[Shape], d: f64, curvature: bool) -> FResult<Option<MovedFace>> {
    let Some(pl) = face.face_plane()? else { return Ok(None) };
    let n = pl.normal.normalize();
    let chain = order_chain(&outer.edges(), 1e-7)?;
    let mut sides: Vec<Side> = Vec::new();
    let mut pts: Vec<DVec3> = Vec::new();
    let mut moved: Vec<bool> = Vec::new();
    for (e, reversed) in &chain {
        let info = e.edge_info()?;
        // In the chain's direction (order_chain walks endpoints()).
        let (s, t) = e.endpoints()?;
        let (a, b) = if *reversed { (t, s) } else { (s, t) };
        // Straight: a line, or any curve on its chord within 0.1 µm (a
        // boundary surface's straight B-spline edge).
        let chord = info.end - info.start;
        let straight = matches!(info.curve, k::Curve::Line { .. })
            || chord.length() > 1e-12 && {
                let off = |i: u32| -> FResult<f64> { Ok((e.point_at(info.first_param + (info.last_param - info.first_param) * i as f64 / 8.0)? - info.start).cross(chord.normalize()).length()) };
                (1..8).map(off).collect::<FResult<Vec<f64>>>()?.iter().all(|&o| o < 1e-7)
            };
        let is_chosen = chosen.iter().any(|c| c.is_same(e));
        let side = match info.curve {
            _ if straight => Side::Line(a, (b - a).normalize()),
            k::Curve::Circle { center, radius, .. } if !is_chosen && !info.closed => {
                let mid = e.point_at((info.first_param + info.last_param) * 0.5)?;
                let turn = if (a - center).cross(mid - center).dot(n) >= 0.0 { 1.0 } else { -1.0 };
                Side::Arc { c: center, r: radius, turn }
            }
            _ => return Ok(None),
        };
        pts.push(a);
        sides.push(side);
        moved.push(is_chosen);
    }
    let centre = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
    for (i, side) in sides.iter_mut().enumerate() {
        if let (true, Side::Line(a, t)) = (moved[i], &*side) {
            let mut out = t.cross(n).normalize();
            if (*a - centre).dot(out) < 0.0 {
                out = -out;
            }
            *side = Side::Line(*a + out * d, *t);
        }
    }
    // Corner i, where side i-1 meets side i (in the plane): the end of side
    // i-1, the start of side i, and a straight piece between them where an
    // arc is extended along its tangent (live's default, "Maintain
    // curvature" off) to a line moved out.
    let m = sides.len();
    let (mut ends, mut starts, mut bridges) = (vec![DVec3::ZERO; m], vec![DVec3::ZERO; m], vec![None; m]);
    let line_meets_circle = |p: DVec3, u: DVec3, c: DVec3, r: f64, near: DVec3| -> Option<DVec3> {
        let f = p - c;
        let (bq, cq) = (f.dot(u), f.length_squared() - r * r);
        let disc = bq * bq - cq;
        (disc >= 0.0).then(|| {
            let [x, y] = [-bq - disc.sqrt(), -bq + disc.sqrt()].map(|s| p + u * s);
            if x.distance(near) <= y.distance(near) { x } else { y }
        })
    };
    let line_meets_line = |p: DVec3, u: DVec3, q: DVec3, v: DVec3| -> Option<DVec3> {
        let w = u.cross(v);
        (w.length() >= 1e-12).then(|| p + u * ((q - p).cross(v).dot(w) / w.length_squared()))
    };
    for i in 0..m {
        let j = (i + m - 1) % m;
        let v = pts[i];
        let (end, start, bridge) = if !moved[i] && !moved[j] {
            (v, v, None)
        } else {
            match (&sides[j], &sides[i]) {
                // Collinear neighbours: the corner stays on the moved line.
                (Side::Line(p, u), Side::Line(q, w)) => {
                    let x = line_meets_line(*p, *u, *q, *w).unwrap_or(if moved[i] { *q } else { *p });
                    (x, x, None)
                }
                (Side::Arc { c, r, turn }, Side::Line(q, w)) | (Side::Line(q, w), Side::Arc { c, r, turn }) => {
                    let arc_first = matches!(sides[j], Side::Arc { .. });
                    // Out: past the arc's end, along its tangent there (or on
                    // round its circle with Maintain curvature). In: back
                    // along the circle.
                    let tangent = n.cross(v - *c).normalize() * *turn;
                    let along = line_meets_line(v, tangent, *q, *w);
                    let outward = d > 0.0;
                    if outward && !curvature {
                        let Some(t) = along else { return Ok(None) };
                        if arc_first { (v, t, Some((v, t))) } else { (t, v, Some((t, v))) }
                    } else {
                        let Some(x) = line_meets_circle(*q, *w, *c, *r, v) else { return Ok(None) };
                        (x, x, None)
                    }
                }
                _ => (v, v, None),
            }
        };
        ends[j] = end;
        starts[i] = start;
        bridges[i] = bridge;
    }
    // In the face's plane, so that consecutive pieces share their vertices.
    let at = |p: DVec3| pl.to_local(p);
    let mut segs = Vec::with_capacity(2 * m);
    let mut from = Vec::with_capacity(2 * m);
    for i in 0..m {
        if let Some((a, b)) = bridges[i] {
            segs.push(k::Segment2d::Line { a: at(a), b: at(b) });
            from.push(((a + b) * 0.5, None));
        }
        let (a, b) = (starts[i], ends[i]);
        segs.push(match &sides[i] {
            Side::Line(..) => {
                from.push(((a + b) * 0.5, Some(chain[i].0.clone())));
                k::Segment2d::Line { a: at(a), b: at(b) }
            }
            Side::Arc { c, turn, .. } => {
                // The arc's middle, turning the same way as the original.
                let (x, y) = ((a - *c).normalize(), n.cross((a - *c).normalize()) * *turn);
                let mut sweep = ((b - *c).dot(y)).atan2((b - *c).dot(x));
                if sweep <= 0.0 {
                    sweep += std::f64::consts::TAU;
                }
                let h = sweep * 0.5;
                let mid = *c + (x * h.cos() + y * h.sin()) * (a - *c).length();
                from.push((mid, Some(chain[i].0.clone())));
                k::Segment2d::Arc3 { a: at(a), mid: at(mid), b: at(b) }
            }
        });
    }
    let wire = k::make_wire_2d(&pl, &segs)?;
    Ok(Some((k::make_face(&pl, &wire, &[])?, from)))
}

/// The moved surface's edges keep the names of the edges they come from (a
/// later Move boundary of the same edge still finds it, qa5 SF6); an arc's
/// tangent extension is named after them.
fn keep_edge_names(body: &mut Body, old: &Body, from: &[(DVec3, Option<Shape>)]) -> FResult<()> {
    let old_edges = old.shape.edges();
    let mut extra = old.edges.len() as u16;
    for (k, e) in body.shape.edges().iter().enumerate() {
        let info = e.edge_info()?;
        let mid = e.point_at((info.first_param + info.last_param) * 0.5)?;
        let Some((_, src)) = from.iter().min_by(|a, b| a.0.distance(mid).total_cmp(&b.0.distance(mid))) else { continue };
        match src.as_ref().and_then(|s| old_edges.iter().position(|o| o.is_same(s))) {
            Some(i) => body.edges[k] = old.edges[i].clone(),
            None => {
                body.edges[k] = Arc::new(TopoName::Boundary { face: old.faces[0].clone(), ordinal: extra });
                extra += 1;
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ boundary surface

/// Onshape's BSURF_PROFILE_ENDS_MISMATCH (live L28).
const BSURF_ENDS: &str = "Could not extract boundary profiles matching at end points.";

/// The two free ends of a chain of edges.
fn chain_ends(edges: &[Shape]) -> Option<(DVec3, DVec3)> {
    let pts: Vec<DVec3> = edges.iter().filter_map(|e| e.endpoints().ok()).flat_map(|(a, b)| [a, b]).collect();
    let free: Vec<DVec3> = pts.iter().copied().filter(|p| pts.iter().filter(|q| q.distance(*p) < 1e-7).count() == 1).collect();
    match free.as_slice() {
        [a, b] => Some((*a, *b)),
        _ => None,
    }
}

pub fn boundary_surface(ev: &mut EvalCtx) -> FResult<()> {
    let collect = |ev: &EvalCtx, array: &str, ents: &str, cond: &str| -> FResult<(Vec<Vec<Shape>>, bool)> {
        let mut out = Vec::new();
        let mut conditions = false;
        for item in ev.params.array(array) {
            let edges = curve_edges(ev, item.query(ents))?;
            if !edges.is_empty() {
                out.push(edges);
            }
            conditions |= !matches!(item.enum_(cond), "" | "DEFAULT");
        }
        Ok((out, conditions))
    };
    let (u, cu) = collect(ev, "uProfilesArray", "uProfileEntities", "uCondition")?;
    let (v, cv) = collect(ev, "vProfilesArray", "vProfileEntities", "vCondition")?;
    // bsurf.fs: at least two profiles in all.
    if u.len() + v.len() < 2 {
        return crate::regen::eval::err_at(&["uProfilesArray", "vProfilesArray"], "Select edges to specify at least two profiles.");
    }
    // Live L28: a third profile in a direction is accepted by the list but
    // can't make the network.
    if u.len() > 2 || v.len() > 2 {
        return err("Could not create valid profiles from selections.");
    }
    // Live L28: a V profile must meet the U profiles at their ends (one
    // crossing a U profile in its middle fails), and the other way round.
    let meets_mid = |a: &[Shape], b: &[Shape]| -> bool {
        a.iter().any(|ea| {
            let ends: Vec<DVec3> = ea.endpoints().map(|(p, q)| vec![p, q]).unwrap_or_default();
            b.iter().any(|eb| match k::distance(ea, eb) {
                Ok(d) if d.distance < 1e-7 => ends.iter().all(|e| e.distance(d.point_a) > 1e-6),
                _ => false,
            })
        })
    };
    if !u.is_empty() && !v.is_empty() && u.iter().any(|pu| v.iter().any(|pv| meets_mid(pu, pv) && meets_mid(pv, pu))) {
        return err(BSURF_ENDS);
    }
    if (cu || cv) && !u.is_empty() && !v.is_empty() {
        ev.warnings.push("Boundary conditions are not applied yet; profiles are matched by position".into());
    }
    let shape = if !u.is_empty() && !v.is_empty() {
        // Four-sided (or three-sided) patch through all boundary curves. Live
        // L28: one U and one V meeting at an end give the patch the U profile
        // sweeps along the V one; its other two sides are the profiles moved
        // to each other's far end.
        let mut all: Vec<Shape> = u.iter().chain(v.iter()).flatten().cloned().collect();
        if let ([pu], [pv]) = (u.as_slice(), v.as_slice()) {
            let (Some(eu), Some(ev_)) = (chain_ends(pu), chain_ends(pv)) else { return err(BSURF_ENDS) };
            // Live (behaviours "skasm (2026-10-31, P3 surfaces row 12)"): one U
            // and one V profile that don't meet mark U profiles (pink).
            let Some((cu, cv)) = [eu.0, eu.1].into_iter().flat_map(|a| [(a, ev_.0), (a, ev_.1)]).find(|(a, b)| a.distance(*b) < 1e-7) else { return crate::regen::eval::err_at(&["uProfilesArray"], BSURF_ENDS) };
            let far = |ends: (DVec3, DVec3), c: DVec3| if ends.0.distance(c) < 1e-7 { ends.1 } else { ends.0 };
            let moved = |edges: &[Shape], by: DVec3| -> FResult<Vec<Shape>> { edges.iter().map(|e| Ok(k::transform(e, &k::Transform::Translate(by), true)?.shape)).collect() };
            all.extend(moved(pu, far(ev_, cv) - cv)?);
            all.extend(moved(pv, far(eu, cu) - cu)?);
        }
        let boundary: Vec<k::FillEdge> = all.into_iter().map(|e| k::FillEdge { edge: e, support: None, continuity: k::Continuity::Position }).collect();
        k::fill_surface(&boundary, &[]).map_err(|_| FeatureError::new(BSURF_FAILED))?.shape
    } else {
        let profiles = if u.is_empty() { &v } else { &u };
        let sections: Vec<Shape> = profiles.iter().map(|es| k::make_wire(&order_chain(es, 1e-6)?.into_iter().map(|(e, _)| e).collect::<Vec<_>>()).map_err(|_| FeatureError::new(BSURF_FAILED))).collect::<FResult<_>>()?;
        // The first and last rows' conditions are the loft's end conditions
        // (loft.fs createProfileConditions; live L28's planar-profile text).
        let (array, ents, ids) = if u.is_empty() {
            ("vProfilesArray", "vProfileEntities", ["vCondition", "vMagnitude", "vDirection"])
        } else {
            ("uProfilesArray", "uProfileEntities", ["uCondition", "uMagnitude", "uDirection"])
        };
        let items: Vec<crate::params::ParamValues> = ev.params.array(array).iter().filter(|i| !i.query(ents).is_empty()).cloned().collect();
        let sketch_normal = |ev: &EvalCtx, item: &crate::params::ParamValues| {
            item.query(ents).iter().find_map(|r| match r {
                Reference::SketchEntity { sketch, .. } | Reference::Sketch(sketch) => ev.ctx.sketch(*sketch).map(|s| s.frame.normal),
                _ => None,
            })
        };
        let mut opts = k::LoftOptions::default();
        if let ([first, last], [s0, s1]) = (items.as_slice(), sections.as_slice()) {
            opts.start = super::solid_a::loft_end(ev, first, ids, s0, sketch_normal(ev, first), "start")?;
            opts.end = super::solid_a::loft_end(ev, last, ids, s1, sketch_normal(ev, last), "end")?;
        }
        k::loft(&sections, &opts).map_err(|_| FeatureError::new(BSURF_FAILED))?.shape
    };
    let names = new_face_names(ev, &shape);
    surface_result(ev, NamedShape::new(shape, names))
}

// ------------------------------------------------------------------ ruled surface

pub fn ruled_surface(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("edges");
    if refs.is_empty() {
        return err("Select edges for the ruled surface");
    }
    // Vertex overrides: (vertex position, item) - applied at matching edge ends.
    let mut overrides: Vec<(DVec3, crate::params::ParamValues)> = Vec::new();
    for item in ev.params.array("vertexOverrides") {
        let Some(r) = item.query("vertex").first() else { continue };
        let Some(p) = ev.resolve_vertex_point(r) else { return err("A vertex override no longer exists") };
        overrides.push((p, item.clone()));
    }
    let edge_refs: Vec<(Shape, Source)> = {
        let mut v = Vec::new();
        for (i, r) in refs.iter().enumerate() {
            for (j, e) in curve_edges(ev, std::slice::from_ref(r))?.into_iter().enumerate() {
                let src = match r {
                    Reference::Edge(n) => Source::Topo(Arc::new(n.clone())),
                    Reference::SketchEntity { sketch, entity } => Source::Sketch { sketch: *sketch, entity: *entity, segment: j as u16 },
                    _ => Source::Index((i * 1000 + j) as u32),
                };
                v.push((e, src));
            }
        }
        v
    };
    let ty = ev.enum_("ruledType").to_string();
    let dist = ev.length("distance")? * if ev.bool("oppositeDirection") { -1.0 } else { 1.0 };
    let vector = match ty.as_str() {
        "ALIGNED_WITH_VECTOR" | "ANGLE_FROM_VECTOR" => {
            let Some(r) = ev.query("axis").first() else { return err("Select a direction") };
            Some(ev.resolve_direction(r).ok_or_else(|| FeatureError::new("Select a direction"))?.normalize())
        }
        _ => None,
    };
    let angle = match ty.as_str() {
        "ANGLE_FROM_VECTOR" => ev.angle("angleFromVector")? * if ev.bool("oppositeAngleFromVector") { -1.0 } else { 1.0 },
        "NORMAL" | "TANGENT" => ev.angle("angle")? * if ev.bool("oppositeAngle") { -1.0 } else { 1.0 },
        _ => 0.0,
    };
    let ref_faces: Vec<Shape> = ev.resolve_faces(ev.query("referenceFaces")).into_iter().map(|(bi, fi)| ev.face_shape(bi, fi)).collect::<FResult<_>>()?;
    let mut pieces = Vec::new();
    let mut piece_names = Vec::new();
    for (e, src) in &edge_refs {
        let face = if matches!(ty.as_str(), "NORMAL" | "TANGENT") {
            let f = ref_faces
                .iter()
                .find(|f| k::distance(f, e).map(|d| d.distance < 1e-6).unwrap_or(false))
                .cloned()
                .or_else(|| super::surfaces::adjacent_face(ev, e));
            Some(f.ok_or_else(|| FeatureError::new("Select reference faces for normal or tangent ruled surfaces"))?)
        } else {
            None
        };
        let centroid = face.as_ref().and_then(|f| f.centroid().ok());
        let dir_at = |p: DVec3, t: DVec3| -> FResult<DVec3> {
            let t = t.normalize_or(DVec3::X);
            let d = match ty.as_str() {
                "ALIGNED_WITH_VECTOR" => vector.unwrap_or(DVec3::Z),
                "ANGLE_FROM_VECTOR" => {
                    let v = vector.unwrap_or(DVec3::Z);
                    let b = (v - t * t.dot(v)).normalize_or(t.any_orthonormal_vector());
                    // Rotate the base direction away from the vector by the angle.
                    let side = t.cross(b);
                    b * angle.cos() + side * angle.sin()
                }
                _ => {
                    let f = face.as_ref().ok_or_else(|| FeatureError::new("Missing reference face"))?;
                    let n = f.normal_at_point(p)?;
                    let base = if ty == "NORMAL" {
                        n
                    } else {
                        let mut s = n.cross(t).normalize_or(n);
                        if let Some(c) = centroid
                            && (c - p).dot(s) > 0.0
                        {
                            s = -s;
                        }
                        s
                    };
                    if angle != 0.0 { DQuat::from_axis_angle(t, angle) * base } else { base }
                }
            };
            Ok(d.normalize())
        };
        let info = e.edge_info()?;
        let n = 24;
        let mut dirs = Vec::with_capacity(n + 1);
        let mut pts = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let prm = info.first_param + (info.last_param - info.first_param) * i as f64 / n as f64;
            let p = e.point_at(prm)?;
            dirs.push(dir_at(p, e.tangent_at(prm)?)?);
            pts.push(p);
        }
        // Ruling vectors, with vertex overrides blended in from the edge ends.
        let mut vecs: Vec<DVec3> = dirs.iter().map(|d| *d * dist).collect();
        let tangents = [e.tangent_at(info.first_param)?, e.tangent_at(info.last_param)?];
        for (end, idx) in [0usize, n].into_iter().enumerate() {
            let p = pts[idx];
            let Some((_, item)) = overrides.iter().find(|(q, _)| (*q - p).length() < 1e-7) else { continue };
            let t = tangents[end].normalize_or(DVec3::X);
            let ovr = item.enum_("overrideType").to_string();
            let target = if ovr == "UP_TO_VERTEX" {
                let Some(b) = item.query("boundaryOverride").first().and_then(|r| ev.resolve_vertex_point(r)) else { return err("Select the vertex to rule up to") };
                b - p
            } else {
                let d_len = ev.eval_num_in(item, "distanceOverride", crate::params::NumKind::Length)? * if item.bool("oppositeDirectionOverride") { -1.0 } else { 1.0 };
                let dir = match ovr.as_str() {
                    "ALIGNED_WITH_VECTOR" => {
                        let Some(r) = item.query("axisOverride").first() else { return err("Select the override direction") };
                        ev.resolve_direction(r).ok_or_else(|| FeatureError::new("The override direction no longer exists"))?.normalize()
                    }
                    _ => {
                        let f = face.as_ref().cloned().or_else(|| super::surfaces::adjacent_face(ev, e)).ok_or_else(|| FeatureError::new("Normal and tangent overrides need a reference face"))?;
                        let nrm = f.normal_at_point(p)?;
                        let base = if ovr == "NORMAL" { nrm } else { nrm.cross(t).normalize_or(nrm) };
                        let a = ev.eval_num_in(item, "angleOverride", crate::params::NumKind::Angle)? * if item.bool("oppositeAngleOverride") { -1.0 } else { 1.0 };
                        (DQuat::from_axis_angle(t, a) * base).normalize()
                    }
                };
                dir * d_len
            };
            let delta = target - vecs[idx];
            for (i, v) in vecs.iter_mut().enumerate() {
                let s = i as f64 / n as f64;
                let w = if idx == 0 { 1.0 - s } else { s };
                *v += delta * w;
            }
        }
        let constant = vecs.iter().all(|d| (*d - vecs[0]).length() < 1e-12);
        let res = if constant {
            k::extrude(e, vecs[0])?
        } else {
            let far: Vec<DVec3> = pts.iter().zip(&vecs).map(|(p, d)| *p + *d).collect();
            let far_edge = k::make_interpolated_spline(&far, &[], false)?;
            k::loft(&[k::make_wire(std::slice::from_ref(e))?, k::make_wire(&[far_edge])?], &k::LoftOptions { solid: false, ruled: true, closed: false, ..Default::default() })?
        };
        let nf = res.shape.count(ShapeType::Face);
        for i in 0..nf {
            let role = Role::Side(src.clone());
            piece_names.push(Arc::new(if nf == 1 { TopoName::face(ev.feature, role) } else { TopoName::Split { parent: Arc::new(TopoName::face(ev.feature, role)), by: ev.feature, ordinal: i as u16 } }));
        }
        pieces.push(res.shape);
    }
    let (shape, names) = if pieces.len() == 1 {
        (pieces.remove(0), piece_names)
    } else {
        let c = Shape::compound(&pieces);
        match k::sew(&pieces, 1e-6, false) {
            Ok(r) if r.shape.count(ShapeType::Face) == piece_names.len() => {
                let all: Vec<Shape> = c.faces();
                let names: Vec<Name> = r
                    .shape
                    .faces()
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let cf = f.centroid().ok();
                        all.iter().position(|g| cf.is_some_and(|cf| g.centroid().map(|gc| (gc - cf).length() < 1e-7).unwrap_or(false))).and_then(|j| piece_names.get(j).cloned()).unwrap_or_else(|| face_name(ev, Role::New(i as u32)))
                    })
                    .collect();
                (r.shape, names)
            }
            _ => (c, piece_names),
        }
    };
    surface_result(ev, NamedShape { names: names_for(&shape, names), shape })
}

// ------------------------------------------------------------------ mutual trim

pub fn mutual_trim(ev: &mut EvalCtx) -> FResult<()> {
    let b1 = ev.resolve_bodies(ev.query("body1")).first().copied().ok_or_else(|| FeatureError::new("Select the first surface"))?;
    let b2 = ev.resolve_bodies(ev.query("body2")).first().copied().ok_or_else(|| FeatureError::new("Select the second surface"))?;
    if b1 == b2 {
        return err("Select two different surfaces");
    }
    for bi in [b1, b2] {
        if ev.ctx.bodies[bi].kind != BodyKind::Surface {
            return err("Mutual trim works on surfaces only");
        }
    }
    let mut kept: Vec<(usize, Shape, Vec<Name>)> = Vec::new();
    for (me, other, flip) in [(b1, b2, ev.bool("keepOtherSide1")), (b2, b1, ev.bool("keepOtherSide2"))] {
        let body = ev.ctx.bodies[me].clone();
        let tool = ev.ctx.bodies[other].shape.clone();
        let res = k::split(std::slice::from_ref(&body.shape), std::slice::from_ref(&tool), &k::BooleanOptions::default())?;
        let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &Default::default(), None);
        let faces = res.shape.faces();
        if faces.len() < 2 {
            return err("The surfaces do not intersect");
        }
        // Group faces into connected sides via sewing; keep the larger side by default.
        let groups = side_groups(&faces, &tool);
        if groups.len() < 2 {
            return err("The surfaces do not cut each other");
        }
        let area = |g: &Vec<usize>| g.iter().map(|&i| faces[i].area().unwrap_or(0.0)).sum::<f64>();
        let big = (0..groups.len()).max_by(|a, b| area(&groups[*a]).total_cmp(&area(&groups[*b]))).unwrap_or(0);
        let chosen: Vec<usize> = if flip { groups.iter().enumerate().filter(|(i, _)| *i != big).flat_map(|(_, g)| g.clone()).collect() } else { groups[big].clone() };
        let fs: Vec<Shape> = chosen.iter().map(|&i| faces[i].clone()).collect();
        let ns: Vec<Name> = chosen.iter().map(|&i| names[i].clone()).collect();
        kept.push((me, if fs.len() == 1 { fs[0].clone() } else { Shape::compound(&fs) }, ns));
    }
    let first = ev.ctx.bodies[b1].clone();
    let ids = [ev.ctx.bodies[b1].id.clone(), ev.ctx.bodies[b2].id.clone()];
    let mut new_bodies = Vec::new();
    if ev.bool("merge") {
        let shapes: Vec<Shape> = kept.iter().map(|k| k.1.clone()).collect();
        let all_faces: Vec<Shape> = shapes.iter().flat_map(|s| s.faces()).collect();
        let all_names: Vec<Name> = kept.iter().flat_map(|k| k.2.clone()).collect();
        let sewn = k::sew(&shapes, 1e-6, true).or_else(|_| k::sew(&shapes, 1e-6, false))?;
        let names: Vec<Name> = sewn
            .shape
            .faces()
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let c = f.centroid().ok();
                all_faces.iter().position(|g| c.is_some_and(|c| g.centroid().map(|gc| (gc - c).length() < 1e-7).unwrap_or(false))).and_then(|j| all_names.get(j).cloned()).unwrap_or_else(|| face_name(ev, Role::New(i as u32)))
            })
            .collect();
        let kind = if sewn.shape.count(ShapeType::Solid) > 0 { BodyKind::Solid } else { BodyKind::Surface };
        new_bodies.push(Body::new(first.id.clone(), first.name.clone(), kind, sewn.shape, names, first.color, ev.feature));
    } else {
        for (me, shape, names) in kept {
            let b = ev.ctx.bodies[me].clone();
            new_bodies.push(Body::new(b.id.clone(), b.name.clone(), b.kind, shape, names, b.color, ev.feature));
        }
    }
    let pos = b1.min(b2);
    ev.ctx.bodies.retain(|b| !ids.contains(&b.id));
    for (j, b) in new_bodies.into_iter().enumerate() {
        let at = (pos + j).min(ev.ctx.bodies.len());
        ev.ctx.bodies.insert(at, Arc::new(b));
    }
    let _ = BodyId { feature: ev.feature, index: 0 };
    Ok(())
}

/// Connected groups of faces (sharing an edge that does not lie on `cut`).
fn side_groups(faces: &[Shape], cut: &Shape) -> Vec<Vec<usize>> {
    let n = faces.len();
    let on_cut = |e: &Shape| -> bool {
        let Ok(i) = e.edge_info() else { return false };
        let Ok(m) = e.point_at((i.first_param + i.last_param) * 0.5) else { return false };
        k::make_vertex(m).and_then(|v| k::distance(cut, &v)).map(|d| d.distance < 1e-6).unwrap_or(false)
    };
    let edges: Vec<Vec<Shape>> = faces.iter().map(|f| f.edges().into_iter().filter(|e| !on_cut(e)).collect()).collect();
    let mut group = vec![usize::MAX; n];
    let mut out: Vec<Vec<usize>> = Vec::new();
    for s in 0..n {
        if group[s] != usize::MAX {
            continue;
        }
        let gi = out.len();
        let mut stack = vec![s];
        group[s] = gi;
        let mut members = Vec::new();
        while let Some(i) = stack.pop() {
            members.push(i);
            for j in 0..n {
                if group[j] == usize::MAX && edges[i].iter().any(|a| edges[j].iter().any(|b| a.is_same(b))) {
                    group[j] = gi;
                    stack.push(j);
                }
            }
        }
        out.push(members);
    }
    out
}

