//! Reference geometry: Mate connector, Point, Composite part, Query variable,
//! Name entity.

use super::bshared::{add_wire_body, canonical_x, extent_along};
use crate::display::BodyKind;
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

/// The world axes at the point a vertex reference lies at.
fn vertex_csys(ev: &EvalCtx, r: &Reference) -> Option<Csys> {
    let p = ev.resolve_vertex_point(r)?;
    // A part's vertex: the world axes at the point (live "ui (10-25 d)",
    // ui-mass/04: a connector on a block's corner is world-aligned, not
    // turned to a face). A sketch vertex too (cf-surface-pattern's Mate
    // connector 1 on a sphere's axis end, sketched on a plane off Front: live's
    // Surface Pattern 3 sets the spheres on the face, up along world Z).
    Some(Csys::new(p, DVec3::Z, Some(DVec3::X)))
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
    // A round face's axis (a rounded corner picked as the CENTER of a Hole's connector on the face beside it, live
    // clamp-hms: the hole on the corner fillet's axis).
    let sec_axis = secondary.iter().find_map(|r| {
        let &(bi, fi) = ev.resolve_faces(std::slice::from_ref(r)).first()?;
        match ev.face_shape(bi, fi).ok()?.face_surface().ok()? {
            k::Surface::Cylinder { origin, axis, .. } | k::Surface::Cone { origin, axis, .. } => Some((origin, axis.normalize())),
            _ => None,
        }
    });
    match face.face_surface()? {
        k::Surface::Cylinder { origin, axis, .. } | k::Surface::Cone { origin, axis, .. }
            if matches!(inference, "TOP_AXIS_POINT" | "MID_AXIS_POINT" | "BOTTOM_AXIS_POINT" | "CENTROID" | "CENTER" | "") =>
        {
            let (lo, hi) = extent_along(face, origin, axis)?;
            let t = match inference {
                "TOP_AXIS_POINT" => hi,
                "BOTTOM_AXIS_POINT" => lo,
                _ => (lo + hi) * 0.5,
            };
            // Z the surface's axis, X world X across it (world Y when the axis is along X), not the surface's x (live
            // "skasm (11-14) through-all hole cylinder frame": config-assembly 08590a's Mate connector 1, x (1,0,0) on a
            // hole whose cylinder has x (0,0,−1)).
            let a = axis.normalize();
            let across = |w: DVec3| w - a * a.dot(w);
            let x = if across(DVec3::X).length() > 1e-9 { across(DVec3::X) } else { across(DVec3::Y) };
            Ok(Csys::new(origin + axis * t, axis, Some(x.normalize())))
        }
        k::Surface::Cone { axis, x_dir, apex, .. } if inference == "VIRTUAL_SHARP" => Ok(Csys::new(apex, axis, Some(x_dir))),
        surf => {
            let plane = if let k::Surface::Plane(p) = surf { Some(p) } else { None };
            let point = match inference {
                "POINT" if sec_point.is_some() => sec_point.unwrap_or_default(),
                // (A face as the secondary resolves to its edges too: its axis first.)
                "CENTER" if sec_axis.is_some() => {
                    let (o, a) = sec_axis.unwrap_or_default();
                    // Where the axis meets the face's plane, else its point nearest the face's centroid.
                    let c = face.centroid()?;
                    match plane.as_ref().map(|p| (p.origin, p.unit_normal())).filter(|(_, n)| a.dot(*n).abs() > 1e-9) {
                        Some((po, n)) => o + a * ((po - o).dot(n) / a.dot(n)),
                        None => o + a * (c - o).dot(a),
                    }
                }
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
        Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. } | Reference::Sketch(_) => {
            let f = ev.resolve_plane(r).ok_or_else(|| not_found("plane"))?;
            Ok(Csys { origin: f.origin, x: f.x, z: f.normal })
        }
        Reference::MateConnector { .. } | Reference::Implicit { .. } => {
            let c = ev.resolve_connector(r).ok_or_else(|| not_found("mate connector"))?;
            Ok(Csys { origin: c.origin, x: c.x, z: c.z })
        }
        Reference::Vertex(_) => vertex_csys(ev, r).ok_or_else(|| not_found("vertex")),
        Reference::SketchEntity { sketch, entity } => {
            let s = ev.ctx.sketch_holding(*sketch, *entity).ok_or_else(|| not_found("sketch entity"))?;
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
            let c = edge_csys(&e, inference)?;
            // Live 1708: a circular edge's CENTER connector points out of the
            // material through the planar face the edge bounds (top rims +z,
            // bottom rims −z, pins and holes alike), its X the sketch's x
            // direction whichever way the circle was drawn (here: world X on
            // the circle's plane, world Y for planes facing X, as the default
            // planes' sketch axes).
            if inference == "CENTER" && matches!(e.edge_info()?.curve, k::Curve::Circle { .. }) {
                let shape = &ev.ctx.bodies[bi].shape;
                let faces = shape.edge_faces()?.get(ei).cloned().unwrap_or_default();
                if let Some(n) = faces.iter().find_map(|&fi| shape.sub_shape(k::ShapeType::Face, fi as usize).ok()?.face_plane().ok()?.map(|p| p.unit_normal())) {
                    let x = DVec3::X - n * n.x;
                    let x = if x.length() > 1e-6 { x } else { DVec3::Y - n * n.y };
                    return Ok(Csys::new(c.origin, n, Some(x.normalize())));
                }
            }
            Ok(c)
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
        // A sketch region: as its face (cf-transform-pattern's Mate connector 8, live: its centroid).
        Reference::SketchRegion { .. } => {
            let face = ev.profiles(one)?.into_iter().next().ok_or_else(|| not_found("sketch region"))?.shape;
            face_csys(ev, &face, inference, secondary)
        }
        _ => err("Failed to resolve mate connector origin."),
    }
}

pub fn mate_connector(ev: &mut EvalCtx) -> FResult<()> {
    // Live (dom/dialog-mate-connector.txt): an empty Origin entity fails.
    let Some(origin_ref) = ev.query("originQuery").first().cloned() else {
        return crate::regen::eval::err_at(&["originQuery"], "Cannot resolve entities.");
    };
    let inference = ev.enum_("entityInferenceType").to_string();
    let secondary = ev.query("secondaryOriginQuery");
    let mut cs = origin_csys(ev, &origin_ref, &inference, secondary)?;
    // Live 2050: an edge's MID_POINT with a secondary edge sits at that edge's midpoint, the axes the origin edge's.
    if inference == "MID_POINT"
        && matches!(origin_ref, Reference::Edge(_) | Reference::SketchEntity { .. })
        && let Some(e) = secondary.iter().find_map(|r| ev.resolve_edges(std::slice::from_ref(r)).first().and_then(|&(bi, ei)| ev.edge_shape(bi, ei).ok()))
    {
        cs.origin = edge_csys(&e, "MID_POINT")?.origin;
    }

    if ev.is("originType", "BETWEEN_ENTITIES") {
        // Live L39: no second entity, no field marked.
        let Some(r) = ev.query("originAdditionalQuery").first() else { return err("Failed to resolve mate connector coordinate system.") };
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
                return err("Failed to resolve mate connector. Secondary axis conflicts with primary axis.");
            }
            cs.x = x.normalize();
        }
    }

    // Live L38: flip and reorient first, then the move in that frame, then
    // the rotation about the moved origin (it doesn't change the move).
    if ev.bool("flipPrimary") {
        cs = cs.rotated(cs.x, std::f64::consts::PI);
    }
    // "+Y" turns X to the old −Y (live: X (0,−1,0) on a +Z face).
    let quarter = match ev.enum_("secondaryAxisType") {
        "PLUS_Y" => -1.0,
        "MINUS_X" => 2.0,
        "MINUS_Y" => 1.0,
        _ => 0.0,
    };
    if quarter != 0.0 {
        cs = cs.rotated(cs.z, quarter * std::f64::consts::FRAC_PI_2);
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

    // Live L37: the owner is required (the dialog fills it); connectors
    // saved before kept working without one.
    if ev.bool("allowOwnerEntity") && ev.bool("requireOwnerPart") {
        let owner = ev.query("ownerPart");
        // An empty owner is what connectorEditLogic would fill: the origin
        // entity's part, else the only part.
        let inferred = || {
            let one = std::slice::from_ref(&origin_ref);
            ev.resolve_faces(one).first().map(|x| x.0).or_else(|| ev.resolve_edges(one).first().map(|x| x.0)).or_else(|| (ev.ctx.bodies.len() == 1).then_some(0))
        };
        let unknown = if owner.is_empty() { !ev.params.bool("legacyNoOwner") && inferred().is_none() } else { ev.resolve_bodies(owner).is_empty() };
        if unknown {
            return crate::regen::eval::err_at(&["ownerPart"], "Cannot determine owner of mate connector.");
        }
        if ev.is("attachmentOption", "TO_SELECTION") && ev.query("attachTo").is_empty() {
            return crate::regen::eval::err_at(&["attachTo"], "Select an entity for the mate connector attachment.");
        }
    }

    let name = ev.name.to_string();
    let reference = Reference::MateConnector { feature: ev.feature, index: 0 };
    // The owner: the picked part, else the one the dialog would pick.
    if ev.bool("allowOwnerEntity") {
        let picked = ev.resolve_bodies(ev.query("ownerPart")).first().copied();
        let one = std::slice::from_ref(&origin_ref);
        let owner = picked.or_else(|| ev.resolve_faces(one).first().map(|x| x.0)).or_else(|| ev.resolve_edges(one).first().map(|x| x.0)).or_else(|| (ev.ctx.bodies.len() == 1).then_some(0));
        if let Some(bi) = owner {
            let id = ev.ctx.bodies[bi].id.clone();
            ev.ctx.connector_owners.push((reference.clone(), id));
        }
    }
    ev.ctx.connectors.push(Connector { reference, name, origin: cs.origin, x: cs.x, z: cs.z });
    Ok(())
}

/// Construction point on an edge (normalized parameter) or at coordinates.
pub fn point(ev: &mut EvalCtx) -> FResult<()> {
    let p = if ev.is("pointType", "MESH_POINT") {
        DVec3::new(ev.length("x")?, ev.length("y")?, ev.length("z")?)
    } else {
        let Some(&(bi, ei)) = ev.resolve_edges(ev.query("entities")).first() else { return err("Cannot resolve entities.") };
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
    // Live L36: the empty field is marked.
    let idx = ev.resolve_bodies(ev.query("bodies"));
    if idx.is_empty() {
        return crate::regen::eval::err_at(&["bodies"], "Select entities to create a composite part.");
    }
    let closed = ev.bool("closed");
    // Live L36: a part inside a closed composite (not the whole composite)
    // can't go into another closed composite.
    let in_closed = |id: &BodyId| ev.ctx.bodies.iter().any(|b| b.composite.as_ref().is_some_and(|c| c.closed && c.members.contains(id)));
    if closed && ev.query("bodies").iter().any(|r| matches!(r, Reference::Body(id) if in_closed(id))) {
        return err("Cannot create a closed composite part that partially overlaps another composite part.");
    }
    make_composite(ev, &idx, closed, 0)
}

/// The composite part of bodies `idx` (also made by imports: an open mesh
/// becomes a closed composite of its pieces, Onshape G27), body `index` of
/// the feature.
pub fn make_composite(ev: &mut EvalCtx, idx: &[usize], closed: bool, index: u32) -> FResult<()> {
    let name = format!("Composite part {}", ev.ctx.bodies.iter().filter(|b| b.name.starts_with("Composite part")).count() + 1);
    let members: Vec<Arc<Body>> = idx.iter().map(|&i| ev.ctx.bodies[i].clone()).collect();
    let id = BodyId { feature: ev.feature, index };
    // A new part: the next colour of the rotation (live part-studio/472:
    // after Part 1 light blue and Part 2 gray, the closed composite is dark
    // blue #3b61b4).
    let color = ev.ctx.alloc_color();
    let body = composite_body(&members, id, name, color, ev.feature, closed);
    let ids: Vec<BodyId> = members.iter().map(|b| b.id.clone()).collect();
    // Help (PartStudio__composite_part): Closed consumes the bodies (only the
    // composite is listed; graphically they are still there); an open
    // composite leaves them as separate parts "in addition to being part of
    // the composite part" (live: "Composite test" keeps Part 1 and Part 2).
    if closed {
        let pos = idx[0];
        ev.ctx.bodies.retain(|b| !ids.contains(&b.id));
        ev.ctx.bodies.insert(pos.min(ev.ctx.bodies.len()), Arc::new(body));
    } else {
        // After the members: entity lookups find the members first.
        ev.ctx.bodies.push(Arc::new(body));
    }
    Ok(())
}

/// A composite part's body: the compound of its members, keeping every member's face, edge and vertex names.
fn composite_body(members: &[Arc<Body>], id: BodyId, name: String, color: usize, feature: crate::reference::FeatureId, closed: bool) -> Body {
    let compound = Shape::compound(&members.iter().map(|b| b.shape.clone()).collect::<Vec<_>>());
    let nf = compound.count(ShapeType::Face);
    let ne = compound.count(ShapeType::Edge);
    let nv = compound.count(ShapeType::Vertex);
    let fallback = || Arc::new(crate::reference::TopoName::face(feature, crate::reference::Role::New(0)));
    let mut faces = vec![fallback(); nf];
    let mut edges = vec![fallback(); ne];
    let mut vertices = vec![fallback(); nv];
    for m in members {
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
        members.first().map_or(BodyKind::Solid, |b| b.kind)
    };
    let mut body = Body::new(id, name, kind, compound, faces, color, feature);
    body.edges = edges;
    body.vertices = vertices;
    body.composite = Some(crate::display::CompositeInfo { members: members.iter().map(|b| b.id.clone()).collect(), closed });
    body
}

/// Open composite parts made again of their members as they are now (live 1714: a fillet on a member changes that
/// member, the composite shows it; features never edit the composite itself, whose lookups go to the members). A
/// member gone (deleted, consumed) leaves the composite.
pub fn refresh_open_composites(ctx: &mut crate::regen::context::Context) {
    let bodies = ctx.bodies.untracked();
    let mut redo: Vec<(usize, Vec<Arc<Body>>)> = Vec::new();
    for (i, b) in bodies.iter().enumerate() {
        let Some(c) = b.composite.as_ref().filter(|c| !c.closed) else { continue };
        let members: Vec<Arc<Body>> = c.members.iter().filter_map(|id| bodies.iter().find(|m| &m.id == id).cloned()).collect();
        // Unchanged: the same members, each one's first entity still in the compound.
        let same = members.len() == c.members.len()
            && members.iter().all(|m| {
                let first = [ShapeType::Face, ShapeType::Edge, ShapeType::Vertex].into_iter().find_map(|k| m.shape.sub_shape(k, 0).ok().map(|s| (k, s)));
                first.is_some_and(|(k, s)| b.shape.index_of(&s, k).is_some())
            });
        if !same && !members.is_empty() {
            redo.push((i, members));
        }
    }
    for (i, members) in redo {
        let old = ctx.bodies.untracked()[i].clone();
        let body = composite_body(&members, old.id.clone(), old.name.clone(), old.color, old.id.feature, false);
        ctx.bodies.replace_untracked(i, Arc::new(body));
    }
}

/// Query variable: validates the name and that a "Selection" query resolves.
// TODO(context): store the resolved selection in the context once it has a
// query-variable table, so other features can reference it.
/// Query variable: evaluates the selection into persistent references and
/// stores them in the context's query-variable table under `name` (without
/// the `#`). Downstream query parameters select the variable as
/// `Reference::QueryVariable(name)`; the regenerator expands it before the
/// using feature runs (references resolve at the point of use, so geometry
/// that keeps its persistent names stays selected; "Evaluate on use" behaves
/// the same).
///
/// Selection types: Selection, Created by (features + entity type), Owned by
/// (bodies + entity type) and All solid bodies; additional queries combine
/// with union / subtract / intersect. Other types store nothing and warn.
pub fn query_variable(ev: &mut EvalCtx) -> FResult<()> {
    let name = ev.params.text("name").trim().trim_start_matches('#').to_string();
    if name.is_empty() || !name.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        // The Name is at fault, not the empty Selections (live ui-parity query-variable-tool).
        return crate::regen::eval::err_at(&["name"], "Variable name must be a letter followed by a string of letters or numbers.");
    }
    let st = ev.enum_("selectionType").to_string();
    // std sets each additional query's lowercase type for its row label
    // ("Union of created by"; SelectionTypeToLowercaseName).
    if ev.bool("addAdditionalQueries") {
        for (i, item) in ev.params.array("additionalQueries").iter().enumerate() {
            let t = lowercase_selection_type(item.enum_("addQselectionType"));
            ev.computed.push((format!("additionalQueries.{i}.addQlowercaseSelectionType"), crate::display::ComputedValue::Text(t.into())));
        }
    }
    let mut refs = query_selection(ev, ev.params, &st, "")?;
    if ev.bool("addAdditionalQueries") {
        for item in ev.params.array("additionalQueries") {
            let t = item.enum_("addQselectionType").to_string();
            let more = query_selection(ev, item, &t, "addQ")?;
            match item.enum_("booleanOperation") {
                "SUBTRACTION" => refs.retain(|r| !more.contains(r)),
                "INTERSECTION" => refs.retain(|r| more.contains(r)),
                _ => {
                    for r in more {
                        if !refs.contains(&r) {
                            refs.push(r);
                        }
                    }
                }
            }
        }
    }
    let unresolved = refs.iter().filter(|r| !resolves(ev, r)).count();
    if unresolved > 0 {
        ev.warnings.push(format!("{unresolved} selected entities could not be resolved"));
    }
    ev.ctx.query_variables.insert(name, refs);
    Ok(())
}

/// std queryVariable.fs SelectionTypeToLowercaseName.
fn lowercase_selection_type(t: &str) -> &'static str {
    match t {
        "CREATED_BY" => "created by",
        "OWNED_BY" => "owned by",
        "PROTRUSION" => "protrusion",
        "POCKET" => "pocket",
        "HOLE" => "hole",
        "FILLETS" => "fillets",
        "BOUNDED_FACES" => "bounded faces",
        "LOOP_CHAIN_CONNECTED" => "loop/chain connected",
        "PARALLEL" => "parallel",
        "TANGENT_CONNECTED" => "tangent connected",
        "MATCHING" => "matching",
        "ALL_SOLID_BODIES" => "all solid bodies",
        "EDGE_CONVEXITY" => "edge convexity",
        "ADJACENT" => "adjacent",
        _ => "selection",
    }
}

/// Parameter id with the additional-query prefix ("selectionQuery" ->
/// "addQselectionQuery", std queryVariable.fs).
fn qid(prefix: &str, id: &str) -> String {
    format!("{prefix}{id}")
}

fn query_selection(ev: &mut EvalCtx, p: &crate::params::ParamValues, st: &str, prefix: &str) -> FResult<Vec<Reference>> {
    // Entity type: per item for additional queries, else the feature's.
    let entity_type = p.enum_(&qid(prefix, "entityType")).to_string();
    let collect = |ev: &EvalCtx, bodies: &[usize], keep: &dyn Fn(&crate::naming::Name) -> bool| -> Vec<Reference> {
        let mut out = Vec::new();
        for &bi in bodies {
            let b = &ev.ctx.bodies[bi];
            match entity_type.as_str() {
                "FACE" => out.extend(b.faces.iter().filter(|n| keep(n)).map(|n| Reference::Face((**n).clone()))),
                "EDGE" => out.extend(b.edges.iter().filter(|n| keep(n)).map(|n| Reference::Edge((**n).clone()))),
                "VERTEX" => out.extend(b.vertices.iter().filter(|n| keep(n)).map(|n| Reference::Vertex((**n).clone()))),
                _ => out.push(Reference::Body(b.id.clone())),
            }
        }
        out
    };
    Ok(match st {
        "SELECTION" => p.query(&qid(prefix, "selectionQuery")).to_vec(),
        "CREATED_BY" => {
            let feats: Vec<crate::reference::FeatureId> = p.query(&qid(prefix, "createdByFeatures")).iter().filter_map(|r| if let Reference::Feature(f) = r { Some(*f) } else { None }).collect();
            let all: Vec<usize> = (0..ev.ctx.bodies.len()).collect();
            let by_body = entity_type == "BODY" || entity_type.is_empty();
            if by_body {
                ev.ctx.bodies.iter().filter(|b| feats.contains(&b.id.feature)).map(|b| Reference::Body(b.id.clone())).collect()
            } else {
                let edge_creator = |n: &crate::naming::Name| -> Option<crate::reference::FeatureId> {
                    match &**n {
                        crate::reference::TopoName::Edge { faces, .. } => faces.iter().filter_map(|f| f.creator()).max_by_key(|f| feats.contains(f)),
                        crate::reference::TopoName::Vertex { faces, .. } => faces.iter().filter_map(|f| f.creator()).max_by_key(|f| feats.contains(f)),
                        crate::reference::TopoName::Boundary { face, .. } => face.creator(),
                        other => other.creator(),
                    }
                };
                collect(ev, &all, &|n| edge_creator(n).is_some_and(|f| feats.contains(&f)))
            }
        }
        "OWNED_BY" => {
            let bodies = ev.resolve_bodies(p.query(&qid(prefix, "seedBodies")));
            collect(ev, &bodies, &|_| true)
        }
        "ALL_SOLID_BODIES" => ev.ctx.bodies.iter().filter(|b| b.kind == BodyKind::Solid).map(|b| Reference::Body(b.id.clone())).collect(),
        // std queryVariable.fs: the Create selection queries, from their seeds.
        "PROTRUSION" | "POCKET" | "HOLE" | "FILLETS" | "TANGENT_CONNECTED" | "PARALLEL" => {
            use crate::select::Criterion;
            let seed_type = p.enum_(&qid(prefix, "seedType")).to_string();
            let (seeds, criterion) = match st {
                "PROTRUSION" => (qid(prefix, "seedFaces"), Criterion::Protrusion),
                "POCKET" => (qid(prefix, "seedFaces"), Criterion::Pocket),
                "HOLE" => (qid(prefix, "seedFaces"), Criterion::Hole),
                "FILLETS" => (qid(prefix, "seedFaces"), Criterion::Fillets),
                "PARALLEL" => (qid(prefix, "seedEdges"), Criterion::ParallelEdges),
                _ if seed_type == "EDGE" => (qid(prefix, "seedEdges"), Criterion::TangentEdges),
                _ => (qid(prefix, "seedFaces"), Criterion::TangentFaces),
            };
            if st == "FILLETS" && !p.is(&qid(prefix, "filletCompareType"), "EQUAL") {
                ev.warnings.push("Query variable: only Equal fillet compare is supported yet".into());
            }
            let mut out: Vec<Reference> = Vec::new();
            for seed in p.query(&seeds).to_vec() {
                for r in crate::select::select_related(&ev.ctx, &seed, criterion) {
                    if !out.contains(&r) {
                        out.push(r);
                    }
                }
            }
            out
        }
        "BOUNDED_FACES" => {
            let bounds = p.query(&qid(prefix, "boundedFacesBounds")).to_vec();
            let mut out: Vec<Reference> = Vec::new();
            for seed in p.query(&qid(prefix, "seedFaces")).to_vec() {
                for r in crate::select::select_bounded(&ev.ctx, &seed, &bounds) {
                    if !out.contains(&r) {
                        out.push(r);
                    }
                }
            }
            out
        }
        "LOOP_CHAIN_CONNECTED" => crate::select::select_loops(&ev.ctx, p.query(&qid(prefix, "seedEdgesOrFaces"))),
        "EDGE_CONVEXITY" => {
            let kind = p.enum_(&qid(prefix, "edgeConvexityType")).to_string();
            let bodies = ev.resolve_bodies(p.query(&qid(prefix, "seedBodies")));
            bodies.into_iter().flat_map(|bi| crate::select::edges_by_convexity(&ev.ctx.bodies[bi], &kind)).collect()
        }
        other => {
            ev.warnings.push(format!("Query variable selection type {other} is not supported yet; it selects nothing"));
            Vec::new()
        }
    })
}

fn resolves(ev: &EvalCtx, r: &Reference) -> bool {
    let one = std::slice::from_ref(r);
    match r {
        Reference::Face(_) => !ev.resolve_faces(one).is_empty(),
        Reference::Edge(_) => !ev.resolve_edges(one).is_empty(),
        Reference::Body(_) => !ev.resolve_bodies(one).is_empty(),
        Reference::Vertex(_) | Reference::SketchEntity { .. } => ev.resolve_vertex_point(r).is_some() || ev.resolve_axis(r).is_some(),
        Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. } => ev.resolve_plane(r).is_some(),
        Reference::MateConnector { .. } | Reference::Implicit { .. } => ev.resolve_connector(r).is_some(),
        _ => true,
    }
}

/// Name entity: names are persistent already; nothing to compute.
pub fn name_entity(_ev: &mut EvalCtx) -> FResult<()> {
    Ok(())
}
