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
        Reference::Plane(_) | Reference::FeaturePlane(_) => {
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
                k::Curve::Circle { center, axis, .. } if !matches!(inference, Inference::Midpoint | Inference::Vertex) => Some(mk(center, axis)),
                _ => {
                    let mid = e.point_at((info.first_param + info.last_param) * 0.5).ok()?;
                    let p = if matches!(inference, Inference::Vertex) { info.start } else { mid };
                    let t = e.tangent_at((info.first_param + info.last_param) * 0.5).ok().unwrap_or(DVec3::X);
                    Some(mk(p, t))
                }
            }
        }
        // A sketch curve's midpoint (picked outside the sketch, like an edge's).
        Reference::SketchEntity { sketch, entity } if ev.ctx.sketch(*sketch).is_some_and(|s| s.sketch.curve(noshape_sketch::EntityId(*entity)).is_some()) => {
            let s = ev.ctx.sketch(*sketch)?;
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
