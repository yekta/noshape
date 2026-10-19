//! Plane (`cPlane`): a port of Onshape's `cplane.fs` (plane types, entity
//! checks and messages, default plane size and the `cPlaneLogic` editing
//! logic that picks the plane type from the preselection).

use crate::display::PlaneFrame;
use crate::params::{ParamValue, ParamValues};
use crate::reference::{DatumPlane, FeatureId, Reference};
use crate::regen::context::{Context, PlaneEntity};
use crate::regen::eval::{EvalCtx, FResult, err_at};
use glam::{DMat3, DMat4, DQuat, DVec2, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;

// Messages (ErrorStringEnum texts).
const MIDPLANE_MSG: &str = "Mid plane requires 2 points, 2 planes or 1 open edge.";
const OFFSET_MSG: &str = "Offset plane requires a plane, circle, ellipse, or arc to offset from.";
const POINT_PLANE_MSG: &str = "Point-Plane requires a point and a plane.";
const LINE_ANGLE_SELECT_MSG: &str = "Select an additional point, plane, or axis to specify where the angle is measured from.";
const LINE_AXIS_MSG: &str = "Line-Angle plane requires a reference line and a point, plane, or axis.";
const LINE_POINT_MSG: &str = "Point-Normal plane requires a point and an axis.";
const DEGENERATE_MSG: &str = "Could not determine the reference direction for a Line-Angle plane.";
const TOO_MANY_MSG: &str = "Too many entities are selected.";
const THREE_POINT_MSG: &str = "Three point plane requires 3 points.";
const COINCIDENT_MSG: &str = "Two or more points are coincident.";
const CURVE_POINT_MSG: &str = "Curve point plane requires a curve and a point.";
const TANGENT_MSG: &str = "Tangent plane requires a cylindrical surface and a point or plane.";
const TANGENT_SELECT_MSG: &str = "Select an additional point or plane to orient the plane.";
const TANGENT_PLANE_INVALID_MSG: &str = "Selected cylinder (axis) and plane are not parallel.";
const TANGENT_POINT_INVALID_MSG: &str = "Selected point must be outside the selected cylinder.";

/// Factor by which to extend the default plane size.
const PLANE_SIZE_EXTENSION_FACTOR: f64 = 0.2;
const PLANE_OFFSET_BOUND_VALUE: f64 = 500.0;
/// `TOLERANCE.zeroLength` (meters) and `TOLERANCE.zeroAngle`.
const ZERO_LENGTH: f64 = 1e-8;
const ZERO_ANGLE: f64 = 1e-11;
/// Display size of the default planes (see `StudioDisplay::defaults`).
const DEFAULT_PLANE_SIZE: f64 = 0.1;

fn fail<T>(m: &str) -> FResult<T> {
    err_at(&["entities"], m)
}

/// Onshape's `plane(origin, normal, x)` (normal and x normalized).
fn plane(origin: DVec3, normal: DVec3, x: DVec3) -> PlaneFrame {
    let n = normal.normalize();
    let x = x.normalize();
    PlaneFrame { origin, x, y: n.cross(x), normal: n }
}

/// Onshape's `plane(origin, normal)`: x is `perpendicularVector(normal)`.
fn plane_n(origin: DVec3, normal: DVec3) -> PlaneFrame {
    plane(origin, normal, perpendicular_vector(normal))
}

/// Port of FeatureScript's `perpendicularVector`.
pub fn perpendicular_vector(v: DVec3) -> DVec3 {
    if v.length_squared() < ZERO_LENGTH * ZERO_LENGTH {
        return DVec3::X;
    }
    let mut different = DVec3::ZERO;
    if v.x.abs() > 1.0366636528619326 * v.y.abs() {
        if v.x.abs() > 0.9517029893922334 * v.z.abs() {
            different.z = 1.0;
        } else {
            different.y = 1.0;
        }
    } else if v.y.abs() > 0.9204199474553859 * v.z.abs() {
        different.x = 1.0;
    } else {
        different.y = 1.0;
    }
    different.cross(v).normalize()
}

fn parallel_vectors(a: DVec3, b: DVec3) -> bool {
    a.cross(b).length_squared() < a.length_squared() * b.length_squared() * ZERO_ANGLE * ZERO_ANGLE
}

fn perpendicular_vectors(a: DVec3, b: DVec3) -> bool {
    let d = a.dot(b);
    d * d < a.length_squared() * b.length_squared() * ZERO_ANGLE * ZERO_ANGLE
}

/// `rotationMatrix3d(axis, angle) * v` (right-handed).
fn rotate(axis: DVec3, angle: f64, v: DVec3) -> DVec3 {
    DQuat::from_axis_angle(axis.normalize(), angle) * v
}

// ------------------------------------------------------------ entities

/// Entity type of a selection, as Onshape's queries see it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// A point (not a mate connector).
    Vertex,
    /// A mate connector (a vertex, also usable as a plane).
    Connector,
    Edge,
    /// A face: part faces, default and construction planes, sketch regions.
    Face,
    Other,
}

fn kind(ctx: &Context, r: &Reference) -> Kind {
    match r {
        Reference::Vertex(_) | Reference::Origin => Kind::Vertex,
        Reference::MateConnector { .. } | Reference::Implicit { .. } => Kind::Connector,
        Reference::Edge(_) => Kind::Edge,
        Reference::Face(_) | Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::SketchRegion { .. } => Kind::Face,
        Reference::SketchEntity { sketch, entity } => {
            let Some(s) = ctx.sketch(*sketch) else { return Kind::Other };
            let id = noshape_sketch::EntityId(*entity);
            if s.sketch.point(id).is_some() {
                Kind::Vertex
            } else if s.sketch.curve(id).is_some() {
                Kind::Edge
            } else {
                Kind::Other
            }
        }
        _ => Kind::Other,
    }
}

fn is_sketch(r: &Reference) -> bool {
    matches!(r, Reference::SketchEntity { .. } | Reference::SketchRegion { .. } | Reference::Sketch(_))
}

/// First kernel shape of the given type for a reference (sketch curves
/// become edges), moved by the feature pattern transform if any.
fn shape_of(ctx: &Context, xform: Option<&DMat4>, r: &Reference, ty: ShapeType) -> Option<Shape> {
    let s = crate::regen::measure::resolve_shapes(ctx, r).into_iter().find(|s| s.shape_type() == ty)?;
    match xform {
        Some(m) if !is_sketch(r) => k::transform(&s, &k::Transform::Matrix(*m), false).ok().map(|o| o.shape),
        _ => Some(s),
    }
}

fn edge_of(ev: &EvalCtx, r: &Reference) -> Option<Shape> {
    (kind(&ev.ctx, r) == Kind::Edge).then(|| shape_of(&ev.ctx, ev.xform.as_ref(), r, ShapeType::Edge)).flatten()
}

fn face_of(ev: &EvalCtx, r: &Reference) -> Option<Shape> {
    matches!(r, Reference::Face(_)).then(|| shape_of(&ev.ctx, ev.xform.as_ref(), r, ShapeType::Face)).flatten()
}

fn curve_of(ev: &EvalCtx, r: &Reference) -> Option<k::Curve> {
    edge_of(ev, r)?.edge_curve().ok()
}

fn is_line(ev: &EvalCtx, r: &Reference) -> bool {
    matches!(curve_of(ev, r), Some(k::Curve::Line { .. }))
}

/// `GeometryType.CYLINDER`.
fn cylinder_of(ev: &EvalCtx, r: &Reference) -> Option<(DVec3, DVec3, f64)> {
    match face_of(ev, r)?.face_surface().ok()? {
        k::Surface::Cylinder { origin, axis, radius, .. } => Some((origin, axis.normalize(), radius)),
        _ => None,
    }
}

/// `evPlane`: planar faces, default / construction planes, sketch regions
/// and mate connectors.
fn ev_plane(ev: &EvalCtx, r: &Reference) -> Option<PlaneFrame> {
    match kind(&ev.ctx, r) {
        Kind::Face | Kind::Connector => ev.resolve_plane(r),
        _ => None,
    }
}

/// `GeometryType.PLANE`.
fn is_planar(ev: &EvalCtx, r: &Reference) -> bool {
    kind(&ev.ctx, r) == Kind::Face && ev.resolve_plane(r).is_some()
}

/// `evAxis`: lines, circles and arcs, cylinders, cones, spheres, tori,
/// revolved surfaces and mate connectors (Z axis).
fn ev_axis(ev: &EvalCtx, r: &Reference) -> Option<(DVec3, DVec3)> {
    match kind(&ev.ctx, r) {
        Kind::Edge => match curve_of(ev, r)? {
            k::Curve::Line { origin, direction } => Some((origin, direction.normalize())),
            k::Curve::Circle { center, axis, .. } => Some((center, axis.normalize())),
            _ => None,
        },
        Kind::Face => match face_of(ev, r)?.face_surface().ok()? {
            k::Surface::Cylinder { origin, axis, .. } | k::Surface::Cone { origin, axis, .. } => Some((origin, axis.normalize())),
            k::Surface::Sphere { center, axis, .. } | k::Surface::Torus { center, axis, .. } => Some((center, axis.normalize())),
            k::Surface::Revolution { axis_origin, axis_dir } => Some((axis_origin, axis_dir.normalize())),
            _ => None,
        },
        Kind::Connector => ev.resolve_connector(r).map(|c| (c.origin, c.z.normalize())),
        _ => None,
    }
}

/// `evVertexPoint` of a point or mate connector.
fn ev_point(ev: &EvalCtx, r: &Reference) -> Option<DVec3> {
    match kind(&ev.ctx, r) {
        Kind::Vertex | Kind::Connector => ev.resolve_vertex_point(r),
        _ => None,
    }
}

/// Plane of a circular or elliptical edge (`planeFromCircularEdge`): the
/// curve's coordinate system.
fn circle_frame(curve: &k::Curve) -> Option<PlaneFrame> {
    match *curve {
        k::Curve::Circle { center, axis, x_dir, .. } | k::Curve::Ellipse { center, axis, x_dir, .. } => Some(plane(center, axis, x_dir)),
        _ => None,
    }
}

/// Frame of a circular or elliptical edge (for the offset manipulator).
pub fn circular_edge_frame(ctx: &Context, r: &Reference) -> Option<PlaneFrame> {
    let e = crate::regen::measure::resolve_shapes(ctx, r).into_iter().find(|s| s.shape_type() == ShapeType::Edge)?;
    circle_frame(&e.edge_curve().ok()?)
}

// ------------------------------------------------------------ plane size

/// 2D bounding box of a face-like entity in a frame's coordinates, and
/// whether the entity is a construction object (planes).
fn entity_box(ev: &EvalCtx, r: &Reference, frame: &PlaneFrame) -> Option<(DVec2, DVec2, bool)> {
    let rect = |f: PlaneFrame, hw: f64, hh: f64| {
        let pts = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(a, b)| frame.to_local(f.origin + f.x * (a * hw) + f.y * (b * hh)));
        let min = pts.iter().fold(DVec2::splat(f64::INFINITY), |m, p| m.min(*p));
        let max = pts.iter().fold(DVec2::splat(f64::NEG_INFINITY), |m, p| m.max(*p));
        (min, max, true)
    };
    match r {
        Reference::Plane(d) => {
            let f = ev.resolve_plane(r).unwrap_or(match d {
                DatumPlane::Top => PlaneFrame::TOP,
                DatumPlane::Front => PlaneFrame::FRONT,
                DatumPlane::Right => PlaneFrame::RIGHT,
            });
            Some(rect(f, DEFAULT_PLANE_SIZE * 0.5, DEFAULT_PLANE_SIZE * 0.5))
        }
        Reference::FeaturePlane(_) => {
            let p = ev.ctx.planes.iter().find(|p| &p.reference == r)?;
            let f = ev.resolve_plane(r)?;
            Some(rect(f, p.half_width, p.half_height))
        }
        Reference::Face(_) | Reference::SketchRegion { .. } => {
            let shape = match r {
                Reference::Face(_) => face_of(ev, r)?,
                _ => ev.profiles(std::slice::from_ref(r)).ok()?.into_iter().next()?.shape,
            };
            let to_world = DMat4::from_cols(frame.x.extend(0.0), frame.y.extend(0.0), frame.normal.extend(0.0), frame.origin.extend(1.0));
            let local = k::transform(&shape, &k::Transform::Matrix(to_world.inverse()), false).ok()?.shape;
            let b = local.bounding_box(false).ok()?;
            Some((b.min.truncate(), b.max.truncate(), false))
        }
        _ => None,
    }
}

/// `getExpandedPlaneBounds` over the face entities of `refs`; `None` when
/// none of them is a face (the default size applies).
fn expanded_bounds(ev: &EvalCtx, refs: &[&Reference], frame: &PlaneFrame) -> Option<(f64, f64)> {
    let mut acc: Option<(DVec2, DVec2)> = None;
    let mut construction = true;
    for r in refs {
        let (lo, hi, c) = entity_box(ev, r, frame)?;
        construction &= c;
        acc = Some(match acc {
            Some((a, b)) => (a.min(lo), b.max(hi)),
            None => (lo, hi),
        });
    }
    let (lo, hi) = acc?;
    let factor = if construction { 0.0 } else { PLANE_SIZE_EXTENSION_FACTOR };
    let (w, h) = ((hi.x - lo.x).abs(), (hi.y - lo.y).abs());
    let offset = w.max(h) * factor;
    Some((w + offset, h + offset))
}

fn max_project(length: f64, plane_normal: DVec3, axis: DVec3, origin1d: f64, bound: f64) -> f64 {
    let scale = plane_normal.cross(axis).length();
    let projected = length * scale;
    let left = origin1d + bound;
    let right = bound - origin1d;
    if projected < left && projected < right {
        return length;
    }
    projected.min(left).min(right) / scale
}

/// `getTruncatedPlaneSize`.
fn truncated_plane_size(size: f64, origin: DVec3, normal: DVec3, bound: f64) -> f64 {
    let n = normal.normalize();
    size.min(max_project(size, n, DVec3::Z, origin.z, bound)).min(max_project(size, n, DVec3::Y, origin.y, bound)).min(max_project(size, n, DVec3::X, origin.x, bound))
}

// ------------------------------------------------------------ evaluation

/// `cPlane` body.
pub fn plane_feature(ev: &mut EvalCtx) -> FResult<()> {
    let refs: Vec<Reference> = ev.query("entities").to_vec();
    let n = refs.len();
    let ty = match ev.enum_("cplaneType") {
        // Ids used before the port.
        "POINT_NORMAL" => "LINE_POINT".to_string(),
        "TANGENT_CYLINDER" => "TANGENT_PLANE".to_string(),
        t => t.to_string(),
    };
    let sign = if ev.bool("oppositeDirection") { -1.0 } else { 1.0 };
    let flip_alignment = ev.bool("flipAlignment");
    let kinds: Vec<Kind> = refs.iter().map(|r| kind(&ev.ctx, r)).collect();
    let of = |k: Kind| refs.iter().zip(&kinds).filter(move |(_, x)| **x == k).map(|(r, _)| r);
    // Face entities used for the default plane size (qEntityFilter FACE).
    let faces: Vec<&Reference> = of(Kind::Face).collect();
    let mut width = ev.length("width").unwrap_or(0.15);
    let mut height = ev.length("height").unwrap_or(0.15);
    let mut size: Option<(f64, f64)> = None;

    let frame = match ty.as_str() {
        "OFFSET" => {
            if n < 1 {
                return fail(OFFSET_MSG);
            }
            if n > 1 {
                return fail(TOO_MANY_MSG);
            }
            let reference = if kinds[0] == Kind::Edge {
                let Some(f) = curve_of(ev, &refs[0]).as_ref().and_then(circle_frame) else { return fail(OFFSET_MSG) };
                f
            } else {
                let Some(f) = ev_plane(ev, &refs[0]) else { return fail(OFFSET_MSG) };
                f
            };
            let d = ev.length("offset")? * sign;
            let f = PlaneFrame { origin: reference.origin + reference.normal * d, ..reference };
            size = expanded_bounds(ev, &faces, &f);
            f
        }
        "PLANE_POINT" => {
            if n < 2 {
                return fail(POINT_PLANE_MSG);
            }
            if n > 2 {
                return fail(TOO_MANY_MSG);
            }
            let pl = refs.iter().zip(&kinds).filter(|(_, k)| matches!(k, Kind::Face | Kind::Connector)).find_map(|(r, _)| ev_plane(ev, r));
            let pt = of(Kind::Vertex).find_map(|r| ev_point(ev, r));
            let (Some(pl), Some(pt)) = (pl, pt) else { return fail(POINT_PLANE_MSG) };
            let f = PlaneFrame { origin: pt, ..pl };
            size = expanded_bounds(ev, &faces, &f);
            f
        }
        "LINE_ANGLE" => {
            let angle = ev.angle("angle")? * sign;
            let (f, axis) = line_angle_plane(ev, &refs, angle)?;
            // getPlaneDefaultSize: the length (or diameter) of the first edge.
            if let Some(e) = refs.iter().zip(&kinds).find(|(_, k)| **k == Kind::Edge).and_then(|(r, _)| edge_of(ev, r)) {
                let mut s = match e.edge_curve() {
                    Ok(k::Curve::Circle { radius, .. }) => radius * 2.0,
                    _ => e.length().unwrap_or(0.0),
                };
                s *= 1.0 + PLANE_SIZE_EXTENSION_FACTOR;
                s = truncated_plane_size(s, axis.0, axis.1, PLANE_OFFSET_BOUND_VALUE);
                size = Some((s, s));
            }
            f
        }
        "LINE_POINT" => {
            if n < 2 {
                return fail(LINE_POINT_MSG);
            }
            if n > 2 {
                return fail(TOO_MANY_MSG);
            }
            let axis = refs.iter().zip(&kinds).filter(|(_, k)| matches!(k, Kind::Edge | Kind::Face)).find_map(|(r, _)| ev_axis(ev, r));
            let pt = refs.iter().zip(&kinds).filter(|(_, k)| matches!(k, Kind::Vertex | Kind::Connector)).find_map(|(r, _)| ev_point(ev, r));
            let (Some((_, d)), Some(p)) = (axis, pt) else { return fail(LINE_POINT_MSG) };
            plane_n(p, d)
        }
        "THREE_POINT" => {
            let pts: Vec<DVec3> = refs.iter().zip(&kinds).filter(|(_, k)| matches!(k, Kind::Vertex | Kind::Connector)).filter_map(|(r, _)| ev_point(ev, r)).collect();
            if pts.len() < 3 {
                return fail(THREE_POINT_MSG);
            }
            if n > 3 {
                return fail(TOO_MANY_MSG);
            }
            let normal = (pts[2] - pts[0]).cross(pts[1] - pts[0]);
            if normal.length() < ZERO_LENGTH {
                return fail(COINCIDENT_MSG);
            }
            let center = (pts[0] + pts[1] + pts[2]) / 3.0;
            let f = plane(center, normal, pts[1] - pts[0]);
            let max_len = pts[..3].iter().map(|p| (center - *p).length()).fold(0.0, f64::max);
            let s = max_len * 2.0;
            width = s + s * PLANE_SIZE_EXTENSION_FACTOR;
            height = width;
            f
        }
        "MID_PLANE" => {
            let vertices: Vec<&Reference> = of(Kind::Vertex).collect();
            let edges: Vec<&Reference> = of(Kind::Edge).collect();
            let planes = faces.len() + of(Kind::Connector).count();
            let f = if vertices.len() == 2 {
                if n > 2 {
                    return fail(TOO_MANY_MSG);
                }
                let (Some(a), Some(b)) = (ev_point(ev, vertices[0]), ev_point(ev, vertices[1])) else { return fail(MIDPLANE_MSG) };
                let normal = b - a;
                if normal.length() <= ZERO_LENGTH {
                    return fail(COINCIDENT_MSG);
                }
                plane_n((a + b) * 0.5, normal)
            } else if edges.len() == 1 {
                if n > 1 {
                    return fail(TOO_MANY_MSG);
                }
                let Some((a, b)) = edge_of(ev, edges[0]).and_then(|e| e.endpoints().ok()) else { return fail(MIDPLANE_MSG) };
                let normal = b - a;
                if normal.length() <= ZERO_LENGTH {
                    return fail(MIDPLANE_MSG);
                }
                plane_n((a + b) * 0.5, normal)
            } else if planes == 2 {
                if n > 2 {
                    return fail(TOO_MANY_MSG);
                }
                let (Some(p1), Some(p2)) = (ev_plane(ev, &refs[0]), ev_plane(ev, &refs[1])) else { return fail(MIDPLANE_MSG) };
                mid_plane_from_two_planes(p1, p2, flip_alignment)
            } else {
                return fail(MIDPLANE_MSG);
            };
            size = expanded_bounds(ev, &faces, &f);
            f
        }
        "CURVE_POINT" => {
            if n < 2 {
                return fail(CURVE_POINT_MSG);
            }
            if n > 2 {
                return fail(TOO_MANY_MSG);
            }
            let edge = of(Kind::Edge).find_map(|r| edge_of(ev, r));
            let pt = refs.iter().zip(&kinds).filter(|(_, k)| matches!(k, Kind::Vertex | Kind::Connector)).find_map(|(r, _)| ev_point(ev, r));
            let (Some(edge), Some(p)) = (edge, pt) else { return fail(CURVE_POINT_MSG) };
            let Some((origin, dir)) = closest_tangent(&edge, p) else { return fail(CURVE_POINT_MSG) };
            plane_n(origin, dir)
        }
        "TANGENT_PLANE" => tangent_plane(ev, &refs, &kinds, flip_alignment)?,
        other => return err_at(&["cplaneType"], format!("Plane type {other} is not supported")),
    };

    let frame = if ev.bool("flipNormal") { PlaneFrame { normal: -frame.normal, y: -frame.y, ..frame } } else { frame };
    if let Some((w, h)) = size {
        (width, height) = (w, h);
    }
    let name = ev.name.to_string();
    ev.ctx.planes.push(PlaneEntity { reference: Reference::FeaturePlane(ev.feature), name, frame, half_width: (width * 0.5).max(5e-6), half_height: (height * 0.5).max(5e-6) });
    Ok(())
}

/// `lineAnglePlane`; also returns the rotation axis (origin, direction).
fn line_angle_plane(ev: &mut EvalCtx, refs: &[Reference], angle: f64) -> FResult<(PlaneFrame, (DVec3, DVec3))> {
    if refs.len() == 1 {
        ev.infos.push(LINE_ANGLE_SELECT_MSG.to_string());
        let Some((o, d)) = ev_axis(ev, &refs[0]) else { return fail(LINE_AXIS_MSG) };
        let base = perpendicular_vector(d);
        let normal = rotate(d, angle, base);
        return Ok((plane(o, normal, d), (o, d)));
    }
    if refs.len() < 2 {
        return fail(LINE_AXIS_MSG);
    }
    if refs.len() > 2 {
        return fail(TOO_MANY_MSG);
    }
    let mut second = &refs[1];
    let mut axis1 = ev_axis(ev, &refs[0]);
    let mut axis2 = ev_axis(ev, &refs[1]);
    if axis1.is_none() {
        // The plane or point was selected first: swap.
        if axis2.is_none() {
            return fail(LINE_AXIS_MSG);
        }
        axis1 = axis2.take();
        second = &refs[0];
    }
    let Some((o1, d1)) = axis1 else { return fail(LINE_AXIS_MSG) };
    let in_plane = match axis2 {
        Some((o2, d2)) => Some(if parallel_vectors(d1, d2) { o2 - o1 } else { d2 }),
        None => match ev_plane(ev, second) {
            Some(p) => Some(d1.cross(p.normal)),
            None => ev_point(ev, second).map(|p| p - o1),
        },
    };
    let Some(in_plane) = in_plane else { return fail(LINE_AXIS_MSG) };
    let normal = d1.cross(in_plane);
    if normal.length_squared() < ZERO_LENGTH * ZERO_LENGTH {
        return fail(DEGENERATE_MSG);
    }
    let normal = rotate(d1, angle, normal);
    Ok((plane(o1, normal, d1), (o1, d1)))
}

/// `createMidPlaneFromTwoPlanes`.
fn mid_plane_from_two_planes(p1: PlaneFrame, p2: PlaneFrame, flip_alignment: bool) -> PlaneFrame {
    // By default the bisector of two adjacent planes of a solid (a wedge):
    // negate plane 2's normal. The other solution is perpendicular to it.
    let n2 = if flip_alignment { p2.normal } else { -p2.normal };
    let mid = (p1.origin + p2.origin) * 0.5;
    let dir = p1.normal.cross(n2);
    if dir.length() < ZERO_ANGLE {
        return plane(mid, p1.normal, p1.x);
    }
    let normal = (p1.normal + n2).normalize();
    let f = plane(mid, normal, dir.normalize());
    // Origin: the midpoint of the origins projected onto the plane through
    // the intersection line (which contains the midpoint's projection).
    let line_point = plane_intersection_point(p1.origin, p1.normal, p2.origin, n2, dir.normalize());
    PlaneFrame { origin: mid + normal * normal.dot(line_point - mid), ..f }
}

/// Point of `intersection(plane1, plane2)`.
fn plane_intersection_point(o1: DVec3, n1: DVec3, o2: DVec3, n2: DVec3, dir: DVec3) -> DVec3 {
    let rhs = DVec3::new(n1.dot(o1), n2.dot(o2), dir.dot((o1 + o2) * 0.5));
    // Rows n1, n2, dir.
    let m = DMat3::from_cols(n1, n2, dir).transpose();
    m.inverse() * rhs
}

/// Closest point of `p` on the edge's curve (extended for lines, circles
/// and ellipses) and the curve tangent there (`evDistance` with
/// `extendSide1` + `evEdgeTangentLine`).
fn closest_tangent(edge: &Shape, p: DVec3) -> Option<(DVec3, DVec3)> {
    let info = edge.edge_info().ok()?;
    let t = match info.curve {
        k::Curve::Line { origin, direction } => {
            let d = direction.normalize();
            let q = origin + d * (p - origin).dot(d);
            let dir = edge.tangent_at(info.first_param).ok()?;
            return Some((q, dir));
        }
        k::Curve::Circle { .. } | k::Curve::Ellipse { .. } => {
            // Periodic: search the whole curve.
            closest_param(edge, p, info.first_param, info.first_param + std::f64::consts::TAU)?
        }
        _ => closest_param(edge, p, info.first_param, info.last_param)?,
    };
    Some((edge.point_at(t).ok()?, edge.tangent_at(t).ok()?))
}

/// Parameter of the point of the curve closest to `p` in `[a, b]`.
fn closest_param(edge: &Shape, p: DVec3, a: f64, b: f64) -> Option<f64> {
    let d = |t: f64| edge.point_at(t).map(|q| (q - p).length_squared()).unwrap_or(f64::INFINITY);
    const N: usize = 64;
    let step = (b - a) / N as f64;
    let (mut best, mut best_d) = (a, d(a));
    for i in 1..=N {
        let t = a + step * i as f64;
        let v = d(t);
        if v < best_d {
            (best, best_d) = (t, v);
        }
    }
    // Golden-section refinement around the best sample.
    let (mut lo, mut hi) = ((best - step).max(a), (best + step).min(b));
    let g = 0.5 * (5f64.sqrt() - 1.0);
    for _ in 0..80 {
        let (c1, c2) = (hi - g * (hi - lo), lo + g * (hi - lo));
        if d(c1) < d(c2) {
            hi = c2;
        } else {
            lo = c1;
        }
    }
    best_d.is_finite().then_some(0.5 * (lo + hi))
}

/// The TANGENT_PLANE branch.
fn tangent_plane(ev: &mut EvalCtx, refs: &[Reference], kinds: &[Kind], flip_alignment: bool) -> FResult<PlaneFrame> {
    let n = refs.len();
    if n == 0 {
        return fail(TANGENT_MSG);
    }
    if n > 2 {
        return fail(TOO_MANY_MSG);
    }
    let cylinders: Vec<(usize, (DVec3, DVec3, f64))> = refs.iter().enumerate().filter_map(|(i, r)| cylinder_of(ev, r).map(|c| (i, c))).collect();
    let [(ci, (origin, z, radius))] = cylinders[..] else { return fail(TANGENT_MSG) };
    let mut flip = flip_alignment;
    let mut normal = perpendicular_vector(z);
    if n == 1 {
        ev.infos.push(TANGENT_SELECT_MSG.to_string());
    } else {
        // The reference entity: a point (or mate connector) or a plane.
        let others: Vec<usize> = (0..n).filter(|&i| i != ci && (matches!(kinds[i], Kind::Vertex | Kind::Connector) || is_planar(ev, &refs[i]))).collect();
        let [oi] = others[..] else { return fail(TANGENT_MSG) };
        let r = &refs[oi];
        if kinds[oi] != Kind::Vertex {
            let Some(p) = ev_plane(ev, r) else { return fail(TANGENT_MSG) };
            normal = p.normal;
        } else {
            let Some(p) = ev_point(ev, r) else { return fail(TANGENT_MSG) };
            let point = p - z * (p - origin).dot(z) - origin;
            let len = point.length();
            if len < radius - ZERO_LENGTH {
                return fail(TANGENT_POINT_INVALID_MSG);
            }
            let mut angle = 0.0;
            if len > radius + ZERO_LENGTH {
                angle = (radius / len).acos();
            }
            if flip {
                // The tangent plane on the other side of the cylinder.
                angle = std::f64::consts::TAU - angle;
                flip = false;
            }
            normal = rotate(z, angle, point / len);
        }
        if !perpendicular_vectors(z, normal) {
            return fail(TANGENT_PLANE_INVALID_MSG);
        }
    }
    let mut f = plane(origin + normal * radius, normal, z);
    if flip {
        f.origin -= f.normal * radius * 2.0;
        f = PlaneFrame { normal: -f.normal, y: -f.y, ..f };
    }
    Ok(f)
}

// ------------------------------------------------------------ editing logic

/// `cPlaneLogic`: on preselection, pick the plane type from the selected
/// entities (run once when a new Plane dialog opens).
pub fn cplane_logic(ctx: &Context, params: &mut ParamValues) {
    let refs: Vec<Reference> = params.query("entities").to_vec();
    let units = noshape_units::UnitPrefs::default();
    let vars = BTreeMap::new();
    let snapshot = params.clone();
    let ev = EvalCtx::new(FeatureId(0), "", &snapshot, &units, ctx.clone(), &vars);
    let kinds: Vec<Kind> = refs.iter().map(|r| kind(ctx, r)).collect();
    let count = |f: &dyn Fn(usize) -> bool| (0..refs.len()).filter(|&i| f(i)).count();
    let total = refs.len();
    let vertices = count(&|i| kinds[i] == Kind::Vertex);
    let lines = count(&|i| kinds[i] == Kind::Edge && is_line(&ev, &refs[i]));
    let connectors = count(&|i| kinds[i] == Kind::Connector);
    let planes = count(&|i| is_planar(&ev, &refs[i])) + connectors;
    let curves = count(&|i| kinds[i] == Kind::Edge && !is_line(&ev, &refs[i]));
    let cylinders = count(&|i| cylinder_of(&ev, &refs[i]).is_some());
    let ty = match total {
        1 => {
            let circular = || curve_of(&ev, &refs[0]).as_ref().and_then(circle_frame).is_some();
            let open_edge = || kinds[0] == Kind::Edge && edge_of(&ev, &refs[0]).is_some_and(|e| e.is_closed().is_ok_and(|c| !c));
            if planes == 1 || (curves == 1 && circular()) {
                Some("OFFSET")
            } else if lines + curves == 1 && open_edge() {
                Some("MID_PLANE")
            } else if ev_axis(&ev, &refs[0]).is_some() {
                Some("LINE_ANGLE")
            } else {
                None
            }
        }
        2 => {
            if planes == 1 && vertices == 1 {
                Some("PLANE_POINT")
            } else if planes == 2 || vertices == 2 {
                Some("MID_PLANE")
            } else if curves == 1 && vertices == 1 {
                Some("CURVE_POINT")
            } else if cylinders == 1 && vertices + planes == 1 {
                Some("TANGENT_PLANE")
            } else if vertices == 1 {
                Some("LINE_POINT")
            } else if lines >= 1 && lines + vertices + planes == 2 {
                Some("LINE_ANGLE")
            } else {
                None
            }
        }
        3 if vertices + connectors == 3 => Some("THREE_POINT"),
        _ => None,
    };
    if let Some(t) = ty {
        params.set("cplaneType", ParamValue::Enum(t.into()));
    }
}
