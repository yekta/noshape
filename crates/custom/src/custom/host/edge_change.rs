//! `@opEdgeChange` (@internal, std geomOperations.fs): move sheet edges
//! within their faces. std sheet metal pulls a hem's edge back by the hem's
//! extent with it (Hem alignment OUTER, live dom/fs-smprobe3-7.txt). An
//! option's `offset` moves its edge's line in the face plane, outwards
//! when positive; the neighbouring sides keep their lines, so their ends
//! follow. Only the faces with a moved edge change: a wall meeting the
//! moved corner keeps its outline (live qa5 Hem on G34: the front flange
//! still runs to x = 60, 9378.1835 mm³). Faces keep their names (and their
//! edges, named from them, theirs).
//! An option's `replaceFace` (a plane) moves its edge onto that face: the line where the face's plane meets it
//! (std sheet metal's Shaped Flange runs a wall's edge up to the face it adds). `transformList` is not supported
//! yet.

use super::query::evaluate_refs;
use super::{context_arg, convert::*, run_op};
use crate::regen::eval::{FResult, FeatureError, err};
use glam::DVec3;
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::Value;
use noshape_kernel as k;
use noshape_kernel::Shape;
use std::collections::BTreeMap;

pub const NATIVES: &[(&str, NativeFn)] = &[("opEdgeChange", op_edge_change), ("opDerip", op_derip)];

const TOL: f64 = 1e-7;

/// Where an edge goes: its line moved in its face's plane by this much (outwards when positive), or onto a plane
/// (origin, normal).
#[derive(Clone, Copy, Debug)]
enum Move {
    Offset(f64),
    Onto(DVec3, DVec3),
}

/// One edge change: the edges, by how much they move (or onto which face), and the face whose side of them moves
/// (an edge between two faces moved on one side only comes apart there: std's derip).
struct EdgeOption {
    edges: Vec<crate::reference::Reference>,
    offset: f64,
    replace: Option<Vec<crate::reference::Reference>>,
    face: Option<Vec<crate::reference::Reference>>,
    /// Onto this plane (origin, normal): an extend's limit given as a Plane.
    plane: Option<(DVec3, DVec3)>,
}

fn op_edge_change(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let def = def_arg(i, &a, 2)?;
    let mut options = Vec::new();
    for o in def.get("edgeChangeOptions").as_array().map(|a| a.iter().cloned().collect::<Vec<_>>()).unwrap_or_default() {
        if !matches!(o.get("transformList"), Value::Undefined) {
            return Err(i.error("opEdgeChange: transformList edge changes are not supported"));
        }
        let edges = evaluate_refs(i, &cx, o.get("edge"))?;
        let replace = match o.get("replaceFace") {
            Value::Undefined => None,
            f => Some(evaluate_refs(i, &cx, f)?),
        };
        let face = match o.get("face") {
            Value::Undefined => None,
            f => Some(evaluate_refs(i, &cx, f)?),
        };
        options.push(EdgeOption { edges, offset: length_of(o.get("offset")).unwrap_or(0.0), replace, face, plane: None });
    }
    change_edges_op(i, &cx, &id, options, true)
}

/// `@opExtendSheetBody` up to a surface (std sheet metal: Make joint's walls to their bisector, a Flange's alignment,
/// Move face): each edge moves in its face onto its limit, a plane or a planar face, as an edge change; walls whose
/// edges meet there are sewn together.
pub(super) fn extend_to_surface(i: &mut Interp, cx: &super::FsContext, id: &[std::sync::Arc<str>], def: &Value) -> R<Value> {
    if length_of(def.get("offset")).is_some_and(|o| o.abs() > TOL) {
        return Err(i.error("opExtendSheetBody: an offset past the limit is not supported yet"));
    }
    let items: Vec<Value> = match def.get("edgeLimitOptions").as_array() {
        Some(a) => a.iter().cloned().collect(),
        None => vec![def.clone()],
    };
    let mut options = Vec::new();
    for o in items {
        let edges = evaluate_refs(i, cx, match o.get("edge") {
            Value::Undefined => o.get("entities"),
            e => e,
        })?;
        let limit = o.get("limitEntity");
        let (plane, replace) = match plane_of(limit) {
            Some(p) => (Some((p.origin, p.normal)), None),
            None => (None, Some(evaluate_refs(i, cx, limit)?)),
        };
        let face = match o.get("faceToExtend") {
            Value::Undefined => None,
            f => Some(evaluate_refs(i, cx, f)?),
        };
        options.push(EdgeOption { edges, offset: 0.0, replace, face, plane });
    }
    change_edges_op(i, cx, id, options, true)
}

/// `@opDerip` (std sheetMetalDeripCall): each sheet metal definition RIP edge comes apart, each of its faces' side
/// moved onto the part face that corresponds to it: std's own createDeripOptions (moveFace.fs), found through the
/// sheet metal association attributes (live 2005 T2: a Tab into a ripped corner extends its wall; the other wall
/// keeps its edge, so the clearance stays).
fn op_derip(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    use super::query::{EType, Ent, with_ents};
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let def = def_arg(i, &a, 2)?;
    let options = with_ents(i, &cx, def.get("edges"), |_, q, ents| {
        let assoc = |e: &Ent| -> Vec<Value> {
            q.st.attributes_of(&q.to_ref(e)).into_iter().map(|v| v.get("attribute").clone()).filter(|v| !v.get("attributeId").is_undefined()).collect()
        };
        let mut out = Vec::new();
        for e in ents {
            let Ent::Edge(b, ei) = e else { continue };
            let sides = q.adjacent(&e, EType::Face, false);
            let Some(edge) = q.st.ctx.bodies[b].shape.edges().get(ei).cloned() else { continue };
            let own = assoc(&e);
            // The part faces sharing the edge's association (solid bodies only).
            let part_faces: Vec<Ent> = q
                .st
                .attributes()
                .iter()
                .filter(|(_, v)| own.contains(v.get("attribute")))
                .flat_map(|(r, _)| q.from_ref(r))
                .filter(|x| matches!(x, Ent::Face(pb, _) if q.st.ctx.bodies[*pb].kind == crate::display::BodyKind::Solid))
                .collect();
            for pf in super::query::uniq(part_faces) {
                let Ent::Face(pb, pi) = pf else { continue };
                // The definition face next to the edge that the part face's neighbours come from.
                let near: Vec<Value> = q.adjacent(&pf, EType::Face, false).iter().flat_map(&assoc).collect();
                let defs: Vec<&Ent> = sides.iter().filter(|f| assoc(f).iter().any(|v| near.contains(v))).collect();
                let [side] = defs.as_slice() else { continue };
                let Some(face) = q.st.ctx.bodies[pb].shape.faces().get(pi).cloned() else { continue };
                // std's signed distance: from the edge to the part face's closest point, along the face's normal there
                // (a curved part face's too: a rip on a rolled wall).
                let Ok(d) = k::distance(&face, &edge) else { continue };
                let Ok(n) = face.normal_at_point(d.point_a) else { continue };
                let offset = n.normalize().dot(d.point_a - d.point_b);
                out.push(EdgeOption { edges: vec![q.to_ref(&e)], offset, replace: None, face: Some(vec![q.to_ref(side)]), plane: None });
            }
        }
        Ok(out)
    })?;
    if super::trace_ops() {
        eprintln!("[fs op] derip options {:?}", options.iter().map(|o| (o.offset, &o.face)).collect::<Vec<_>>());
    }
    if options.is_empty() {
        return Ok(Value::Undefined);
    }
    change_edges_op(i, &cx, &id, options, false)
}

/// `join`: a moved edge's neighbour that now runs on along another edge between the same faces becomes one edge with
/// it (an extended wall's bend edge, live cf-sheet-metal-shaped-flange's Make joint; a raised wall's RIP edge, live 2053).
fn change_edges_op(i: &mut Interp, cx: &super::FsContext, id: &[std::sync::Arc<str>], options: Vec<EdgeOption>, join: bool) -> R<Value> {
    // The attributes on the edges it moves (Parasolid's stay on a moved edge; the operation's attribute remap, by
    // where they were, would drop them).
    let kept: Vec<(Value, crate::reference::Reference, super::query::Snapshot)> = {
        let st = cx.state.lock();
        options
            .iter()
            .flat_map(|o| o.edges.iter())
            .filter_map(|r| st.attr_snapshots.get(r).map(|(_, snap)| (r, snap.clone())))
            .flat_map(|(r, snap)| st.attributes_of(r).into_iter().map(move |v| (v.clone(), r.clone(), snap.clone())).collect::<Vec<_>>())
            .collect()
    };
    let moves = run_op(i, cx, id, "REGEN_ERROR", |ev| {
        let mut moves: Vec<(Vec<DVec3>, Vec<DVec3>)> = Vec::new();
        let mut by_body: BTreeMap<usize, BTreeMap<(usize, Option<usize>), Move>> = BTreeMap::new();
        for EdgeOption { edges, offset, replace, face, plane } in &options {
            if super::trace_ops() {
                eprintln!("[fs op] edgeChange {:?} -> {:?} offset {offset} replace {replace:?} plane {plane:?}", edges, ev.resolve_edges(edges));
            }
            let mv = match (plane, replace) {
                (Some((o, n)), _) => Move::Onto(*o, n.normalize()),
                (None, None) => Move::Offset(*offset),
                (None, Some(refs)) => match ev.resolve_faces(refs).first() {
                    Some(&(fb, fi)) => {
                        let pl = ev.face_shape(fb, fi)?.face_plane().map_err(fe)?.ok_or_else(|| fe("Only planar faces can replace an edge yet"))?;
                        Move::Onto(pl.origin, pl.normal.normalize())
                    }
                    // A sketch region: its sketch's plane.
                    None => {
                        let p = ev.profiles(refs)?.into_iter().next().ok_or_else(|| fe("Cannot resolve the face to replace the edge with"))?;
                        Move::Onto(p.frame.origin, p.frame.normal.normalize())
                    }
                },
            };
            let side = face.as_ref().map(|f| ev.resolve_faces(f));
            for (b, e) in ev.resolve_edges(edges) {
                match &side {
                    None => {
                        by_body.entry(b).or_default().insert((e, None), mv);
                    }
                    Some(faces) => {
                        for &(_, fi) in faces.iter().filter(|(fb, _)| *fb == b) {
                            by_body.entry(b).or_default().insert((e, Some(fi)), mv);
                        }
                    }
                }
            }
        }
        for (b, offsets) in by_body {
            let body = ev.ctx.bodies[b].clone();
            let (shape, order, edge_from) = change_edges(&body.shape, &offsets, join)?;
            // Where each moved edge went (tracking finds it there).
            let (old, new) = (body.shape.edges(), shape.edges());
            for (i, from) in edge_from.iter().enumerate() {
                if let Some(j) = from.filter(|j| offsets.keys().any(|(e, _)| e == j))
                    && let (Some(a), Some(b)) = (old.get(j).and_then(edge_points), new.get(i).and_then(edge_points))
                {
                    moves.push((a, b));
                }
            }
            let names = order.iter().map(|j| body.faces[*j].clone()).collect();
            ev.replace_body(b, shape, names);
            keep_edge_names(ev, b, &body.edges, &edge_from);
        }
        Ok(moves)
    })?;
    let mut st = cx.state.lock();
    st.edge_moves.extend(moves);
    // Each dropped attribute back on the edge where its edge went.
    let back: Vec<(crate::reference::Reference, Value)> = {
        let q = super::query::QueryCx::new(&st);
        kept.into_iter()
            .filter(|(v, r, _)| !st.attributes_of(r).contains(&v))
            .filter_map(|(v, _, snap)| q.tracked(&snap, false).first().map(|e| (q.to_ref(e), v)))
            .filter(|(r, v)| !st.attributes_of(r).contains(&v))
            .collect()
    };
    for (r, v) in back {
        st.push_attribute(r, v);
    }
    Ok(Value::Undefined)
}

/// An edge's start, middle and end (as a tracking snapshot has them).
pub(super) fn edge_points(e: &Shape) -> Option<Vec<DVec3>> {
    let i = e.edge_info().ok()?;
    [0.0, 0.5, 1.0].iter().map(|t| e.point_at(i.first_param + (i.last_param - i.first_param) * t).ok()).collect()
}

/// The changed body's edges keep the names of the edges they were, moved or
/// not (std tracks a moved edge by name: a partial flange's piece of an edge
/// that sheds two step edges shifted the boundary edges' ordinals); new
/// edges are named apart from them.
pub(super) fn keep_edge_names(ev: &mut crate::regen::eval::EvalCtx, b: usize, old: &[crate::naming::Name], edge_from: &[Option<usize>]) {
    use crate::reference::TopoName;
    let nb = ev.ctx.bodies[b].clone();
    if nb.edges.len() != edge_from.len() {
        return;
    }
    let mut used = std::collections::HashSet::new();
    let mut edges = nb.edges.clone();
    let mut fresh = 0u16;
    for (i, from) in edge_from.iter().enumerate() {
        match from.and_then(|j| old.get(j)) {
            Some(n) if used.insert(n.clone()) => edges[i] = n.clone(),
            _ => {
                if let TopoName::Boundary { face, .. } = &*edges[i] {
                    edges[i] = std::sync::Arc::new(TopoName::Boundary { face: face.clone(), ordinal: u16::MAX - fresh });
                    fresh += 1;
                }
            }
        }
    }
    let names = crate::naming::ShapeNames { faces: nb.faces.clone(), edges, vertices: nb.vertices.clone() };
    ev.ctx.bodies[b] = std::sync::Arc::new(crate::regen::context::Body::named(nb.id.clone(), nb.name.clone(), nb.kind, nb.shape.clone(), names, nb.color, ev.feature));
}

fn fe(e: impl ToString) -> FeatureError {
    FeatureError::new(e.to_string())
}

/// A planar face's oriented normal.
fn normal(f: &Shape) -> FResult<DVec3> {
    let [u0, u1, v0, v1] = f.uv_bounds().map_err(fe)?;
    Ok(f.normal_at_uv((u0 + u1) * 0.5, (v0 + v1) * 0.5).map_err(fe)?.normalize())
}

/// A face loop's corners, side moves and side edges (see [`face_loop`]).
type FaceLoop = (Vec<DVec3>, Vec<Option<Move>>, Vec<Option<usize>>);

/// A face's outer loop: corners, the move of each side (`pts[k]` to
/// `pts[k + 1]`) if it moves, and the index of each side's edge in `all`.
fn face_loop(f: &Shape, moved: &[(Shape, Move)], all: &[Shape]) -> FResult<FaceLoop> {
    let wires = f.wires();
    if wires.len() != 1 {
        return err("Edges of faces with holes cannot be changed yet");
    }
    let mut segs = Vec::new();
    for e in wires[0].edges() {
        let info = e.edge_info().map_err(fe)?;
        if !matches!(info.curve, k::Curve::Line { .. }) {
            return err("Only straight edges can be changed yet");
        }
        segs.push((info.start, info.end, moved.iter().find(|(m, _)| m.is_same(&e)).map(|x| x.1), all.iter().position(|x| x.is_same(&e))));
    }
    let (first, mut at, offset, from) = segs.swap_remove(0);
    let (mut pts, mut sides, mut froms) = (vec![first], vec![offset], vec![from]);
    while !segs.is_empty() {
        let Some(k) = segs.iter().position(|(a, b, _, _)| a.distance(at) < TOL || b.distance(at) < TOL) else { return err("A face boundary is not closed") };
        let (a, b, offset, from) = segs.swap_remove(k);
        pts.push(at);
        sides.push(offset);
        froms.push(from);
        at = if a.distance(at) < TOL { b } else { a };
    }
    Ok((pts, sides, froms))
}

/// An old edge's index and its new segment (start, end).
type Segment = (usize, DVec3, DVec3);

/// A face with curved edges whose straight edges move: its other edges kept as they are, a moved edge's straight
/// neighbours run to its new ends (a curved neighbour of a moved edge isn't supported). The face and each old
/// straight edge's new segment.
fn change_curved_face(f: &Shape, moved: &[(Shape, Move)], all: &[Shape]) -> FResult<(Shape, Vec<Segment>)> {
    let wires = f.wires();
    if wires.len() != 1 {
        return err("Edges of faces with holes cannot be changed yet");
    }
    let n = normal(f)?;
    // The loop's edges in order, each with its start and end along the loop.
    let mut rest: Vec<(Shape, DVec3, DVec3)> = wires[0].edges().into_iter().map(|e| e.edge_info().map(|i| (e, i.start, i.end)).map_err(fe)).collect::<FResult<_>>()?;
    let mut order: Vec<(Shape, DVec3, DVec3)> = vec![rest.swap_remove(0)];
    while !rest.is_empty() {
        let at = order[order.len() - 1].2;
        let Some(k) = rest.iter().position(|(_, a, b)| a.distance(at) < TOL || b.distance(at) < TOL) else { return err("A face boundary is not closed") };
        let (e, a, b) = rest.swap_remove(k);
        order.push(if a.distance(at) < TOL { (e, a, b) } else { (e, b, a) });
    }
    let m = order.len();
    let straight = |e: &Shape| e.edge_info().is_ok_and(|i| matches!(i.curve, k::Curve::Line { .. }));
    let mv = |e: &Shape| moved.iter().find(|(x, _)| x.is_same(e)).map(|x| x.1);
    let area: DVec3 = (0..m).map(|k| order[k].1.cross(order[(k + 1) % m].1)).sum();
    let ccw = area.dot(n) > 0.0;
    // An edge's line where it goes (as `change_edges`' straight faces).
    let line = |k: usize| -> (DVec3, DVec3) {
        let (_, p, q) = &order[k];
        let d = (*q - *p).normalize();
        let out = if ccw { d.cross(n) } else { n.cross(d) };
        match mv(&order[k].0) {
            None => (*p, d),
            Some(Move::Offset(o)) => (*p + out * o, d),
            Some(Move::Onto(o, pm)) => {
                let den = out.dot(pm);
                let t = if den.abs() < 1e-12 { 0.0 } else { (o - *p).dot(pm) / den };
                (*p + out * t, d)
            }
        }
    };
    // The new corner at each edge's start where it or the one before moves.
    let mut start: Vec<DVec3> = order.iter().map(|x| x.1).collect();
    for k in 0..m {
        let prev = (k + m - 1) % m;
        if mv(&order[prev].0).is_none() && mv(&order[k].0).is_none() {
            continue;
        }
        if !straight(&order[prev].0) || !straight(&order[k].0) {
            return err("Only straight edges next to a changed edge can be changed yet");
        }
        let ((p0, d0), (p1, d1)) = (line(prev), line(k));
        let c = d0.cross(d1);
        if c.length_squared() < 1e-20 {
            continue;
        }
        let t = (p1 - p0).cross(d1).dot(c) / c.length_squared();
        start[k] = p0 + d0 * t;
    }
    let mut edges = Vec::with_capacity(m);
    let mut segments = Vec::new();
    for k in 0..m {
        let (e, a, b) = &order[k];
        let (s0, s1) = (start[k], start[(k + 1) % m]);
        let from = all.iter().position(|x| x.is_same(e));
        if s0.distance(*a) < TOL && s1.distance(*b) < TOL {
            edges.push(e.clone());
            if let (Some(j), true) = (from, straight(e)) {
                segments.push((j, s0, s1));
            }
            continue;
        }
        if !straight(e) {
            return err("Only straight edges next to a changed edge can be changed yet");
        }
        edges.push(k::make_line(s0, s1).map_err(fe)?);
        if let Some(j) = from {
            segments.push((j, s0, s1));
        }
    }
    let wire = k::make_wire(&edges).map_err(fe)?;
    let mut face = k::make_face_on(f, &wire).map_err(fe)?;
    if normal(&face)?.dot(n) < 0.0 {
        face = face.reversed().map_err(fe)?;
    }
    Ok((face, segments))
}

/// The one offset every one of a one-face sheet's `n` edges moves by, if they all do.
fn uniform_offset(offsets: &BTreeMap<(usize, Option<usize>), Move>, n: usize) -> Option<f64> {
    let edges: std::collections::BTreeSet<usize> = offsets.keys().map(|(e, _)| *e).collect();
    if edges.len() != n || offsets.len() != n {
        return None;
    }
    let mut all = offsets.values().map(|m| match m {
        Move::Offset(o) => Some(*o),
        Move::Onto(..) => None,
    });
    let first = all.next()??;
    all.all(|o| o.is_some_and(|o| (o - first).abs() < TOL)).then_some(first)
}

/// A planar face with every edge moved by `o` (std's Fill pattern's border:
/// a face with holes and arcs): its loops offset in its plane, sides kept
/// straight to their corners. Its edges are named anew.
fn offset_face(f: &Shape, o: f64) -> FResult<(Shape, Vec<usize>, Vec<Option<usize>>)> {
    let pl = f.face_plane().map_err(fe)?.ok_or_else(|| fe("Only edges of planar faces can be changed yet"))?;
    let size = |w: &Shape| w.bounding_box(false).map(|b| b.diagonal()).unwrap_or(0.0);
    let mut wires = f.wires();
    wires.sort_by(|a, b| size(b).total_cmp(&size(a)));
    // Each loop on its own, the way that grows the face by o: the outer one out, the holes in.
    let mut loops = Vec::new();
    for (i, w) in wires.iter().enumerate() {
        let grow = (i == 0) == (o > 0.0);
        // A full circle: a circle of the radius it grows to (OCCT's offset of a lone circle inwards crashes).
        if let [e] = w.edges().as_slice()
            && let Ok(k::Curve::Circle { center, axis, x_dir, radius }) = e.edge_info().map(|i| i.curve)
        {
            let r = if grow { radius + o.abs() } else { radius - o.abs() };
            if r <= TOL {
                return err("The face's boundary could not be offset");
            }
            loops.push(k::make_wire_2d(&k::Plane::new(center, axis, x_dir), &[k::Segment2d::Circle { center: glam::DVec2::ZERO, radius: r }]).map_err(fe)?);
            continue;
        }
        let mut moved = None;
        for d in [o.abs(), -o.abs()] {
            let Ok(m) = k::offset_wire_2d(w, d, k::JoinType::Intersection, false) else { continue };
            if (size(&m) > size(w)) == grow {
                moved = Some(m);
                break;
            }
        }
        loops.push(moved.ok_or_else(|| fe("The face's boundary could not be offset"))?);
    }
    let Some((outer, holes)) = loops.split_first() else { return err("The face has no boundary") };
    let mut face = k::make_face(&pl, outer, holes).map_err(fe)?;
    if normal(&face)?.dot(normal(f)?) < 0.0 {
        face = face.reversed().map_err(fe)?;
    }
    let edges = face.edges().len();
    Ok((face, vec![0], vec![None; edges]))
}

/// The sheet with the edges moved, for each of its faces the index of the
/// face it was, and for each of its edges the edge it was (on that edge's
/// line, moved with it).
fn change_edges(shape: &Shape, offsets: &BTreeMap<(usize, Option<usize>), Move>, join: bool) -> FResult<(Shape, Vec<usize>, Vec<Option<usize>>)> {
    let edges = shape.edges();
    let faces = shape.faces();
    // The edges that move on face `fi`'s side (a move for one face leaves the other side of its edge as it is).
    let moved_on = |fi: usize| -> Vec<(Shape, Move)> { offsets.iter().filter(|((_, f), _)| f.is_none_or(|f| f == fi)).filter_map(|((e, _), d)| edges.get(*e).map(|s| (s.clone(), *d))).collect() };
    if let [f] = faces.as_slice()
        && let Some(o) = uniform_offset(offsets, edges.len())
        && (f.wires().len() > 1 || edges.iter().any(|e| e.edge_info().is_ok_and(|i| !matches!(i.curve, k::Curve::Line { .. }))))
    {
        return offset_face(f, o);
    }
    // Faces no edge of which moves stay as they are; a face with curved edges (a dogbone's arcs) is changed apart.
    let touched = |f: &Shape, moved: &[(Shape, Move)]| f.edges().iter().any(|e| moved.iter().any(|(m, _)| m.is_same(e)));
    let curved = |f: &Shape| f.edges().iter().any(|e| e.edge_info().is_ok_and(|i| !matches!(i.curve, k::Curve::Line { .. })));
    let mut kept: Vec<Option<Shape>> = Vec::with_capacity(faces.len());
    let mut segments: Vec<(usize, DVec3, DVec3)> = Vec::new();
    let mut loops = Vec::new();
    let mut loop_faces = Vec::new();
    for (fi, f) in faces.iter().enumerate() {
        let moved = moved_on(fi);
        if !touched(f, &moved) {
            kept.push(Some(f.clone()));
            continue;
        }
        if f.face_plane().map_err(fe)?.is_none() {
            return err("Only edges of planar faces can be changed yet");
        }
        if !touched(f, &moved) {
            kept.push(Some(f.clone()));
            continue;
        }
        if curved(f) {
            let (face, segs) = change_curved_face(f, &moved, &edges)?;
            segments.extend(segs);
            kept.push(Some(face));
            continue;
        }
        kept.push(None);
        let (pts, sides, froms) = face_loop(f, &moved, &edges)?;
        loops.push((normal(f)?, pts, sides, froms));
        loop_faces.push(f.clone());
    }
    // Each old edge's segment where it goes (moved by its offset): `segments`, above.
    // New corners where a moved side meets its neighbours, per face; a side
    // moved off a collinear neighbour (a partial flange's piece of an edge,
    // live request 1599 (a)) steps across to it: two corners.
    let mut corners: Vec<Vec<(DVec3, Vec<DVec3>)>> = Vec::new();
    for (n, pts, sides, froms) in &loops {
        let mut own = Vec::new();
        let m = pts.len();
        let area: DVec3 = (0..m).map(|k| pts[k].cross(pts[(k + 1) % m])).sum();
        let ccw = area.dot(*n) > 0.0;
        let line = |k: usize| {
            let (p, q) = (pts[k], pts[(k + 1) % m]);
            let d = (q - p).normalize();
            let out = if ccw { d.cross(*n) } else { n.cross(d) };
            match sides[k] {
                None => (p, d),
                Some(Move::Offset(o)) => (p + out * o, d),
                // Where the face's plane meets the other: the point of it nearest p, the direction along d.
                Some(Move::Onto(o, m)) => {
                    let dir = n.cross(m);
                    if dir.length_squared() < 1e-20 {
                        return (p, d);
                    }
                    let dir = dir.normalize() * if n.cross(m).dot(d) < 0.0 { -1.0 } else { 1.0 };
                    // Along `out` (in the face's plane, across d) from p to the plane.
                    let den = out.dot(m);
                    let s = if den.abs() < 1e-12 { 0.0 } else { (o - p).dot(m) / den };
                    (p + out * s, dir)
                }
            }
        };
        for k in 0..m {
            if let Some(j) = froms[k] {
                let (p, d) = line(k);
                segments.push((j, p, p + d * pts[k].distance(pts[(k + 1) % m])));
            }
        }
        for k in 0..m {
            let prev = (k + m - 1) % m;
            if sides[prev].is_none() && sides[k].is_none() {
                continue;
            }
            let ((p0, d0), (p1, d1)) = (line(prev), line(k));
            let c = d0.cross(d1);
            if c.length_squared() < 1e-20 {
                if (p1 - p0).cross(d0).length() > TOL {
                    let at = |q: DVec3, d: DVec3| q + d * (pts[k] - q).dot(d);
                    own.push((pts[k], vec![at(p0, d0), at(p1, d1)]));
                }
                continue;
            }
            let t = (p1 - p0).cross(d1).dot(c) / c.length_squared();
            own.push((pts[k], vec![p0 + d0 * t]));
        }
        corners.push(own);
    }
    let mut rebuilt = Vec::new();
    for (((n, pts, _, _), f), corners) in loops.iter().zip(&loop_faces).zip(&corners) {
        let new: Vec<DVec3> = pts.iter().flat_map(|p| corners.iter().find(|(o, _)| o.distance(*p) < TOL).map_or(vec![*p], |x| x.1.clone())).collect();
        if new.len() == pts.len() && new.iter().zip(pts).all(|(a, b)| a.distance(*b) < TOL) {
            rebuilt.push(f.clone());
            continue;
        }
        let wire = k::make_polyline(&new, true).map_err(fe)?;
        let mut face = k::make_face_from_wire(&wire, true).map_err(fe)?;
        if normal(&face)?.dot(*n) < 0.0 {
            face = face.reversed().map_err(fe)?;
        }
        rebuilt.push(face);
    }
    // In the faces' order.
    let mut rebuilt = rebuilt.into_iter();
    let mut built: Vec<Shape> = kept.into_iter().map(|k| k.or_else(|| rebuilt.next())).collect::<Option<Vec<_>>>().ok_or_else(|| fe("A changed face is missing"))?;
    // A new edge is the old edge it lies along (the one whose segment holds
    // its middle, else the nearest of those on its line).
    let edge_from = |result: &Shape| -> Vec<Option<usize>> {
        result
            .edges()
            .iter()
            .map(|e| {
                let (a, b) = e.endpoints().ok()?;
                let on_line = |p: DVec3, q: DVec3, x: DVec3| ((x - p).cross((q - p).normalize())).length() < 1e-6;
                let mid = (a + b) * 0.5;
                let along = |p: DVec3, q: DVec3| {
                    let t = (mid - p).dot(q - p) / (q - p).length_squared().max(1e-300);
                    (t.clamp(0.0, 1.0) - t).abs()
                };
                segments.iter().filter(|(_, p, q)| on_line(*p, *q, a) && on_line(*p, *q, b)).min_by(|x, y| along(x.1, x.2).total_cmp(&along(y.1, y.2))).map(|x| x.0)
            })
            .collect()
    };
    if built.len() == 1 {
        let face = built.remove(0);
        let from = edge_from(&face);
        return Ok((face, vec![0], from));
    }
    let sewn = k::sew(&built, TOL, false).map_err(fe)?.shape;
    let sewn = if join { k::unify_edges(&sewn).map_err(fe)?.shape } else { sewn };
    // Sewing may reorder the faces: match them by their boxes.
    let key = |f: &Shape| f.bounding_box(false).map(|b| (b.center(), b.diagonal())).ok();
    let keys: Vec<_> = built.iter().map(key).collect();
    let order = sewn
        .faces()
        .iter()
        .map(|f| {
            let kf = key(f)?;
            keys.iter().position(|k| k.is_some_and(|k| k.0.distance(kf.0) < 1e-9 && (k.1 - kf.1).abs() < 1e-9))
        })
        .collect::<Option<Vec<usize>>>()
        .ok_or_else(|| fe("The changed sheet's faces could not be matched"))?;
    let from = edge_from(&sewn);
    Ok((sewn, order, from))
}
