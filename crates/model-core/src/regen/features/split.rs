//! Split: parts/surfaces/curves by a plane, face, surface or mate connector
//! (keep both sides or one side, keep tools), and faces by curves, faces or
//! planes (projected along a direction or the target normal).
//!
//! Pieces of a split body are ordered front (positive side of the tool
//! normal) to back; the first kept piece keeps the original part id and
//! name, the others become new parts with ids derived from the feature, the
//! target's position in the selection and the piece order.

use super::helpers_a::*;
use crate::display::BodyKind;
use crate::naming::{Name, ShapeNames};
use crate::reference::{BodyId, Reference, Role, TopoName};
use crate::regen::context::Body;
use crate::regen::eval::{EvalCtx, FResult, NamedShape, err, kernel_plane, names_for};
use glam::DVec3;
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;
use std::sync::Arc;

struct Tool {
    shape: Shape,
    names: ShapeNames,
    plane: Option<k::Plane>,
    /// Sheet body used as the tool (deleted unless Keep tools).
    body: Option<BodyId>,
}

fn resolve_tool(ev: &EvalCtx, r: &Reference, trimmed: bool) -> FResult<Tool> {
    let plane_tool = |pl: k::Plane| -> FResult<Tool> {
        let shape = big_plane_face(ev, &pl)?;
        let names = names_for(&shape, vec![Arc::new(TopoName::face(ev.feature, Role::Datum(0)))]);
        Ok(Tool { shape, names, plane: Some(pl), body: None })
    };
    match r {
        Reference::Body(id) => {
            let b = ev.ctx.body(id).ok_or_else(|| crate::regen::eval::FeatureError::at(&["tool"], "Select entity to split with."))?;
            if b.kind != BodyKind::Surface {
                return err("Incorrect input for tool type.");
            }
            let faces = rebrand(ev, b.faces.clone());
            let plane = if b.shape.count(ShapeType::Face) == 1 && !trimmed { b.shape.faces()[0].face_plane()? } else { None };
            if let Some(pl) = plane {
                return Ok(Tool { body: Some(b.id.clone()), ..plane_tool(pl)? });
            }
            Ok(Tool { names: names_for(&b.shape, faces), shape: b.shape.clone(), plane: None, body: Some(b.id.clone()) })
        }
        Reference::Face(_) => {
            let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(r)).first().ok_or_else(|| crate::regen::eval::FeatureError::at(&["tool"], "Select entity to split with."))?;
            let b = &ev.ctx.bodies[bi];
            let f = b.shape.sub_shape(ShapeType::Face, fi)?;
            if !trimmed && let Some(pl) = f.face_plane()? {
                return plane_tool(pl);
            }
            let faces = rebrand(ev, vec![b.faces[fi].clone()]);
            // Untrimmed, a cylinder splits as its whole surface (c2-rib-wrap's Split 1 by the part's own round lobe:
            // live cuts the disc it closes off out of the part).
            let f = match f.face_surface() {
                Ok(k::Surface::Cylinder { origin, axis, x_dir, radius }) if !trimmed => whole_cylinder(ev, &f, origin, axis, x_dir, radius)?,
                _ => f,
            };
            Ok(Tool { names: names_for(&f, faces), shape: f, plane: None, body: None })
        }
        _ => {
            let frame = ev.resolve_plane(r).ok_or_else(|| crate::regen::eval::FeatureError::at(&["tool"], "Select entity to split with."))?;
            plane_tool(kernel_plane(&frame))
        }
    }
}

/// The whole cylinder of `face`'s surface across the model, facing as `face` does.
fn whole_cylinder(ev: &EvalCtx, face: &Shape, origin: DVec3, axis: DVec3, x_dir: DVec3, radius: f64) -> FResult<Shape> {
    let (axis, ext) = (axis.normalize(), ev.model_extent() * 2.0);
    let c = ev.ctx.bounds().map(|(a, b)| (a + b) * 0.5).unwrap_or(origin);
    let base = origin + axis * ((c - origin).dot(axis) - ext);
    let solid = k::make_cylinder(&k::Plane::new(base, axis, x_dir), radius, 2.0 * ext, std::f64::consts::TAU)?;
    let Some(side) = solid.faces().into_iter().find(|s| matches!(s.face_surface(), Ok(k::Surface::Cylinder { .. }))) else { return Ok(face.clone()) };
    let p = face.centroid()?;
    let same = match (face_normal_near(face, p), face_normal_near(&side, p)) {
        (Some((_, a)), Some((_, b))) => a.dot(b) >= 0.0,
        _ => true,
    };
    Ok(if same { side } else { side.reversed()? })
}

/// Centroid of a piece (volume, area or length weighted).
fn piece_center(s: &Shape) -> DVec3 {
    // (A solid's by the quick estimate: the exact integral took 0.9 s of c2-parting-line-draft-parting-line's Split 3,
    // a piece cut by a free-form surface; only its side and order are wanted.)
    if s.count(ShapeType::Solid) > 0
        && let Ok(c) = k::volume_centre_estimate(s)
    {
        return c;
    }
    let props = if s.count(ShapeType::Solid) > 0 {
        k::volume_properties(s)
    } else if s.count(ShapeType::Face) > 0 {
        k::surface_properties(s)
    } else {
        k::linear_properties(s)
    };
    props.map(|p| p.center).unwrap_or_else(|_| s.bounding_box(false).map(|b| b.center()).unwrap_or(DVec3::ZERO))
}

/// A point of a sheet piece away from the tool, to tell its side by: of the point nearest its centre, its vertices and
/// its edges' middles the one farthest from the tool. (A sheet's centre may lie off it: c2-surf-workshop-split's Split 2
/// cuts a disc out of Surface 1, the holed piece's centre over the hole, inside the tool's tube like the disc's.)
fn sheet_point(piece: &Shape, centre: DVec3, tool: &Shape) -> DVec3 {
    let on = k::make_vertex(centre).ok().and_then(|v| k::distance(&v, piece).ok()).map(|d| d.point_b);
    let ends = piece.vertices().iter().filter_map(|v| v.vertex_point().ok()).collect::<Vec<_>>();
    let mids = piece.edges().iter().filter_map(edge_mid).collect::<Vec<_>>();
    let away = |p: &DVec3| k::make_vertex(*p).ok().and_then(|v| k::distance(&v, tool).ok()).map_or(0.0, |d| d.distance);
    on.into_iter().chain(ends).chain(mids).map(|p| (away(&p), p)).max_by(|a, b| a.0.total_cmp(&b.0)).map_or(centre, |(_, p)| p)
}

/// Signed side of a point relative to the tool (positive = front).
fn side_of(tool: &Tool, p: DVec3) -> f64 {
    if let Some(pl) = &tool.plane {
        return pl.signed_distance(p);
    }
    let mut best: Option<(f64, f64)> = None;
    for f in tool.shape.faces() {
        if let Some((q, n)) = face_normal_near(&f, p) {
            let d = p.distance(q);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, (p - q).dot(n)));
            }
        }
    }
    best.map(|b| b.1).unwrap_or(0.0)
}

/// Connected pieces of a split result with their face names.
/// A shell's faces in pieces joined only across edges off `tool`: OCCT keeps a sheet split by a sheet in one shell, its
/// halves joined along the cut (c2-surf-workshop-manualblend's Split 1: live 2 pieces of each sheet).
fn apart_at(shell: &Shape, tool: &Shape) -> Vec<Shape> {
    let faces = shell.faces();
    let Ok(edge_faces) = shell.edge_faces() else { return vec![shell.clone()] };
    if faces.len() < 2 {
        return vec![shell.clone()];
    }
    let mut root: Vec<usize> = (0..faces.len()).collect();
    fn find(root: &mut [usize], mut i: usize) -> usize {
        while root[i] != i {
            root[i] = root[root[i]];
            i = root[i];
        }
        i
    }
    for (e, fs) in shell.edges().iter().zip(&edge_faces) {
        let [a, b] = fs[..] else { continue };
        let on_tool = edge_mid(e).and_then(|p| k::make_vertex(p).ok()).and_then(|v| k::distance(&v, tool).ok()).is_some_and(|d| d.distance < 1e-6);
        if !on_tool {
            let (a, b) = (find(&mut root, a as usize), find(&mut root, b as usize));
            root[a] = b;
        }
    }
    let mut groups: BTreeMap<usize, Vec<Shape>> = BTreeMap::new();
    for (i, f) in faces.into_iter().enumerate() {
        groups.entry(find(&mut root, i)).or_default().push(f);
    }
    if groups.len() < 2 {
        return vec![shell.clone()];
    }
    groups.values().map(|g| Shape::shell(g)).collect::<Result<Vec<Shape>, _>>().unwrap_or_else(|_| vec![shell.clone()])
}

fn pieces_of(shape: &Shape, names: &[Name], kind: BodyKind, tool: &Shape, by: crate::reference::FeatureId) -> Vec<NamedShape> {
    match kind {
        BodyKind::Solid => crate::regen::eval::split_solids(shape, names, Some(by)),
        BodyKind::Surface => {
            let mut parts: Vec<Shape> = shape.shells().iter().flat_map(|s| apart_at(s, tool)).collect();
            let in_shell: Vec<Shape> = parts.iter().flat_map(|s| s.faces()).collect();
            for f in shape.faces() {
                if !in_shell.iter().any(|g| g.is_same(&f)) {
                    parts.push(f);
                }
            }
            let mut pieces: Vec<NamedShape> = parts
                .into_iter()
                .map(|p| {
                    let n = p.faces().iter().map(|f| shape.index_of(f, ShapeType::Face).and_then(|i| names.get(i).cloned()).unwrap_or_else(|| Arc::new(TopoName::face(crate::reference::FeatureId(0), Role::New(0))))).collect();
                    NamedShape::new(p, n)
                })
                .collect();
            crate::regen::eval::name_pieces_apart(&mut pieces, by);
            pieces
        }
        _ => {
            let edges = shape.edges();
            let wires = k::edges_to_wires(&Shape::compound(&edges), 1e-7).unwrap_or_default();
            wires.into_iter().map(|w| NamedShape::new(w, Vec::new())).collect()
        }
    }
}

/// Project a curve onto a face along `dir` (or the face normal). Curves
/// parallel to a planar face are translated onto it exactly.
fn project_onto(curve: &Shape, face: &Shape, dir: Option<DVec3>) -> FResult<Vec<Shape>> {
    if let Some(pl) = face.face_plane()? {
        let n = pl.unit_normal();
        let d = dir.unwrap_or(n);
        let pts: Vec<DVec3> = curve.vertices().iter().filter_map(|v| v.vertex_point().ok()).chain(edge_mid(curve)).collect();
        let h: Vec<f64> = pts.iter().map(|p| pl.signed_distance(*p)).collect();
        if d.dot(n).abs() > 1e-9 && h.iter().all(|x| (x - h[0]).abs() < 1e-9) {
            let moved = curve.translated(-d * (h[0] / d.dot(n)))?;
            return Ok(moved.edges());
        }
    }
    let proj = k::project_curves(curve, face, dir).map_err(kerr("Tool entity cannot split the selected part/face."))?;
    Ok(proj.edges())
}

/// Split Face's error when the tools change nothing (std's
/// SPLIT_FACE_NO_CHANGE; std Jog's `ownExistingImprints` split expects it).
pub const NO_SPLIT: &str = "Selected entities do not split selected faces.";

/// A body's faces in creation order: by the feature that made each (its place in the list), then in that Extrude's own
/// order. `None` when no face's maker is known.
fn face_creation_order(ev: &EvalCtx, faces: &[Name]) -> Option<Vec<usize>> {
    if let Some(order) = crate::naming::extrude_face_order(faces) {
        return Some(order);
    }
    let made: Vec<Option<usize>> = faces.iter().map(|f| f.creator().and_then(|c| ev.features.order_of(c))).collect();
    if made.iter().all(Option::is_none) {
        return None;
    }
    let mut rank = vec![usize::MAX; faces.len()];
    let mut makers: Vec<usize> = made.iter().flatten().copied().collect();
    makers.sort_unstable();
    makers.dedup();
    for m in makers {
        let mine: Vec<usize> = (0..faces.len()).filter(|&i| made[i] == Some(m)).collect();
        let mut roots: Vec<Name> = Vec::new();
        for &i in &mine {
            let r = Arc::new(faces[i].root().clone());
            if !roots.contains(&r) {
                roots.push(r);
            }
        }
        let within = crate::naming::extrude_face_order(&roots).unwrap_or_else(|| (0..roots.len()).collect());
        for &i in &mine {
            let r = roots.iter().position(|x| **x == *faces[i].root()).unwrap_or(0);
            rank[i] = within.iter().position(|&w| w == r).unwrap_or(r);
        }
    }
    let mut order: Vec<usize> = (0..faces.len()).collect();
    order.sort_by_key(|&i| (made[i].unwrap_or(usize::MAX), rank[i], i));
    Some(order)
}

pub fn split_part(ev: &mut EvalCtx) -> FResult<()> {
    if ev.is("splitType", "FACE") {
        return split_faces(ev);
    }
    let target_ids: Vec<BodyId> = ev.resolve_bodies(ev.query("targets")).into_iter().map(|i| ev.ctx.bodies[i].id.clone()).collect();
    if target_ids.is_empty() {
        return crate::regen::eval::err_at(&["targets"], "Select parts, surfaces, or curves to split.");
    }
    // std splitpart.fs / Onshape's opSplitPart texts.
    let Some(tool_ref) = ev.query("tool").first() else { return crate::regen::eval::err_at(&["tool"], "Select entity to split with.") };
    // A single face, plane or mate connector stays (std reports which).
    if !ev.bool("keepTools") {
        match tool_ref {
            Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. } | Reference::MateConnector { .. } | Reference::Implicit { .. } => ev.infos.push("Planes and mate connectors will be kept.".into()),
            Reference::Face(_) => ev.infos.push("Faces selected as split tools must be kept.".into()),
            _ => {}
        }
    }
    let tool = resolve_tool(ev, tool_ref, ev.bool("useTrimmed"))?;
    // A whole cylinder's pieces: the faces left on one surface are one face, as Parasolid's (c2-rib-wrap's disc: its
    // lobe and the cut are one cylinder, 3 faces).
    let whole_cylinder = !ev.bool("useTrimmed") && matches!(tool_ref, Reference::Face(_)) && matches!(tool.shape.face_surface(), Ok(k::Surface::Cylinder { .. }));
    // A face's plane through its own part: the cut and the face it came from one face, as Parasolid's (c2-body-draft-naming's
    // Split 2 at Extrude 1's cap: live's box 6/12/8). Only faces on the plane merge (the boss's Split 1 imprint stays).
    let face_plane = tool.plane.filter(|_| matches!(tool_ref, Reference::Face(_)));
    let on_plane = |f: &Shape, pl: &k::Plane| f.face_plane().ok().flatten().is_some_and(|q| q.normal.cross(pl.normal).length() < 1e-9 && (q.origin - pl.origin).dot(pl.normal).abs() < 1e-7);
    let both = ev.bool("keepBothSides");
    let front = ev.bool("keepFront");
    let mut changed = false;
    // Every target's kept pieces, named apart across the targets before they become parts (below).
    let mut kept: Vec<(BodyId, BodyKind, usize, NamedShape)> = Vec::new();
    for (tord, id) in target_ids.iter().enumerate() {
        if tool.body.as_ref() == Some(id) {
            continue;
        }
        let Some(ti) = index_of_body(ev, id) else { continue };
        let target = ev.ctx.bodies[ti].clone();
        let res = k::split(std::slice::from_ref(&target.shape), std::slice::from_ref(&tool.shape), &k::BooleanOptions::default()).map_err(kerr("Tool entity cannot split the selected part/face."))?;
        let face_names = if target.kind == BodyKind::Wire { Vec::new() } else { ev.name_result(&[&target.shape_names(), &tool.names], &res.history, &res.shape, &BTreeMap::new(), None) };
        let mut pieces: Vec<(f64, DVec3, NamedShape)> = pieces_of(&res.shape, &face_names, target.kind, &tool.shape, ev.feature)
            .into_iter()
            .filter(|p| !p.shape.is_null())
            .map(|p| {
                if let Some(pl) = &face_plane {
                    let off = |s: &Shape| s.faces().iter().filter(|f| !on_plane(f, pl)).count();
                    if p.shape.faces().iter().filter(|f| on_plane(f, pl)).count() < 2 {
                        return Ok(p);
                    }
                    let un = k::unify_same_domain(&p.shape)?;
                    if un.shape.count(ShapeType::Face) >= p.shape.count(ShapeType::Face) || off(&un.shape) != off(&p.shape) {
                        return Ok(p);
                    }
                    let names = ev.name_result(&[&p.names], &un.history, &un.shape, &BTreeMap::new(), None);
                    return Ok(NamedShape { names: names_for(&un.shape, names), shape: un.shape });
                }
                if !whole_cylinder {
                    return Ok(p);
                }
                let un = k::unify_same_domain(&p.shape)?;
                let names = ev.name_result(&[&p.names], &un.history, &un.shape, &BTreeMap::new(), None);
                Ok(NamedShape { names: names_for(&un.shape, names), shape: un.shape })
            })
            .collect::<FResult<Vec<NamedShape>>>()?
            .into_iter()
            .map(|p| {
                let c = piece_center(&p.shape);
                let side = if target.kind == BodyKind::Surface { side_of(&tool, sheet_point(&p.shape, c, &tool.shape)) } else { side_of(&tool, c) };
                (side, c, p)
            })
            .collect();
        if pieces.len() < 2 {
            // One side kept: a part the tool doesn't cut stays or goes by the side it lies on (live ui-parity
            // split-new-edit-reedit: Part 1 wholly behind Part 2's top face is gone once Keep both sides is off).
            if !both {
                changed = true;
                let side = side_of(&tool, piece_center(&target.shape));
                if (front && side < 0.0) || (!front && side > 0.0) {
                    remove_bodies(ev, std::slice::from_ref(id));
                }
            }
            continue;
        }
        changed = true;
        let q = |x: f64| (x * 1e9).round() as i64;
        pieces.sort_by(|a, b| q(b.0).cmp(&q(a.0)).then(q(a.1.x).cmp(&q(b.1.x))).then(q(a.1.y).cmp(&q(b.1.y))).then(q(a.1.z).cmp(&q(b.1.z))));
        // Live split-order-1123: the piece keeping the target's first face (in its creation order) that not every
        // piece keeps comes first, whichever way the tool faces and however big the pieces. A body several features
        // made: the earlier feature's faces first (c2-body-draft-naming's Split 2: Extrude 1's box is Part 1, the boss
        // Part 2).
        if let Some(order) = face_creation_order(ev, &target.faces) {
            let holds = |p: &NamedShape, i: usize| p.names.faces.iter().any(|f| f.descends_from(&target.faces[i]));
            let keys: Vec<usize> = pieces.iter().map(|p| order.iter().position(|&i| holds(&p.2, i) && !pieces.iter().all(|o| holds(&o.2, i))).unwrap_or(usize::MAX)).collect();
            let mut keyed: Vec<_> = keys.into_iter().zip(pieces).collect();
            keyed.sort_by_key(|k| k.0);
            pieces = keyed.into_iter().map(|k| k.1).collect();
        }
        if !both {
            pieces.retain(|p| if front { p.0 > 0.0 } else { p.0 < 0.0 });
        }
        // Live ("qa5 booleans"): every piece is a new part (new id) in the
        // source's colour, at the end of the parts list.
        ev.ctx.bodies.remove(ti);
        for (j, (_, _, p)) in pieces.into_iter().enumerate() {
            let bid = BodyId { feature: ev.feature, index: (tord * 64 + j) as u32 };
            ev.ctx.part_source.insert(bid.clone(), (id.clone(), true));
            kept.push((bid, target.kind, target.color, p));
        }
    }
    // One tool through several bodies names each body's cut face alike (it comes from the tool): apart per part, as
    // Parasolid's, so a face picked on one ring is that ring's only (user P0 11-04, the Gyro: a Chamfer of one ring's
    // cut face chamfered all three). A reference made before still finds every one (`descends_from`), as it did.
    if kept.len() > 1 {
        let mut named: Vec<NamedShape> = kept.iter().map(|k| k.3.clone()).collect();
        crate::regen::eval::name_pieces_apart(&mut named, ev.feature);
        for (k, n) in kept.iter_mut().zip(named) {
            k.3 = n;
        }
    }
    for (bid, kind, color, p) in kept {
        let name = ev.ctx.alloc_part_name(kind);
        ev.ctx.bodies.push(Arc::new(Body::new(bid, name, kind, p.shape, p.names.faces, color, ev.feature)));
    }
    if !changed {
        // Live "fs (10-25 06:15)": an info (the feature's only one), the
        // part and the tool unchanged.
        ev.infos.clear();
        ev.infos.push("Selected entity does not split selected parts.".into());
        return Ok(());
    }
    if !ev.bool("keepTools")
        && let Some(id) = &tool.body
    {
        remove_bodies(ev, std::slice::from_ref(id));
    }
    Ok(())
}

fn split_faces(ev: &mut EvalCtx) -> FResult<()> {
    let targets = by_body(ev.resolve_faces(ev.query("faceTargets")));
    if targets.is_empty() {
        return crate::regen::eval::err_at(&["faceTargets"], "Select faces to split.");
    }
    let tool_refs = ev.query("faceTools");
    if tool_refs.is_empty() {
        return crate::regen::eval::err_at(&["faceTools"], "Select entities to split with.");
    }
    let curves = resolve_curves(ev, tool_refs)?;
    let direction = if ev.is("projectionType", "NORMAL_TO_TARGET") {
        None
    } else if ev.bool("useSketchPlaneDirection") {
        tool_refs
            .iter()
            .find_map(|r| match r {
                Reference::SketchEntity { sketch, .. } | Reference::Sketch(sketch) => ev.ctx.sketch(*sketch).map(|s| s.frame.normal),
                _ => None,
            })
            .or_else(|| (!curves.is_empty()).then_some(DVec3::Z))
    } else {
        match ev.query("directionQuery").first() {
            Some(r) => Some(ev.resolve_direction(r).ok_or_else(|| crate::regen::eval::FeatureError::at(&["directionQuery"], "Select edge projection direction for split."))?),
            None if curves.is_empty() => None,
            None => return crate::regen::eval::err_at(&["directionQuery"], "Select edge projection direction for split."),
        }
    };
    // Surface-like tools (faces, planes, sheet bodies).
    let mut surf_tools: Vec<Shape> = Vec::new();
    let mut tool_bodies: Vec<BodyId> = Vec::new();
    for r in tool_refs {
        match r {
            Reference::Face(_) => {
                for (bi, fi) in ev.resolve_faces(std::slice::from_ref(r)) {
                    let face = ev.face_shape(bi, fi)?;
                    // FeatureScript opSplitFace "extendToCompletion": planar tools split as whole planes.
                    match face.face_plane() {
                        Ok(Some(pl)) if ev.bool("extendToCompletion") => surf_tools.push(big_plane_face(ev, &pl)?),
                        _ => surf_tools.push(face),
                    }
                }
            }
            Reference::Body(id) => {
                if let Some(b) = ev.ctx.body(id) {
                    if b.kind == BodyKind::Surface {
                        surf_tools.push(b.shape.clone());
                    }
                    if matches!(b.kind, BodyKind::Surface | BodyKind::Wire) {
                        tool_bodies.push(b.id.clone());
                    }
                }
            }
            Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. } | Reference::MateConnector { .. } | Reference::Implicit { .. } => {
                if let Some(f) = ev.resolve_plane(r) {
                    surf_tools.push(big_plane_face(ev, &kernel_plane(&f))?);
                }
            }
            _ => {}
        }
    }
    let target_ids: Vec<(BodyId, Vec<usize>)> = targets.into_iter().map(|(bi, fs)| (ev.ctx.bodies[bi].id.clone(), fs)).collect();
    let mut changed = false;
    for (id, fs) in target_ids {
        let Some(bi) = index_of_body(ev, &id) else { continue };
        let body = ev.ctx.bodies[bi].clone();
        let faces: Vec<Shape> = fs.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
        let target = Shape::compound(&faces);
        // A curve that misses a face projects onto the others (c2-hunting-knife's Split 3: the edge's arc lies over
        // one of the two spine faces); the error only when none projects anywhere.
        let mut imprint: Vec<Shape> = Vec::new();
        let mut missed = None;
        for face in &faces {
            for c in &curves {
                match project_onto(&c.shape, face, direction) {
                    Ok(edges) => imprint.extend(edges),
                    Err(e) => missed = Some(e),
                }
            }
        }
        if let Some(e) = missed.filter(|_| imprint.is_empty() && surf_tools.is_empty()) {
            return Err(e);
        }
        for t in &surf_tools {
            if let Ok(sec) = k::section(&target, t) {
                imprint.extend(sec.shape.edges());
            }
        }
        if imprint.is_empty() {
            continue;
        }
        let res = k::imprint_kept(&body.shape, &imprint).map_err(kerr("Tool entity cannot split the selected part/face."))?;
        let res_solid = if body.kind == BodyKind::Solid { res.shape.solids().into_iter().next().unwrap_or(res.shape.clone()) } else { res.shape.clone() };
        if res_solid.count(ShapeType::Face) <= body.shape.count(ShapeType::Face) {
            continue;
        }
        let empty = ShapeNames::default();
        let names = ev.name_result(&[&body.shape_names(), &empty], &res.history, &res.shape, &BTreeMap::new(), None);
        // Keep only the target body's pieces (drop imprinted tool remnants).
        let names: Vec<Name> = res_solid.faces().iter().map(|f| res.shape.index_of(f, ShapeType::Face).and_then(|i| names.get(i).cloned()).unwrap_or_else(|| Arc::new(TopoName::face(ev.feature, Role::New(0))))).collect();
        let names = dedup_names(ev, names);
        ev.replace_body(bi, res_solid, names);
        changed = true;
    }
    if !changed {
        return err(NO_SPLIT);
    }
    if !ev.bool("keepToolSurfaces") {
        remove_bodies(ev, &tool_bodies);
    }
    Ok(())
}
