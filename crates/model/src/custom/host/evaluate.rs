//! `@ev*` natives: geometric evaluation of queried entities.

use super::query::{EType, Ent, QueryCx, entity_axis, entity_type_of, with_ents};
use super::{FsContext, context_arg, convert::*, raise_error, raise_faulty, std_enum, std_tag};
use glam::{DVec2, DVec3};
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::{Map, Value};
use noshape_kernel as k;
use noshape_kernel::ShapeType;

pub const NATIVES: &[(&str, NativeFn)] = &[
    ("clusterBodies", cluster_bodies),
    ("evApproximateBSplineCurve", ev_approximate_bspline_curve),
    ("evVertexPoint", ev_vertex_point),
    ("evLine", ev_line),
    ("evAxis", ev_axis),
    ("evPlane", ev_plane),
    ("evOwnerSketchPlane", ev_owner_sketch_plane),
    ("evEdgeTangentLines", ev_edge_tangent_lines),
    ("evFaceTangentPlanes", ev_face_tangent_planes),
    ("evFaceTangentPlanesAtEdge", ev_face_tangent_planes_at_edge),
    ("evLength", ev_length),
    ("evArea", ev_area),
    ("evVolume", ev_volume),
    ("evBox", ev_box),
    ("evApproximateCentroid", ev_centroid),
    ("evApproximateMassProperties", ev_mass_properties),
    ("evDistance", ev_distance),
    ("evMateConnector", ev_mate_connector),
    ("evMateConnectorCoordSystem", ev_mate_connector),
    ("evCurveDefinition", ev_curve_definition),
    ("evSurfaceDefinition", ev_surface_definition),
    ("evEdgeConvexity", ev_edge_convexity),
    ("evPlanarEdge", ev_planar_edge),
    ("evPlanarEdges", ev_planar_edges),
    ("evFilletRadius", ev_fillet_radius),
    ("evEdgeCurvatures", ev_edge_curvatures),
    ("evFaceCurvatures", ev_face_curvatures),
    ("evRaycast", ev_raycast),
    ("evCollisionDetection", ev_collision_detection),
    ("evFacePeriodicity", ev_face_periodicity),
];

/// The query field of an evaluation argument map.
fn field(a: &[Value], key: &str) -> Value {
    a.get(1).map(|d| d.get(key).clone()).unwrap_or_default()
}

/// Run `f` on the first entity of the definition's query field `key`. An
/// empty query raises CANNOT_RESOLVE_ENTITIES with the field as faulty
/// parameter (live: evPlane of qNothing); `f` returning `None` raises `err`.
fn on_first<T>(i: &mut Interp, cx: &FsContext, a: &[Value], key: &str, err: &str, f: impl FnOnce(&QueryCx, &Ent) -> Option<T>) -> R<T> {
    let q = field(a, key);
    let r = with_ents(i, cx, &q, |_, qc, ents| Ok(ents.first().map(|e| f(qc, e))))?;
    match r {
        Some(Some(v)) => Ok(v),
        Some(None) => Err(raise_error(i, err, None)),
        None => Err(raise_faulty(i, "CANNOT_RESOLVE_ENTITIES", &[key])),
    }
}

fn ev_vertex_point(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let p = on_first(i, &cx, &a, "vertex", "CANNOT_EVALUATE_VERTEX", |q, e| (entity_type_of(e) == EType::Vertex).then(|| q.point(e)).flatten())?;
    Ok(point_value(p))
}

fn ev_line(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let (o, d) = on_first(i, &cx, &a, "edge", "INVALID_INPUT", |q, e| {
        let s = q.shape(e)?;
        let info = s.edge_info().ok()?;
        match info.curve {
            k::Curve::Line { direction, .. } => Some((info.start, direction.normalize_or_zero())),
            _ => None,
        }
    })?;
    Ok(line_value(o, d))
}

fn ev_axis(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let (o, d) = on_first(i, &cx, &a, "axis", "NO_ROTATION_AXIS", |q, e| match e {
        Ent::Connector(c) => {
            let c = &q.st.ctx.connectors[*c];
            Some((c.origin, c.z))
        }
        Ent::Datum(_) | Ent::Plane(_) => q.plane(e).map(|p| (p.origin, p.normal)),
        _ => entity_axis(q, e).map(|(o, d)| {
            // Lines start at their start point.
            if let Some(info) = q.shape(e).and_then(|s| s.edge_info().ok())
                && matches!(info.curve, k::Curve::Line { .. })
            {
                return (info.start, d);
            }
            (o, d)
        }),
    })?;
    Ok(line_value(o, d))
}

fn ev_plane(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let p = on_first(i, &cx, &a, "face", "CANNOT_RESOLVE_PLANE", |q, e| q.plane(e))?;
    Ok(plane_value(&p))
}

fn ev_owner_sketch_plane(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let p = on_first(i, &cx, &a, "entity", "CANNOT_RESOLVE_PLANE", |q, e| match e {
        Ent::SketchBody(s) | Ent::SketchRegion(s, _) | Ent::SketchEdge(s, _) | Ent::SketchVertex(s, _) => Some(q.st.ctx.sketches[*s].frame),
        _ => None,
    })?;
    Ok(plane_value(&p))
}

/// Edge parameter (0..1) to curve parameter, by arc length when asked.
pub(super) fn edge_param(s: &k::Shape, info: &k::EdgeInfo, t: f64, arc_length: bool) -> f64 {
    let lin = info.first_param + (info.last_param - info.first_param) * t;
    if !arc_length || matches!(info.curve, k::Curve::Line { .. } | k::Curve::Circle { .. }) {
        return lin;
    }
    // Invert arc length by sampling.
    let n = 64;
    let mut acc = vec![0.0];
    let mut prev = s.point_at(info.first_param).unwrap_or(info.start);
    for j in 1..=n {
        let u = info.first_param + (info.last_param - info.first_param) * j as f64 / n as f64;
        let p = s.point_at(u).unwrap_or(prev);
        acc.push(acc[j - 1] + (p - prev).length());
        prev = p;
    }
    let total = acc[n];
    if total <= 0.0 {
        return lin;
    }
    let target = t * total;
    let j = acc.iter().position(|x| *x >= target).unwrap_or(n).max(1);
    let f = (target - acc[j - 1]) / (acc[j] - acc[j - 1]).max(1e-300);
    info.first_param + (info.last_param - info.first_param) * ((j - 1) as f64 + f) / n as f64
}

fn ev_edge_tangent_lines(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let params: Vec<f64> = field(&a, "parameters").as_array().map(|v| v.iter().filter_map(|x| x.as_number()).collect()).unwrap_or_default();
    let arc = field(&a, "arcLengthParameterization").as_bool().unwrap_or(true);
    let lines = on_first(i, &cx, &a, "edge", "NO_TANGENT_LINE", |q, e| {
        let s = q.shape(e)?;
        let info = s.edge_info().ok()?;
        let mut out = Vec::new();
        for t in &params {
            let u = edge_param(&s, &info, *t, arc);
            out.push(line_value(s.point_at(u).ok()?, s.tangent_at(u).ok()?.normalize_or_zero()));
        }
        Some(out)
    })?;
    Ok(Value::array(lines))
}

fn face_tangent_plane(s: &k::Shape, uv: DVec2) -> Option<crate::display::PlaneFrame> {
    let b = s.uv_bounds().ok()?;
    let u = b[0] + (b[1] - b[0]) * uv.x;
    let v = b[2] + (b[3] - b[2]) * uv.y;
    let p = s.point_at_uv(u, v).ok()?;
    let n = s.normal_at_uv(u, v).ok()?.normalize_or_zero();
    // x along the u direction.
    let du = (b[1] - b[0]) * 1e-6;
    let p2 = s.point_at_uv(u + du, v).ok().unwrap_or(p);
    let mut x = (p2 - p) - n * (p2 - p).dot(n);
    if x.length_squared() < 1e-30 {
        x = any_perpendicular(n);
    }
    let x = x.normalize_or_zero();
    Some(crate::display::PlaneFrame { origin: p, x, y: n.cross(x), normal: n })
}

fn ev_face_tangent_planes(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let params: Vec<DVec2> = field(&a, "parameters").as_array().map(|v| v.iter().filter_map(vec2_of).collect()).unwrap_or_default();
    let planes = on_first(i, &cx, &a, "face", "NO_TANGENT_PLANE", |q, e| {
        if let Some(p) = matches!(e, Ent::Datum(_) | Ent::Plane(_) | Ent::SketchRegion(..)).then(|| q.plane(e)).flatten() {
            return Some(params.iter().map(|_| plane_value(&p)).collect::<Vec<_>>());
        }
        let s = q.shape(e)?;
        params.iter().map(|uv| face_tangent_plane(&s, *uv).map(|p| plane_value(&p))).collect()
    })?;
    Ok(Value::array(planes))
}

fn ev_face_tangent_planes_at_edge(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let params: Vec<f64> = field(&a, "parameters").as_array().map(|v| v.iter().filter_map(|x| x.as_number()).collect()).unwrap_or_default();
    let face_q = field(&a, "face");
    let face = with_ents(i, &cx, &face_q, |_, q, e| Ok(e.first().and_then(|f| q.shape(f))))?;
    let Some(face) = face else { return Err(raise_error(i, "NO_TANGENT_PLANE", None)) };
    let planes = on_first(i, &cx, &a, "edge", "NO_TANGENT_PLANE", |q, e| {
        let s = q.shape(e)?;
        let info = s.edge_info().ok()?;
        params
            .iter()
            .map(|t| {
                let p = s.point_at(edge_param(&s, &info, *t, true)).ok()?;
                let n = face.normal_at_point(p).ok()?.normalize_or_zero();
                let x = any_perpendicular(n);
                Some(plane_value(&crate::display::PlaneFrame { origin: p, x, y: n.cross(x), normal: n }))
            })
            .collect()
    })?;
    Ok(Value::array(planes))
}

fn sum_measure(i: &mut Interp, a: &[Value], key: &str, want: EType) -> R<f64> {
    let cx = context_arg(i, a)?;
    with_ents(i, &cx, &field(a, key), |_, q, ents| {
        Ok(ents.iter().filter(|e| entity_type_of(e) == want).filter_map(|e| q.measure(e)).sum())
    })
}

fn ev_length(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    Ok(length_value(sum_measure(i, &a, "edges", EType::Edge)?))
}

fn ev_area(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    Ok(area_value(sum_measure(i, &a, "faces", EType::Face)?))
}

fn ev_volume(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    Ok(volume_value(sum_measure(i, &a, "bodies", EType::Body)?))
}

fn ev_box(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let tight = field(&a, "tight").as_bool().unwrap_or(true);
    let frame = coord_system_of(&field(&a, "cSys"));
    let r = with_ents(i, &cx, &field(&a, "topology"), |_, q, ents| {
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        for e in &ents {
            let pts: Vec<DVec3> = match q.shape(e) {
                Some(s) => {
                    let s = match frame {
                        Some((o, x, z)) => {
                            let y = z.cross(x);
                            let m = glam::DMat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), o.extend(1.0)).inverse();
                            match k::transform(&s, &k::Transform::Matrix(m), true) {
                                Ok(r) => r.shape,
                                Err(_) => continue,
                            }
                        }
                        None => s,
                    };
                    match s.bounding_box(tight) {
                        Ok(b) => vec![b.min, b.max],
                        Err(_) => continue,
                    }
                }
                None => q.point(e).into_iter().collect(),
            };
            for p in pts {
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        Ok((lo.is_finite() && hi.is_finite()).then_some((lo, hi)))
    })?;
    let Some((lo, hi)) = r else { return Err(raise_error(i, "CANNOT_COMPUTE_BBOX", None)) };
    Ok(Value::map_from([("minCorner", point_value(lo)), ("maxCorner", point_value(hi))]))
}

/// (dimension, measure, centroid, inertia about the centroid) of entities.
fn mass_props(q: &QueryCx, ents: &[Ent]) -> (u32, f64, DVec3, glam::DMat3, usize) {
    let dim = |e: &Ent| match entity_type_of(e) {
        EType::Body => match q.body_type(e) {
            "SOLID" => 3,
            "SHEET" => 2,
            "WIRE" => 1,
            _ => 0,
        },
        EType::Face => 2,
        EType::Edge => 1,
        EType::Vertex => 0,
    };
    let top = ents.iter().map(dim).max().unwrap_or(0);
    let mut total = 0.0;
    let mut moment = DVec3::ZERO;
    let mut inertia = glam::DMat3::ZERO;
    let mut parts = Vec::new();
    let mut count = 0;
    for e in ents.iter().filter(|e| dim(e) == top) {
        count += 1;
        let props = match (top, q.shape(e)) {
            (3, Some(s)) => k::volume_properties(&s).ok(),
            (2, Some(s)) => k::surface_properties(&s).ok(),
            (1, Some(s)) => k::linear_properties(&s).ok(),
            _ => q.point(e).map(|p| k::MassProperties { mass: 1.0, center: p, inertia: glam::DMat3::ZERO }),
        };
        if let Some(p) = props {
            total += p.mass;
            moment += p.center * p.mass;
            parts.push(p);
        }
    }
    let c = if total > 0.0 { moment / total } else { DVec3::ZERO };
    // Parallel axis theorem to the common centroid.
    for p in &parts {
        let d = p.center - c;
        let dd = d.dot(d);
        let shift = glam::DMat3::from_diagonal(DVec3::splat(dd)) - glam::DMat3::from_cols(d * d.x, d * d.y, d * d.z);
        inertia += p.inertia + shift * p.mass;
    }
    (top, total, c, inertia, count)
}

fn ev_centroid(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let c = with_ents(i, &cx, &field(&a, "entities"), |_, q, ents| Ok((!ents.is_empty()).then(|| mass_props(q, &ents).2)))?;
    match c {
        Some(c) => Ok(point_value(c)),
        None => Err(raise_error(i, "CANNOT_RESOLVE_ENTITIES", None)),
    }
}

fn ev_mass_properties(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let frame = coord_system_of(&field(&a, "referenceFrame"));
    let (dim, total, c, inertia, count) = with_ents(i, &cx, &field(&a, "entities"), |_, q, ents| Ok(mass_props(q, &ents)))?;
    let (centroid, inertia) = match frame {
        Some((o, x, z)) => {
            let y = z.cross(x);
            let r = glam::DMat3::from_cols(x, y, z).transpose();
            let d = c - o;
            let dd = d.dot(d);
            let shift = glam::DMat3::from_diagonal(DVec3::splat(dd)) - glam::DMat3::from_cols(d * d.x, d * d.y, d * d.z);
            (r * d, r * (inertia + shift * total) * r.transpose())
        }
        None => (c, inertia),
    };
    let row = |r: usize| Value::array((0..3).map(|col| Value::num(inertia.col(col)[r])).collect());
    let mut m = Map::new();
    m.insert(Value::str("highestDimension"), Value::num(dim as f64));
    let key = match dim {
        3 => "volume",
        2 => "area",
        1 => "length",
        _ => "count",
    };
    m.insert(Value::str(key), Value::num(if dim == 0 { count as f64 } else { total }));
    m.insert(Value::str("centroid"), Value::array(vec![Value::num(centroid.x), Value::num(centroid.y), Value::num(centroid.z)]));
    m.insert(Value::str("inertia"), Value::array(vec![row(0), row(1), row(2)]));
    Ok(Value::map(m))
}

/// One side of an evDistance: a query, a point, a Line or a Plane.
enum Side {
    Shapes(Vec<(usize, k::Shape)>),
    Point(DVec3),
}

fn side_of(i: &mut Interp, cx: &FsContext, v: &Value) -> R<Side> {
    if let Some(p) = vec3_of(v) {
        return Ok(Side::Point(p));
    }
    if let Some((o, d)) = line_of(v)
        && v.get("queryType").is_undefined()
    {
        let l = k::make_line(o - d * 1e3, o + d * 1e3).map_err(|e| i.error(e.to_string()))?;
        return Ok(Side::Shapes(vec![(0, l)]));
    }
    if let Some(p) = plane_of(v)
        && v.get("queryType").is_undefined()
    {
        let f = super::query::plane_face(&p).map_err(|e| i.error(e.to_string()))?;
        return Ok(Side::Shapes(vec![(0, f)]));
    }
    with_ents(i, cx, v, |_, q, ents| {
        let mut out = Vec::new();
        for (k_, e) in ents.iter().enumerate() {
            if let Some(s) = q.shape(e) {
                out.push((k_, s));
            } else if let Some(p) = q.plane(e) {
                if let Ok(f) = super::query::plane_face(&p) {
                    out.push((k_, f));
                }
            } else if let Some(p) = q.point(e)
                && let Ok(vx) = k::make_vertex(p)
            {
                out.push((k_, vx));
            }
        }
        Ok(Side::Shapes(out))
    })
}

fn param_on(s: &k::Shape, p: DVec3) -> Value {
    match s.shape_type() {
        ShapeType::Edge => {
            let Ok(info) = s.edge_info() else { return Value::num(0.0) };
            // Nearest sampled parameter, refined.
            let n = 200;
            let mut best = (0.0, f64::MAX);
            for j in 0..=n {
                let t = j as f64 / n as f64;
                if let Ok(x) = s.point_at(edge_param(s, &info, t, true)) {
                    let d = (x - p).length();
                    if d < best.1 {
                        best = (t, d);
                    }
                }
            }
            Value::num(best.0)
        }
        ShapeType::Face => match (s.project_point(p), s.uv_bounds()) {
            (Ok(pr), Ok(b)) => Value::array(vec![Value::num((pr.u - b[0]) / (b[1] - b[0]).max(1e-300)), Value::num((pr.v - b[2]) / (b[3] - b[2]).max(1e-300))]).with_tag(std_tag("vector.fs", "Vector")),
            _ => Value::Undefined,
        },
        _ => Value::num(0.0),
    }
}

fn ev_distance(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let s0 = side_of(i, &cx, &field(&a, "side0"))?;
    let s1 = side_of(i, &cx, &field(&a, "side1"))?;
    let maximum = field(&a, "maximum").as_bool().unwrap_or(false);
    let to_shapes = |s: &Side| -> Vec<(usize, k::Shape)> {
        match s {
            Side::Shapes(v) => v.clone(),
            Side::Point(p) => k::make_vertex(*p).map(|v| vec![(0, v)]).unwrap_or_default(),
        }
    };
    let (a0, a1) = (to_shapes(&s0), to_shapes(&s1));
    let mut best: Option<(f64, usize, DVec3, usize, DVec3)> = None;
    for (i0, x) in &a0 {
        for (i1, y) in &a1 {
            if maximum {
                // Maximum over vertices (exact for polyhedral input).
                for vx in x.vertices().iter().chain(std::iter::once(x)).filter_map(|v| v.vertex_point().ok()) {
                    for vy in y.vertices().iter().chain(std::iter::once(y)).filter_map(|v| v.vertex_point().ok()) {
                        let d = (vx - vy).length();
                        if best.is_none_or(|b| d > b.0) {
                            best = Some((d, *i0, vx, *i1, vy));
                        }
                    }
                }
                continue;
            }
            if let Ok(d) = k::distance(x, y)
                && best.is_none_or(|b| d.distance < b.0)
            {
                best = Some((d.distance, *i0, d.point_a, *i1, d.point_b));
            }
        }
    }
    let Some((d, i0, p0, i1, p1)) = best else { return Err(raise_error(i, "CANNOT_RESOLVE_ENTITIES", None)) };
    let side = |idx: usize, p: DVec3, shapes: &[(usize, k::Shape)], is_point: bool| -> Value {
        let s = shapes.iter().find(|x| x.0 == idx).map(|x| &x.1);
        let mut entries = vec![("point", point_value(p)), ("index", Value::num(idx as f64))];
        if !is_point && let Some(s) = s {
            entries.push(("parameter", param_on(s, p)));
        }
        Value::map_from(entries)
    };
    let sides = vec![side(i0, p0, &a0, matches!(s0, Side::Point(_))), side(i1, p1, &a1, matches!(s1, Side::Point(_)))];
    Ok(Value::map_from([("distance", length_value(d)), ("sides", Value::array(sides))]).with_tag(std_tag("evaluate.fs", "DistanceResult")))
}

fn ev_mate_connector(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let (o, x, z) = on_first(i, &cx, &a, "mateConnector", "CANNOT_RESOLVE_ENTITIES", |q, e| match e {
        Ent::Connector(c) => {
            let c = &q.st.ctx.connectors[*c];
            Some((c.origin, c.x, c.z))
        }
        _ => q.plane(e).map(|p| (p.origin, p.x, p.normal)),
    })?;
    Ok(coord_system_value(o, x, z))
}

fn ev_curve_definition(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let v = on_first(i, &cx, &a, "edge", "INVALID_INPUT", |q, e| {
        let s = q.shape(e)?;
        let info = s.edge_info().ok()?;
        Some(match info.curve {
            k::Curve::Line { direction, .. } => line_value(info.start, direction.normalize_or_zero()),
            k::Curve::Circle { center, axis, x_dir, radius } => Value::map_from([("coordSystem", coord_system_value(center, x_dir.normalize_or_zero(), axis.normalize_or_zero())), ("radius", length_value(radius))]).with_tag(std_tag("curveGeometry.fs", "Circle")),
            k::Curve::Ellipse { center, axis, x_dir, major_radius, minor_radius } => Value::map_from([
                ("coordSystem", coord_system_value(center, x_dir.normalize_or_zero(), axis.normalize_or_zero())),
                ("majorRadius", length_value(major_radius)),
                ("minorRadius", length_value(minor_radius)),
            ])
            .with_tag(std_tag("curveGeometry.fs", "Ellipse")),
            _ => bspline_value(&s.edge_bspline().ok()?),
        })
    })?;
    Ok(v)
}

/// A std `BSplineCurve` (knots expanded by multiplicity).
fn bspline_value(b: &k::BSplineCurveData) -> Value {
    let poles: Vec<Value> = b.poles.iter().map(|p| point_value(*p)).collect();
    let mut knots = Vec::new();
    for (kv, m) in b.knots.iter().zip(&b.mults) {
        for _ in 0..*m {
            knots.push(Value::num(*kv));
        }
    }
    let mut entries = vec![("degree", Value::num(b.degree as f64)), ("dimension", Value::num(3.0)), ("isRational", Value::bool(b.weights.is_some())), ("isPeriodic", Value::bool(b.periodic)), ("controlPoints", Value::array(poles)), ("knots", Value::array(knots).with_tag(std_tag("splineUtils.fs", "KnotArray")))];
    if let Some(w) = &b.weights {
        entries.push(("weights", Value::array(w.iter().map(|x| Value::num(*x)).collect())));
    }
    Value::map_from(entries).with_tag(std_tag("curveGeometry.fs", "BSplineCurve"))
}

/// The edge as a B-spline: exact when possible; with `forceCubic` /
/// `forceNonRational` that the exact form doesn't meet, a cubic
/// non-rational interpolation through samples of the edge.
fn ev_approximate_bspline_curve(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let cubic = field(&a, "forceCubic").as_bool().unwrap_or(false);
    let non_rational = field(&a, "forceNonRational").as_bool().unwrap_or(false);
    on_first(i, &cx, &a, "edge", "INVALID_INPUT", |q, e| {
        let s = q.shape(e)?;
        let exact = s.edge_bspline().ok()?;
        if (!cubic || exact.degree == 3) && (!non_rational || exact.weights.is_none()) {
            return Some(bspline_value(&exact));
        }
        let info = s.edge_info().ok()?;
        let n = (exact.poles.len() * 4).clamp(16, 200);
        let pts: Vec<DVec3> = (0..=n).filter_map(|j| s.point_at(info.first_param + (info.last_param - info.first_param) * j as f64 / n as f64).ok()).collect();
        let fit = k::make_interpolated_spline(&pts, &[], false).ok()?;
        Some(bspline_value(&fit.edges().first()?.edge_bspline().ok()?))
    })
}

fn surface_type(name: &str) -> Value {
    std_enum("surfacetype.gen.fs", "SurfaceType", name).unwrap_or_else(|| Value::str(name))
}

/// Add `surfaceType` (what the standard library reads) to a definition map.
fn with_surface_type(v: Value, name: &str) -> Value {
    let tag = v.tag();
    match v {
        Value::Map(m, _) => {
            let mut m = (*m).clone();
            m.insert(Value::str("surfaceType"), surface_type(name));
            Value::Map(std::sync::Arc::new(m), tag)
        }
        other => other,
    }
}

fn ev_surface_definition(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let v = on_first(i, &cx, &a, "face", "CANNOT_RESOLVE_PLANE", |q, e| {
        if let Some(p) = matches!(e, Ent::Datum(_) | Ent::Plane(_) | Ent::SketchRegion(..)).then(|| q.plane(e)).flatten() {
            return Some(with_surface_type(plane_value(&p), "PLANE"));
        }
        let s = q.shape(e)?;
        let cs = |o: DVec3, x: DVec3, z: DVec3| coord_system_value(o, x.normalize_or_zero(), z.normalize_or_zero());
        Some(match s.face_surface().ok()? {
            k::Surface::Plane(_) => with_surface_type(plane_value(&q.plane(e)?), "PLANE"),
            k::Surface::Cylinder { origin, axis, x_dir, radius } => with_surface_type(Value::map_from([("coordSystem", cs(origin, x_dir, axis)), ("radius", length_value(radius))]).with_tag(std_tag("surfaceGeometry.fs", "Cylinder")), "CYLINDER"),
            k::Surface::Cone { apex, axis, x_dir, half_angle, .. } => with_surface_type(Value::map_from([("coordSystem", cs(apex, x_dir, axis)), ("halfAngle", angle_value(half_angle))]).with_tag(std_tag("surfaceGeometry.fs", "Cone")), "CONE"),
            k::Surface::Sphere { center, axis, radius } => with_surface_type(Value::map_from([("coordSystem", cs(center, any_perpendicular(axis.normalize_or_zero()), axis)), ("radius", length_value(radius))]).with_tag(std_tag("surfaceGeometry.fs", "Sphere")), "SPHERE"),
            k::Surface::Torus { center, axis, major_radius, minor_radius } => with_surface_type(
                Value::map_from([("coordSystem", cs(center, any_perpendicular(axis.normalize_or_zero()), axis)), ("radius", length_value(major_radius)), ("minorRadius", length_value(minor_radius))]).with_tag(std_tag("surfaceGeometry.fs", "Torus")),
                "TORUS",
            ),
            k::Surface::Revolution { .. } => Value::map_from([("surfaceType", surface_type("REVOLVED"))]),
            k::Surface::Extrusion { .. } => Value::map_from([("surfaceType", surface_type("EXTRUDED"))]),
            k::Surface::BSpline { .. } | k::Surface::Bezier { .. } => Value::map_from([("surfaceType", surface_type("SPLINE"))]),
            _ => Value::map_from([("surfaceType", surface_type("OTHER"))]),
        })
    })?;
    Ok(v)
}

/// Convexity of an edge between two faces.
fn convexity(q: &QueryCx, e: &Ent) -> Option<&'static str> {
    let Ent::Edge(b, x) = e else { return None };
    let shape = &q.st.ctx.bodies[*b].shape;
    let ef = shape.edge_faces().ok()?;
    let faces = ef.get(*x)?;
    if faces.len() != 2 {
        return None;
    }
    let edge = q.shape(e)?;
    let info = edge.edge_info().ok()?;
    let fa = shape.sub_shape(ShapeType::Face, faces[0] as usize).ok()?;
    let fb = shape.sub_shape(ShapeType::Face, faces[1] as usize).ok()?;
    let mut kinds = Vec::new();
    for t in [0.2, 0.5, 0.8] {
        let p = edge.point_at(info.first_param + (info.last_param - info.first_param) * t).ok()?;
        let (na, nb) = (fa.normal_at_point(p).ok()?.normalize_or_zero(), fb.normal_at_point(p).ok()?.normalize_or_zero());
        if na.dot(nb) > 1.0 - 1e-6 {
            kinds.push("SMOOTH");
            continue;
        }
        // Direction into face b from the edge, tangent to b.
        let pb = fb.project_point(p + (fb.centroid().ok()? - p) * 1e-3).ok()?.point;
        let into_b = (pb - p).normalize_or_zero();
        kinds.push(if into_b.dot(na) < 0.0 { "CONVEX" } else { "CONCAVE" });
    }
    kinds.dedup();
    Some(if kinds.len() == 1 { kinds[0] } else { "VARIABLE" })
}

fn ev_edge_convexity(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let c = on_first(i, &cx, &a, "edge", "INVALID_INPUT", convexity)?;
    Ok(std_enum("edgeconvexitytype.gen.fs", "EdgeConvexityType", c).unwrap_or_else(|| Value::str(c)))
}

fn edge_plane(q: &QueryCx, e: &Ent) -> Option<crate::display::PlaneFrame> {
    if let Ent::SketchEdge(s, _) = e {
        return Some(q.st.ctx.sketches[*s].frame);
    }
    let s = q.shape(e)?;
    let info = s.edge_info().ok()?;
    match info.curve {
        k::Curve::Circle { center, axis, x_dir, .. } | k::Curve::Ellipse { center, axis, x_dir, .. } => {
            let n = axis.normalize_or_zero();
            let x = x_dir.normalize_or_zero();
            Some(crate::display::PlaneFrame { origin: center, x, y: n.cross(x), normal: n })
        }
        _ => {
            // Fit a plane through samples.
            let pts: Vec<DVec3> = (0..=8).filter_map(|j| s.point_at(info.first_param + (info.last_param - info.first_param) * j as f64 / 8.0).ok()).collect();
            let o = pts[0];
            let mut n = DVec3::ZERO;
            for w in pts.windows(2) {
                n += (w[0] - o).cross(w[1] - o);
            }
            let n = n.normalize_or_zero();
            if n == DVec3::ZERO || pts.iter().any(|p| (*p - o).dot(n).abs() > 1e-8) {
                return None;
            }
            let x = (pts[pts.len() - 1] - o).normalize_or_zero();
            Some(crate::display::PlaneFrame { origin: o, x, y: n.cross(x), normal: n })
        }
    }
}

fn ev_planar_edge(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let p = on_first(i, &cx, &a, "edge", "CANNOT_RESOLVE_PLANE", edge_plane)?;
    Ok(plane_value(&p))
}

fn ev_planar_edges(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let p = on_first(i, &cx, &a, "edges", "CANNOT_RESOLVE_PLANE", edge_plane)?;
    Ok(plane_value(&p))
}

fn ev_fillet_radius(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let r = on_first(i, &cx, &a, "face", "INVALID_INPUT", |q, e| match q.shape(e)?.face_surface().ok()? {
        k::Surface::Cylinder { radius, .. } | k::Surface::Sphere { radius, .. } => Some(radius),
        k::Surface::Torus { minor_radius, .. } => Some(minor_radius),
        _ => None,
    })?;
    Ok(length_value(r))
}

fn ev_edge_curvatures(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let params: Vec<f64> = field(&a, "parameters").as_array().map(|v| v.iter().filter_map(|x| x.as_number()).collect()).unwrap_or_default();
    let out = on_first(i, &cx, &a, "edge", "INVALID_INPUT", |q, e| {
        let s = q.shape(e)?;
        let info = s.edge_info().ok()?;
        let mut out = Vec::new();
        for t in &params {
            let u = edge_param(&s, &info, *t, true);
            let h = (info.last_param - info.first_param) * 1e-4;
            let p = s.point_at(u).ok()?;
            let t0 = s.tangent_at(u).ok()?.normalize_or_zero();
            let t1 = s.tangent_at(u + h).ok().unwrap_or(t0).normalize_or_zero();
            let ds = (s.point_at(u + h).ok().unwrap_or(p) - p).length().max(1e-300);
            let dt = (t1 - t0) / ds;
            let curv = dt.length();
            let nrm = if curv > 1e-12 { dt / curv } else { any_perpendicular(t0) };
            let b = t0.cross(nrm);
            let frame = coord_system_value(p, t0, b);
            out.push(Value::map_from([("frame", frame), ("curvature", Value::num(curv).with_tag(noshape_featurescript::value::Tag::NONE))]));
        }
        Some(out)
    })?;
    Ok(Value::array(out))
}

fn ev_face_curvatures(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let params: Vec<DVec2> = field(&a, "parameters").as_array().map(|v| v.iter().filter_map(vec2_of).collect()).unwrap_or_default();
    let out = on_first(i, &cx, &a, "face", "INVALID_INPUT", |q, e| {
        let s = q.shape(e)?;
        let surf = s.face_surface().ok()?;
        let mut out = Vec::new();
        for uv in &params {
            let pl = face_tangent_plane(&s, *uv)?;
            let (k1, k2) = match &surf {
                k::Surface::Plane(_) => (0.0, 0.0),
                k::Surface::Cylinder { radius, .. } => (1.0 / radius, 0.0),
                k::Surface::Sphere { radius, .. } => (1.0 / radius, 1.0 / radius),
                _ => (0.0, 0.0),
            };
            out.push(Value::map_from([("minCurvature", Value::num(k2.min(k1))), ("maxCurvature", Value::num(k1.max(k2))), ("minDirection", dir_value(pl.y)), ("maxDirection", dir_value(pl.x))]));
        }
        Some(out)
    })?;
    Ok(Value::array(out))
}

fn ev_raycast(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let Some((o, d)) = line_of(&field(&a, "ray")) else { return Err(i.error("evRaycast: ray should be a Line")) };
    let include_intersections = field(&a, "closest").as_bool().map(|c| !c).unwrap_or(false);
    let hits = with_ents(i, &cx, &field(&a, "entities"), |_, q, ents| {
        let ray = k::make_line(o, o + d * 1e4).ok();
        let mut hits = Vec::new();
        for e in &ents {
            let (Some(s), Some(r)) = (q.shape(e), &ray) else { continue };
            for f in if s.shape_type() == ShapeType::Face { vec![s.clone()] } else { s.faces() } {
                if let Ok(dist) = k::distance(&f, r)
                    && dist.distance < 1e-8
                {
                    let t = (dist.point_b - o).dot(d);
                    hits.push((t, q.to_ref(e), dist.point_b));
                }
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        Ok(hits)
    })?;
    let take = if include_intersections { hits.len() } else { hits.len().min(1) };
    let mut out = Vec::new();
    for (t, r, p) in hits.into_iter().take(take) {
        let q = super::query::query_of_refs(&cx, &[r]);
        out.push(Value::map_from([("entity", q), ("intersection", point_value(p)), ("distance", length_value(t))]));
    }
    Ok(Value::array(out))
}

fn ev_collision_detection(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let tools = super::query::evaluate_refs(i, &cx, &field(&a, "tools"))?;
    let targets = super::query::evaluate_refs(i, &cx, &field(&a, "targets"))?;
    let shapes = |refs: &[crate::reference::Reference]| -> Vec<(crate::reference::Reference, k::Shape)> {
        let st = cx.state.lock();
        let q = QueryCx::new(&st);
        refs.iter().flat_map(|r| q.from_ref(r).into_iter().filter_map(|e| q.shape(&e)).map(move |s| (r.clone(), s))).collect()
    };
    let (ts, gs) = (shapes(&tools), shapes(&targets));
    let mut out = Vec::new();
    for (tr, t) in &ts {
        for (gr, g) in &gs {
            if t.is_same(g) {
                continue;
            }
            let Ok(d) = k::distance(t, g) else { continue };
            if d.distance > 1e-8 {
                continue;
            }
            let kind = match k::common(t, g).ok().and_then(|c| c.shape.volume().ok()) {
                Some(v) if v > 1e-15 => "INTERFERE",
                _ => "ABUT_NO_CLASS",
            };
            let ty = std_enum("clashtype.gen.fs", "ClashType", kind).unwrap_or_else(|| Value::str(kind));
            out.push(Value::map_from([("type", ty), ("tool", super::query::query_of_refs(&cx, std::slice::from_ref(tr))), ("target", super::query::query_of_refs(&cx, std::slice::from_ref(gr)))]));
        }
    }
    Ok(Value::array(out))
}

fn ev_face_periodicity(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let r = on_first(i, &cx, &a, "face", "INVALID_INPUT", |q, e| {
        Some(match q.shape(e)?.face_surface().ok()? {
            k::Surface::Cylinder { .. } | k::Surface::Cone { .. } => [true, false],
            k::Surface::Sphere { .. } => [true, false],
            k::Surface::Torus { .. } => [true, true],
            k::Surface::BSpline { u_periodic, v_periodic, .. } => [u_periodic, v_periodic],
            _ => [false, false],
        })
    })?;
    Ok(Value::array(vec![Value::bool(r[0]), Value::bool(r[1])]))
}

/// std topologyUtils clusterBodies: groups of bodies with identical topology
/// and geometry up to a rigid motion (not reflection), as indices into the
/// evaluated `bodies`. Compared by entity counts, the measure (volume, area
/// or length) and the inertia tensor's rotation invariants, each within
/// `relativeTolerance`.
fn cluster_bodies(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let tol = field(&a, "relativeTolerance").as_number().unwrap_or(1e-6);
    let signatures = with_ents(i, &cx, &field(&a, "bodies"), |_, q, ents| {
        Ok(ents
            .iter()
            .map(|e| {
                let s = q.shape(e)?;
                let props = if !s.solids().is_empty() {
                    k::volume_properties(&s)
                } else if !s.faces().is_empty() {
                    k::surface_properties(&s)
                } else {
                    k::linear_properties(&s)
                }
                .ok()?;
                let m = props.inertia;
                let tr = m.x_axis.x + m.y_axis.y + m.z_axis.z;
                let tr2 = (m * m).x_axis.x + (m * m).y_axis.y + (m * m).z_axis.z;
                let counts = [s.count(ShapeType::Face), s.count(ShapeType::Edge), s.count(ShapeType::Vertex)];
                Some((counts, [props.mass, tr, 0.5 * (tr * tr - tr2), m.determinant()]))
            })
            .collect::<Vec<_>>())
    })?;
    let same = |x: f64, y: f64| (x - y).abs() <= tol * x.abs().max(y.abs()) + 1e-30;
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    for (k, sig) in signatures.iter().enumerate() {
        let found = sig.as_ref().and_then(|(c, v)| {
            clusters.iter().position(|cl| match &signatures[cl[0]] {
                Some((c0, v0)) => c0 == c && v0.iter().zip(v).all(|(x, y)| same(*x, *y)),
                None => false,
            })
        });
        match found {
            Some(ci) => clusters[ci].push(k),
            None => clusters.push(vec![k]),
        }
    }
    Ok(Value::array(clusters.into_iter().map(|cl| Value::array(cl.into_iter().map(|k| Value::num(k as f64)).collect())).collect()))
}
