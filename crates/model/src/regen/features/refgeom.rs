//! Reference geometry: Mate connector, Point, Composite part, Query variable,
//! Name entity, and the Tangent-to-cylinder plane construction.

use super::bshared::{add_wire_body, canonical_x, extent_along};
use crate::display::{BodyKind, PlaneFrame};
use crate::reference::{BodyId, Reference};
use crate::regen::context::{Body, Connector, PointEntity};
use crate::regen::eval::{EvalCtx, FResult, FeatureError, err};
use glam::{DQuat, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::sync::Arc;

/// A local coordinate system (origin, X, Z; Y = Z x X).
#[derive(Clone, Copy, Debug)]
struct Csys {
    origin: DVec3,
    x: DVec3,
    z: DVec3,
}

impl Csys {
    fn new(origin: DVec3, z: DVec3, x_hint: Option<DVec3>) -> Csys {
        let z = z.normalize();
        let x = x_hint.map(|h| h - z * z.dot(h)).filter(|h| h.length_squared() > 1e-12).map(|h| h.normalize()).unwrap_or_else(|| canonical_x(z));
        Csys { origin, x, z }
    }
    fn y(&self) -> DVec3 {
        self.z.cross(self.x)
    }
    fn rotated(self, axis: DVec3, angle: f64) -> Csys {
        let q = DQuat::from_axis_angle(axis.normalize(), angle);
        Csys { origin: self.origin, x: (q * self.x).normalize(), z: (q * self.z).normalize() }
    }
}

fn not_found(what: &str) -> FeatureError {
    FeatureError::new(format!("Selected {what} could not be resolved"))
}

/// Point where a vertex reference lies and the normal of one adjacent face.
fn vertex_csys(ev: &EvalCtx, r: &Reference) -> Option<Csys> {
    let p = ev.resolve_vertex_point(r)?;
    if let Reference::Vertex(n) = r {
        for b in &ev.ctx.bodies {
            if let Some(&vi) = b.find_vertices(n).first() {
                let vf = b.shape.vertex_faces().ok()?;
                if let Some(&fi) = vf.get(vi).and_then(|f| f.first())
                    && let Ok(face) = b.shape.sub_shape(ShapeType::Face, fi as usize)
                    && let Ok(nrm) = face.normal_at_point(p)
                {
                    return Some(Csys::new(p, nrm, None));
                }
            }
        }
    }
    if let Reference::SketchEntity { sketch, .. } = r
        && let Some(s) = ev.ctx.sketch(*sketch)
    {
        return Some(Csys::new(p, s.frame.normal, Some(s.frame.x)));
    }
    Some(Csys::new(p, DVec3::Z, None))
}

fn edge_csys(e: &Shape, inference: &str) -> FResult<Csys> {
    let info = e.edge_info()?;
    let mid_t = (info.first_param + info.last_param) * 0.5;
    let mid = e.point_at(mid_t)?;
    match info.curve {
        k::Curve::Circle { center, axis, x_dir, .. } => match inference {
            "MID_POINT" => Ok(Csys::new(mid, axis, Some(x_dir))),
            "POINT" => Ok(Csys::new(info.start, axis, Some(x_dir))),
            _ => Ok(Csys::new(center, axis, Some(x_dir))),
        },
        k::Curve::Ellipse { center, axis, x_dir, .. } if !matches!(inference, "MID_POINT" | "POINT") => Ok(Csys::new(center, axis, Some(x_dir))),
        _ => {
            let (p, t) = if inference == "POINT" { (info.start, e.tangent_at(info.first_param)?) } else { (mid, e.tangent_at(mid_t)?) };
            let t = if t.length_squared() > 1e-18 { t } else { (info.end - info.start).normalize_or(DVec3::Z) };
            Ok(Csys::new(p, t, None))
        }
    }
}

fn face_csys(ev: &EvalCtx, face: &Shape, inference: &str, secondary: &[Reference]) -> FResult<Csys> {
    // Inference refined by a secondary entity (vertex / edge on the face).
    let sec_point = secondary.iter().find_map(|r| match r {
        Reference::Vertex(_) | Reference::SketchEntity { .. } => ev.resolve_vertex_point(r),
        _ => None,
    });
    let sec_edge = secondary.iter().find_map(|r| ev.resolve_edges(std::slice::from_ref(r)).first().and_then(|&(bi, ei)| ev.edge_shape(bi, ei).ok()));
    match face.face_surface()? {
        k::Surface::Cylinder { origin, axis, x_dir, .. } | k::Surface::Cone { origin, axis, x_dir, .. }
            if matches!(inference, "TOP_AXIS_POINT" | "MID_AXIS_POINT" | "BOTTOM_AXIS_POINT" | "CENTROID" | "CENTER" | "") =>
        {
            let (lo, hi) = extent_along(face, origin, axis)?;
            let t = match inference {
                "TOP_AXIS_POINT" => hi,
                "BOTTOM_AXIS_POINT" => lo,
                _ => (lo + hi) * 0.5,
            };
            Ok(Csys::new(origin + axis * t, axis, Some(x_dir)))
        }
        k::Surface::Cone { axis, x_dir, apex, .. } if inference == "VIRTUAL_SHARP" => Ok(Csys::new(apex, axis, Some(x_dir))),
        surf => {
            let plane = if let k::Surface::Plane(p) = surf { Some(p) } else { None };
            let point = match inference {
                "POINT" if sec_point.is_some() => sec_point.unwrap_or_default(),
                "MID_POINT" | "CENTER" | "LOOP_CENTER" if sec_edge.is_some() => {
                    let e = sec_edge.clone().unwrap_or_default();
                    let c = edge_csys(&e, if inference == "MID_POINT" { "MID_POINT" } else { "CENTER" })?;
                    if inference == "LOOP_CENTER" && !matches!(e.edge_curve()?, k::Curve::Circle { .. }) {
                        loop_center(face, &e).unwrap_or(c.origin)
                    } else {
                        c.origin
                    }
                }
                _ => face.centroid()?,
            };
            match plane {
                Some(p) => {
                    let n = p.unit_normal();
                    let n = face.face_plane()?.map(|q| q.unit_normal()).unwrap_or(n);
                    let on = point - n * (point - p.origin).dot(n);
                    Ok(Csys::new(on, n, None))
                }
                None => {
                    let pr = face.project_point(point)?;
                    let n = face.normal_at_uv(pr.u, pr.v)?;
                    Ok(Csys::new(pr.point, n, None))
                }
            }
        }
    }
}

/// Centroid of the inner loop (wire of `face`) containing `edge`.
fn loop_center(face: &Shape, edge: &Shape) -> Option<DVec3> {
    for w in face.wires() {
        if w.edges().iter().any(|e| e.is_same(edge)) {
            let f = k::make_face_from_wire(&w, true).ok()?;
            return f.centroid().ok();
        }
    }
    None
}

fn origin_csys(ev: &EvalCtx, r: &Reference, inference: &str, secondary: &[Reference]) -> FResult<Csys> {
    let one = std::slice::from_ref(r);
    match r {
        Reference::Origin => Ok(match inference {
            "ORIGIN_X" => Csys::new(DVec3::ZERO, DVec3::X, Some(DVec3::Y)),
            "ORIGIN_Y" => Csys::new(DVec3::ZERO, DVec3::Y, Some(DVec3::Z)),
            _ => Csys::new(DVec3::ZERO, DVec3::Z, Some(DVec3::X)),
        }),
        Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::Sketch(_) => {
            let f = ev.resolve_plane(r).ok_or_else(|| not_found("plane"))?;
            Ok(Csys { origin: f.origin, x: f.x, z: f.normal })
        }
        Reference::MateConnector { .. } | Reference::Implicit { .. } => {
            let c = ev.resolve_connector(r).ok_or_else(|| not_found("mate connector"))?;
            Ok(Csys { origin: c.origin, x: c.x, z: c.z })
        }
        Reference::Vertex(_) => vertex_csys(ev, r).ok_or_else(|| not_found("vertex")),
        Reference::SketchEntity { sketch, entity } => {
            let s = ev.ctx.sketch(*sketch).ok_or_else(|| not_found("sketch entity"))?;
            let id = noshape_sketch::EntityId(*entity);
            match s.sketch.curve(id) {
                None => vertex_csys(ev, r).ok_or_else(|| not_found("sketch point")),
                Some(curve) => {
                    let f = s.frame;
                    use noshape_sketch::Curve2 as C;
                    match curve {
                        C::Line { a, b } => {
                            let (wa, wb) = (f.to_world(a), f.to_world(b));
                            let p = if inference == "POINT" { wa } else { (wa + wb) * 0.5 };
                            Ok(Csys::new(p, (wb - wa).normalize_or(f.normal), None))
                        }
                        C::Circle { center, .. } | C::Arc { center, .. } | C::Ellipse { center, .. } | C::EllipticalArc { center, .. } => {
                            Ok(Csys::new(f.to_world(center), f.normal, Some(f.x)))
                        }
                        C::Nurbs(_) => {
                            let e = super::bshared::sketch_edge(ev, *sketch, *entity)?.ok_or_else(|| not_found("sketch curve"))?;
                            edge_csys(&e, inference)
                        }
                    }
                }
            }
        }
        Reference::Edge(_) => {
            let &(bi, ei) = ev.resolve_edges(one).first().ok_or_else(|| not_found("edge"))?;
            let e = ev.edge_shape(bi, ei)?;
            if let Some(p) = secondary.iter().find_map(|s| ev.resolve_vertex_point(s)) {
                let info = e.edge_info()?;
                let at_start = (p - info.start).length() <= (p - info.end).length();
                let t = e.tangent_at(if at_start { info.first_param } else { info.last_param })?;
                return Ok(Csys::new(p, t.normalize_or(DVec3::Z), None));
            }
            edge_csys(&e, inference)
        }
        Reference::Face(_) => {
            let &(bi, fi) = ev.resolve_faces(one).first().ok_or_else(|| not_found("face"))?;
            let face = ev.face_shape(bi, fi)?;
            face_csys(ev, &face, inference, secondary)
        }
        Reference::Body(_) => {
            // Part origin.
            if ev.resolve_bodies(one).is_empty() {
                return Err(not_found("part"));
            }
            Ok(Csys::new(DVec3::ZERO, DVec3::Z, Some(DVec3::X)))
        }
        _ => err("Select a vertex, edge, face or mate connector for the origin"),
    }
}

pub fn mate_connector(ev: &mut EvalCtx) -> FResult<()> {
    let origin_ref = ev.query("originQuery").first().cloned().unwrap_or(Reference::Origin);
    let inference = ev.enum_("entityInferenceType").to_string();
    let secondary = ev.query("secondaryOriginQuery");
    let mut cs = origin_csys(ev, &origin_ref, &inference, secondary)?;

    if ev.is("originType", "BETWEEN_ENTITIES") {
        let Some(r) = ev.query("originAdditionalQuery").first() else { return err("Select the second entity to place the origin between") };
        let target = match ev.resolve_faces(std::slice::from_ref(r)).first() {
            Some(&(bi, fi)) => ev.face_shape(bi, fi)?.project_point(cs.origin)?.point,
            None => match ev.resolve_plane(r) {
                Some(f) => cs.origin - f.normal * (cs.origin - f.origin).dot(f.normal),
                None => return Err(not_found("between entity")),
            },
        };
        cs.origin = (cs.origin + target) * 0.5;
    }

    if ev.bool("realign") {
        if let Some(r) = ev.query("primaryAxisQuery").first() {
            let d = ev.resolve_direction(r).ok_or_else(|| not_found("primary axis entity"))?;
            cs = Csys::new(cs.origin, d, Some(cs.x));
        }
        if let Some(r) = ev.query("secondaryAxisQuery").first() {
            let d = ev.resolve_direction(r).ok_or_else(|| not_found("secondary axis entity"))?;
            let x = d - cs.z * cs.z.dot(d);
            if x.length_squared() < 1e-12 {
                return err("Secondary axis can't be parallel to the primary axis");
            }
            cs.x = x.normalize();
        }
    }

    if ev.bool("transform") {
        let (tx, ty, tz) = (ev.length("translationX")?, ev.length("translationY")?, ev.length("translationZ")?);
        cs.origin += cs.x * tx + cs.y() * ty + cs.z * tz;
        let angle = ev.angle("rotation")?;
        if angle != 0.0 {
            let axis = match ev.enum_("rotationType") {
                "ABOUT_X" => cs.x,
                "ABOUT_Y" => cs.y(),
                _ => cs.z,
            };
            cs = cs.rotated(axis, angle);
        }
    }

    if ev.bool("flipPrimary") {
        cs = cs.rotated(cs.x, std::f64::consts::PI);
    }
    let quarter = match ev.enum_("secondaryAxisType") {
        "PLUS_Y" => 1.0,
        "MINUS_X" => 2.0,
        "MINUS_Y" => 3.0,
        _ => 0.0,
    };
    if quarter != 0.0 {
        cs = cs.rotated(cs.z, quarter * std::f64::consts::FRAC_PI_2);
    }

    if ev.bool("allowOwnerEntity") && ev.bool("requireOwnerPart") {
        let owner = ev.query("ownerPart");
        if !owner.is_empty() && ev.resolve_bodies(owner).is_empty() {
            return err("Owner part could not be resolved");
        }
    }

    let name = ev.name.to_string();
    let reference = Reference::MateConnector { feature: ev.feature, index: 0 };
    ev.ctx.connectors.push(Connector { reference, name, origin: cs.origin, x: cs.x, z: cs.z });
    Ok(())
}

/// Construction point on an edge (normalized parameter) or at coordinates.
pub fn point(ev: &mut EvalCtx) -> FResult<()> {
    let p = if ev.is("pointType", "MESH_POINT") {
        DVec3::new(ev.length("x")?, ev.length("y")?, ev.length("z")?)
    } else {
        let Some(&(bi, ei)) = ev.resolve_edges(ev.query("entities")).first() else { return err("Select an edge") };
        let e = ev.edge_shape(bi, ei)?;
        let info = e.edge_info()?;
        let t = ev.real("parameter")?.clamp(0.0, 1.0);
        let t = if matches!(e.orientation(), k::Orientation::Reversed) { 1.0 - t } else { t };
        e.point_at(info.first_param + (info.last_param - info.first_param) * t)?
    };
    let v = k::make_vertex(p)?;
    add_wire_body(ev, v, 0, BodyKind::Point);
    ev.ctx.points.push(PointEntity { reference: Reference::Body(BodyId { feature: ev.feature, index: 0 }), position: p });
    Ok(())
}

/// Composite part: groups the selected bodies into one compound body that
/// keeps every constituent's face, edge and vertex names.
pub fn composite_part(ev: &mut EvalCtx) -> FResult<()> {
    let idx = ev.resolve_bodies(ev.query("bodies"));
    if idx.is_empty() {
        return err("Select parts, surfaces or curves to group");
    }
    let members: Vec<Arc<Body>> = idx.iter().map(|&i| ev.ctx.bodies[i].clone()).collect();
    let compound = Shape::compound(&members.iter().map(|b| b.shape.clone()).collect::<Vec<_>>());
    let nf = compound.count(ShapeType::Face);
    let ne = compound.count(ShapeType::Edge);
    let nv = compound.count(ShapeType::Vertex);
    let fallback = || Arc::new(crate::reference::TopoName::face(ev.feature, crate::reference::Role::New(0)));
    let mut faces = vec![fallback(); nf];
    let mut edges = vec![fallback(); ne];
    let mut vertices = vec![fallback(); nv];
    for m in &members {
        for (i, f) in m.shape.faces().iter().enumerate() {
            if let (Some(j), Some(n)) = (compound.index_of(f, ShapeType::Face), m.faces.get(i)) {
                faces[j] = n.clone();
            }
        }
        for (i, e) in m.shape.edges().iter().enumerate() {
            if let (Some(j), Some(n)) = (compound.index_of(e, ShapeType::Edge), m.edges.get(i)) {
                edges[j] = n.clone();
            }
        }
        for (i, v) in m.shape.vertices().iter().enumerate() {
            if let (Some(j), Some(n)) = (compound.index_of(v, ShapeType::Vertex), m.vertices.get(i)) {
                vertices[j] = n.clone();
            }
        }
    }
    let kind = if members.iter().any(|b| b.kind == BodyKind::Solid) {
        BodyKind::Solid
    } else if members.iter().any(|b| b.kind == BodyKind::Surface) {
        BodyKind::Surface
    } else {
        members[0].kind
    };
    let id = BodyId { feature: ev.feature, index: 0 };
    let name = format!("Composite part {}", ev.ctx.bodies.iter().filter(|b| b.name.starts_with("Composite part")).count() + 1);
    let mut body = Body::new(id, name, kind, compound, faces, members[0].color, ev.feature);
    body.edges = edges;
    body.vertices = vertices;
    let ids: Vec<BodyId> = members.iter().map(|b| b.id.clone()).collect();
    let pos = idx[0];
    ev.ctx.bodies.retain(|b| !ids.contains(&b.id));
    ev.ctx.bodies.insert(pos.min(ev.ctx.bodies.len()), Arc::new(body));
    Ok(())
}

/// Query variable: validates the name and that a "Selection" query resolves.
// TODO(context): store the resolved selection in the context once it has a
// query-variable table, so other features can reference it.
pub fn query_variable(ev: &mut EvalCtx) -> FResult<()> {
    let name = ev.params.text("name").trim().trim_start_matches('#').to_string();
    if name.is_empty() || !name.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return err("Enter a valid variable name");
    }
    if ev.is("selectionType", "SELECTION") {
        let refs = ev.query("selectionQuery");
        let unresolved = refs.iter().filter(|r| !resolves(ev, r)).count();
        if unresolved > 0 {
            ev.warnings.push(format!("{unresolved} selected entities could not be resolved"));
        }
    }
    Ok(())
}

fn resolves(ev: &EvalCtx, r: &Reference) -> bool {
    let one = std::slice::from_ref(r);
    match r {
        Reference::Face(_) => !ev.resolve_faces(one).is_empty(),
        Reference::Edge(_) => !ev.resolve_edges(one).is_empty(),
        Reference::Body(_) => !ev.resolve_bodies(one).is_empty(),
        Reference::Vertex(_) | Reference::SketchEntity { .. } => ev.resolve_vertex_point(r).is_some() || ev.resolve_axis(r).is_some(),
        Reference::Plane(_) | Reference::FeaturePlane(_) => ev.resolve_plane(r).is_some(),
        Reference::MateConnector { .. } | Reference::Implicit { .. } => ev.resolve_connector(r).is_some(),
        _ => true,
    }
}

/// Name entity: names are persistent already; nothing to compute.
pub fn name_entity(_ev: &mut EvalCtx) -> FResult<()> {
    Ok(())
}

/// Plane tangent to a cylindrical face, parallel to a plane or through a
/// point (`cPlane` type TANGENT_CYLINDER). `other_side` picks the second
/// tangent solution.
pub fn tangent_cylinder_frame(ev: &EvalCtx, refs: &[Reference], other_side: bool) -> FResult<PlaneFrame> {
    let mut cyl = None;
    let mut rest = Vec::new();
    for r in refs {
        if cyl.is_none()
            && let Some(&(bi, fi)) = ev.resolve_faces(std::slice::from_ref(r)).first()
            && let Ok(face) = ev.face_shape(bi, fi)
            && let Ok(k::Surface::Cylinder { origin, axis, radius, .. }) = face.face_surface()
        {
            let c = face.centroid().unwrap_or(origin);
            // Axis point closest to the face centroid.
            let a = axis.normalize();
            let foot = origin + a * (c - origin).dot(a);
            cyl = Some((foot, a, radius));
            continue;
        }
        rest.push(r);
    }
    let Some((foot, a, radius)) = cyl else { return err("Select a cylindrical face") };
    let Some(other) = rest.first() else { return err("Select a plane or point to define the tangent plane") };
    let side = if other_side { -1.0 } else { 1.0 };
    let normal = if matches!(other, Reference::Vertex(_) | Reference::SketchEntity { .. } | Reference::Origin)
        && let Some(p) = ev.resolve_vertex_point(other)
    {
        let foot_p = foot + a * (p - foot).dot(a);
        let d = (p - foot_p).length();
        if d < radius - 1e-9 {
            return err("The point is inside the cylinder");
        }
        let u = (p - foot_p) / d;
        let phi = (radius / d).clamp(-1.0, 1.0).acos();
        DQuat::from_axis_angle(a, side * phi) * u
    } else if let Some(f) = ev.resolve_plane(other) {
        let n = f.normal - a * f.normal.dot(a);
        if n.length() < 1e-9 {
            return err("The plane must be parallel to the cylinder axis");
        }
        n.normalize() * side
    } else {
        return err("Select a plane or point to define the tangent plane");
    };
    let origin = foot + normal * radius;
    Ok(PlaneFrame { origin, x: a, y: normal.cross(a), normal })
}
