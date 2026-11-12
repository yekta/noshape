//! Implicit mate connectors (Onshape's hover inference points).

use super::context::Connector;
use super::eval::EvalCtx;
use crate::reference::{Inference, Reference};
use glam::DVec3;
use noshape_kernel as k;
use noshape_kernel::ShapeType;

pub fn implicit_connector(ev: &EvalCtx, on: &Reference, inference: Inference) -> Option<Connector> {
    let mk = |origin: DVec3, z: DVec3| {
        let z = z.normalize();
        let mut x = DVec3::X - z * z.x;
        if x.length_squared() < 1e-12 {
            x = DVec3::Y - z * z.y;
        }
        Connector { reference: Reference::Implicit { on: Box::new(on.clone()), inference }, name: "Mate connector".into(), origin, x: x.normalize(), z }
    };
    match on {
        Reference::Origin => Some(mk(DVec3::ZERO, DVec3::Z)),
        Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. } => {
            let f = ev.raw_plane(on)?;
            Some(Connector { reference: Reference::Implicit { on: Box::new(on.clone()), inference }, name: "Mate connector".into(), origin: f.origin, x: f.x, z: f.normal })
        }
        Reference::Face(_) => {
            let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(on)).first()?;
            let face = ev.face_shape(bi, fi).ok()?;
            match face.face_surface().ok()? {
                k::Surface::Cylinder { origin, axis, .. } | k::Surface::Cone { origin, axis, .. } => {
                    let bb = face.bounding_box(true).ok()?;
                    let c = bb.center();
                    let t = (c - origin).dot(axis);
                    let half = (bb.size().dot(axis.abs())) * 0.5;
                    let along = match inference {
                        Inference::TopAxis => t + half,
                        Inference::BottomAxis => t - half,
                        _ => t,
                    };
                    Some(mk(origin + axis * along, axis))
                }
                _ => {
                    let c = face.centroid().ok()?;
                    let n = face.normal_at_point(c).ok()?;
                    Some(mk(c, n))
                }
            }
        }
        Reference::Edge(_) => {
            let (bi, ei) = *ev.resolve_edges(std::slice::from_ref(on)).first()?;
            let e = ev.edge_shape(bi, ei).ok()?;
            let info = e.edge_info().ok()?;
            match info.curve {
                // Arcs and circles: the center, except an arc's midpoint
                // (Onshape offers center, midpoint and ends on an arc).
                k::Curve::Circle { center, axis, .. } if !matches!(inference, Inference::Midpoint | Inference::Vertex) => {
                    let outward = match inference {
                        Inference::CenterRim => rim_z(ev, bi, ei, info.start),
                        Inference::CenterOutward => rim_normal(ev, bi, ei, info.start, axis.normalize()),
                        _ => None,
                    };
                    Some(mk(center, outward.unwrap_or(axis)))
                }
                _ => {
                    let mid = e.point_at((info.first_param + info.last_param) * 0.5).ok()?;
                    let p = if matches!(inference, Inference::Vertex) { info.start } else { mid };
                    let t = e.tangent_at((info.first_param + info.last_param) * 0.5).ok().unwrap_or(DVec3::X);
                    Some(mk(p, t))
                }
            }
        }
        // A sketch curve's midpoint (picked outside the sketch, like an edge's).
        Reference::SketchEntity { sketch, entity } if ev.ctx.sketch_holding(*sketch, *entity).is_some_and(|s| s.sketch.curve(noshape_sketch::EntityId(*entity)).is_some()) => {
            let s = ev.ctx.sketch_holding(*sketch, *entity)?;
            let c = s.sketch.curve(noshape_sketch::EntityId(*entity))?;
            let seg = super::features::extrude::curve_to_segment(&c)?;
            let e = k::make_edge_2d(&super::eval::kernel_plane(&s.frame), &seg).ok()?;
            let info = e.edge_info().ok()?;
            let u = (info.first_param + info.last_param) * 0.5;
            let mid = e.point_at(u).ok()?;
            let t = e.tangent_at(u).ok().unwrap_or(DVec3::X);
            Some(mk(mid, t))
        }
        Reference::Vertex(_) | Reference::SketchEntity { .. } => {
            let p = ev.raw_vertex_point(on)?;
            Some(mk(p, DVec3::Z))
        }
        // A sketch region: its area centroid, Z along the sketch normal
        // (live assembly/601: hovering inside a sketch instance's loop).
        Reference::SketchRegion { sketch, key } => {
            let (s, region) = ev.resolve_region(*sketch, key)?;
            let profile = super::eval::region_profile(&s, &region, crate::reference::Source::Region { sketch: *sketch, key: key.clone() }).ok()?;
            let c = profile.shape.centroid().ok()?;
            Some(Connector { reference: Reference::Implicit { on: Box::new(on.clone()), inference }, name: "Mate connector".into(), origin: c, x: s.frame.x, z: s.frame.normal })
        }
        _ => {
            let _ = ShapeType::Face;
            None
        }
    }
}

/// v0.4.45's rule (`Inference::CenterOutward`, kept for what it saved): out through the planar face it
/// bounds (that face's outward normal: +z on top rims, −z on bottom rims, pins and holes alike); on a rim between a
/// cone and a cylinder against the cone's outward normal (asm-robotic-arm: an M3 screw's head-to-shank rim points to
/// the head, its countersunk hole's rim to the bore). None for other rims (the circle's own axis then).
fn rim_normal(ev: &EvalCtx, bi: usize, ei: usize, at: DVec3, axis: DVec3) -> Option<DVec3> {
    let shape = &ev.ctx.bodies[bi].shape;
    let faces: Vec<k::Shape> = shape.edge_faces().ok()?.get(ei).cloned().unwrap_or_default().iter().filter_map(|&fi| shape.sub_shape(ShapeType::Face, fi as usize).ok()).collect();
    let along = |f: &k::Shape| f.normal_at_point(at).ok().map(|n| n.dot(axis)).filter(|d| d.abs() > 1e-6);
    if let Some(d) = faces.iter().filter(|f| f.is_planar()).find_map(along) {
        return Some(axis * d.signum());
    }
    let cone = faces.iter().find(|f| matches!(f.face_surface(), Ok(k::Surface::Cone { .. })))?;
    along(cone).map(|d| axis * -d.signum())
}

/// Live 1708 / 2519: a circular edge's CENTER connector points out through the planar face it bounds (that face's
/// outward normal); between two curved faces along the rim circle's own normal, as the feature that made it runs it:
/// a revolve's rims (two side faces of one feature) against its axis, ours along it; others as ours (a chamfer's or a
/// boolean's countersink rims +z on a +z hole).
fn rim_z(ev: &EvalCtx, bi: usize, ei: usize, at: DVec3) -> Option<DVec3> {
    let body = &ev.ctx.bodies[bi];
    let shape = &body.shape;
    let indices = shape.edge_faces().ok()?.get(ei).cloned().unwrap_or_default();
    let faces: Vec<(usize, k::Shape)> = indices.iter().filter_map(|&fi| Some((fi as usize, shape.sub_shape(ShapeType::Face, fi as usize).ok()?))).collect();
    if let Some((_, f)) = faces.iter().find(|(_, f)| f.is_planar()) {
        return f.normal_at_point(at).ok();
    }
    let k::Curve::Circle { axis, .. } = shape.sub_shape(ShapeType::Edge, ei).ok()?.edge_info().ok()?.curve else { return None };
    // (A face's own name through the splits later features made and a custom feature's wrapping of its operations'
    // faces, `Generated`: robotic-arm's screws are a custom feature's opRevolve.)
    let side_of = |fi: usize| {
        use crate::reference::{Role, TopoName};
        let mut n = body.faces.get(fi)?.as_ref();
        loop {
            match n {
                TopoName::Split { parent, .. } => n = parent,
                TopoName::Face { role: Role::Generated(inner), .. } => n = inner,
                TopoName::Face { feature, role: Role::Side(_) } => return Some(*feature),
                _ => return None,
            }
        }
    };
    let swept = faces.len() == 2 && side_of(faces[0].0).is_some() && side_of(faces[0].0) == side_of(faces[1].0);
    Some(if swept { -axis.normalize() } else { axis.normalize() })
}
