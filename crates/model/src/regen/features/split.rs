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
            let b = ev.ctx.body(id).ok_or_else(|| fe("The split tool no longer exists"))?;
            if b.kind != BodyKind::Surface {
                return err("Split with a surface, face, plane or mate connector");
            }
            let faces = rebrand(ev, b.faces.clone());
            let plane = if b.shape.count(ShapeType::Face) == 1 && !trimmed { b.shape.faces()[0].face_plane()? } else { None };
            if let Some(pl) = plane {
                return Ok(Tool { body: Some(b.id.clone()), ..plane_tool(pl)? });
            }
            Ok(Tool { names: names_for(&b.shape, faces), shape: b.shape.clone(), plane: None, body: Some(b.id.clone()) })
        }
        Reference::Face(_) => {
            let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(r)).first().ok_or_else(|| fe("The split face no longer exists"))?;
            let b = &ev.ctx.bodies[bi];
            let f = b.shape.sub_shape(ShapeType::Face, fi)?;
            if !trimmed && let Some(pl) = f.face_plane()? {
                return plane_tool(pl);
            }
            let faces = rebrand(ev, vec![b.faces[fi].clone()]);
            Ok(Tool { names: names_for(&f, faces), shape: f, plane: None, body: None })
        }
        _ => {
            let frame = ev.resolve_plane(r).ok_or_else(|| fe("The split tool could not be resolved"))?;
            plane_tool(kernel_plane(&frame))
        }
    }
}

/// Centroid of a piece (volume, area or length weighted).
fn piece_center(s: &Shape) -> DVec3 {
    let props = if s.count(ShapeType::Solid) > 0 {
        k::volume_properties(s)
    } else if s.count(ShapeType::Face) > 0 {
        k::surface_properties(s)
    } else {
        k::linear_properties(s)
    };
    props.map(|p| p.center).unwrap_or_else(|_| s.bounding_box(false).map(|b| b.center()).unwrap_or(DVec3::ZERO))
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
fn pieces_of(shape: &Shape, names: &[Name], kind: BodyKind) -> Vec<NamedShape> {
    match kind {
        BodyKind::Solid => crate::regen::eval::split_solids(shape, names),
        BodyKind::Surface => {
            let mut parts: Vec<Shape> = shape.shells();
            let in_shell: Vec<Shape> = parts.iter().flat_map(|s| s.faces()).collect();
            for f in shape.faces() {
                if !in_shell.iter().any(|g| g.is_same(&f)) {
                    parts.push(f);
                }
            }
            parts
                .into_iter()
                .map(|p| {
                    let n = p.faces().iter().map(|f| shape.index_of(f, ShapeType::Face).and_then(|i| names.get(i).cloned()).unwrap_or_else(|| Arc::new(TopoName::face(crate::reference::FeatureId(0), Role::New(0))))).collect();
                    NamedShape::new(p, n)
                })
                .collect()
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
    let proj = k::project_curves(curve, face, dir).map_err(kerr("Projecting the split curves failed"))?;
    Ok(proj.edges())
}

pub fn split_part(ev: &mut EvalCtx) -> FResult<()> {
    if ev.is("splitType", "FACE") {
        return split_faces(ev);
    }
    let target_ids: Vec<BodyId> = ev.resolve_bodies(ev.query("targets")).into_iter().map(|i| ev.ctx.bodies[i].id.clone()).collect();
    if target_ids.is_empty() {
        return crate::regen::eval::err_at(&["targets"], "Select parts, surfaces, or curves to split.");
    }
    let Some(tool_ref) = ev.query("tool").first() else { return crate::regen::eval::err_at(&["tool"], "Select an entity to split with") };
    let tool = resolve_tool(ev, tool_ref, ev.bool("useTrimmed"))?;
    let both = ev.bool("keepBothSides");
    let front = ev.bool("keepFront");
    let mut changed = false;
    for (tord, id) in target_ids.iter().enumerate() {
        if tool.body.as_ref() == Some(id) {
            continue;
        }
        let Some(ti) = index_of_body(ev, id) else { continue };
        let target = ev.ctx.bodies[ti].clone();
        let res = k::split(std::slice::from_ref(&target.shape), std::slice::from_ref(&tool.shape), &k::BooleanOptions::default()).map_err(kerr("Split failed"))?;
        let face_names = if target.kind == BodyKind::Wire { Vec::new() } else { ev.name_result(&[&target.shape_names(), &tool.names], &res.history, &res.shape, &BTreeMap::new(), None) };
        let mut pieces: Vec<(f64, DVec3, NamedShape)> = pieces_of(&res.shape, &face_names, target.kind)
            .into_iter()
            .filter(|p| !p.shape.is_null())
            .map(|p| {
                let c = piece_center(&p.shape);
                (side_of(&tool, c), c, p)
            })
            .collect();
        if pieces.len() < 2 {
            continue;
        }
        changed = true;
        let q = |x: f64| (x * 1e9).round() as i64;
        pieces.sort_by(|a, b| q(b.0).cmp(&q(a.0)).then(q(a.1.x).cmp(&q(b.1.x))).then(q(a.1.y).cmp(&q(b.1.y))).then(q(a.1.z).cmp(&q(b.1.z))));
        if !both {
            pieces.retain(|p| if front { p.0 > 0.0 } else { p.0 < 0.0 });
        }
        let mut bodies: Vec<Body> = Vec::new();
        for (j, (_, _, p)) in pieces.into_iter().enumerate() {
            let (bid, name) = if j == 0 {
                (target.id.clone(), target.name.clone())
            } else {
                (BodyId { feature: ev.feature, index: (tord * 64 + j) as u32 }, ev.ctx.alloc_part_name(target.kind))
            };
            bodies.push(Body::new(bid, name, target.kind, p.shape, p.names.faces, target.color, ev.feature));
        }
        ev.ctx.bodies.remove(ti);
        for (k2, b) in bodies.into_iter().enumerate() {
            ev.ctx.bodies.insert(ti + k2, Arc::new(b));
        }
    }
    if !changed {
        return err("The split tool doesn't split the selected parts");
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
        return err("Select faces to split");
    }
    let tool_refs = ev.query("faceTools");
    if tool_refs.is_empty() {
        return err("Select entities to split with");
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
            Some(r) => Some(ev.resolve_direction(r).ok_or_else(|| fe("The projection direction could not be resolved"))?),
            None if curves.is_empty() => None,
            None => return err("Select a projection direction"),
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
            Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::MateConnector { .. } | Reference::Implicit { .. } => {
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
        let mut imprint: Vec<Shape> = Vec::new();
        for face in &faces {
            for c in &curves {
                imprint.extend(project_onto(&c.shape, face, direction)?);
            }
        }
        for t in &surf_tools {
            if let Ok(sec) = k::section(&target, t) {
                imprint.extend(sec.shape.edges());
            }
        }
        if imprint.is_empty() {
            continue;
        }
        let res = k::imprint(&body.shape, &imprint).map_err(kerr("Split failed"))?;
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
        return err("The tools don't split the selected faces");
    }
    if !ev.bool("keepToolSurfaces") {
        remove_bodies(ev, &tool_bodies);
    }
    Ok(())
}
