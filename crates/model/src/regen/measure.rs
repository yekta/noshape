//! Measurement and mass properties for selections (Onshape's measure tool
//! and mass properties panel).

use super::context::Context;
use crate::reference::Reference;
use glam::DVec3;
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};

#[derive(Clone, Debug, PartialEq)]
pub enum Measurement {
    Length(f64),
    Area(f64),
    Radius(f64),
    Diameter(f64),
    /// Minimum distance with the closest points.
    Distance { distance: f64, a: DVec3, b: DVec3, dx: f64, dy: f64, dz: f64 },
    Angle(f64),
    /// A percentage (Curvature deviation).
    Percent(f64),
    Point(DVec3),
    Volume(f64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MassProperties {
    pub volume: f64,
    pub surface_area: f64,
    /// kg/m^3 if a density is known.
    pub density: Option<f64>,
    pub mass: Option<f64>,
    pub center_of_mass: DVec3,
    /// Principal moments (per unit density if no density).
    pub inertia: glam::DMat3,
}

/// Resolve one reference to kernel shapes (sub-shapes or whole bodies).
pub fn resolve_shapes(ctx: &Context, r: &Reference) -> Vec<Shape> {
    let mut out = Vec::new();
    match r {
        Reference::Face(n) => {
            for b in &ctx.bodies {
                for i in b.find_faces(n) {
                    if let Ok(s) = b.shape.sub_shape(ShapeType::Face, i) {
                        out.push(s);
                    }
                }
            }
        }
        Reference::Edge(n) => {
            for b in &ctx.bodies {
                for i in b.find_edges(n) {
                    if let Ok(s) = b.shape.sub_shape(ShapeType::Edge, i) {
                        out.push(s);
                    }
                }
            }
        }
        Reference::Vertex(n) => {
            for b in &ctx.bodies {
                for i in b.find_vertices(n) {
                    if let Ok(s) = b.shape.sub_shape(ShapeType::Vertex, i) {
                        out.push(s);
                    }
                }
            }
        }
        Reference::Body(id) => {
            if let Some(b) = ctx.body(id) {
                out.push(b.shape.clone());
            }
        }
        Reference::Origin => out.push(k::make_vertex(DVec3::ZERO).ok().into_iter().next().unwrap_or_else(Shape::null)),
        // Default planes and plane features measure as their (unbounded)
        // plane: a large planar face (qa5 AN1: vertex / edge + Top plane).
        Reference::Plane(d) => {
            let f = match d {
                crate::reference::DatumPlane::Top => crate::display::PlaneFrame::TOP,
                crate::reference::DatumPlane::Front => crate::display::PlaneFrame::FRONT,
                crate::reference::DatumPlane::Right => crate::display::PlaneFrame::RIGHT,
            };
            out.extend(plane_face(&f));
        }
        Reference::FeaturePlane(_) => {
            if let Some(p) = ctx.planes.iter().find(|p| &p.reference == r) {
                out.extend(plane_face(&p.frame));
            }
        }
        Reference::SketchEntity { sketch, entity } => {
            if let Some(s) = ctx.sketch(*sketch) {
                let id = noshape_sketch::EntityId(*entity);
                if let Some(p) = s.sketch.point(id) {
                    if let Ok(v) = k::make_vertex(s.frame.to_world(p)) {
                        out.push(v);
                    }
                } else if let Some(c) = s.sketch.curve(id)
                    && let Some(seg) = super::features::extrude::curve_to_segment(&c)
                    && let Ok(e) = k::make_edge_2d(&super::eval::kernel_plane(&s.frame), &seg)
                {
                    out.push(e);
                }
            }
        }
        // Edge midpoints (Onshape lets you pick them to measure from).
        Reference::Implicit { on, inference: crate::reference::Inference::Midpoint } => {
            for e in resolve_shapes(ctx, on) {
                if let Ok(info) = e.edge_info()
                    && let Ok(p) = e.point_at((info.first_param + info.last_param) * 0.5)
                    && let Ok(v) = k::make_vertex(p)
                {
                    out.push(v);
                }
            }
        }
        _ => {}
    }
    out.retain(|s| !s.is_null());
    out
}

/// A plane as a face for measuring: 2 km square, centred on its origin.
fn plane_face(f: &crate::display::PlaneFrame) -> Option<Shape> {
    k::make_plane_face(&super::eval::kernel_plane(f), -1e3, 1e3, -1e3, 1e3).ok()
}

/// Measurements for a selection (single entity properties, or pairwise
/// distance/angle for two entities).
pub fn measure(ctx: &Context, refs: &[Reference]) -> Vec<(String, Measurement)> {
    let shapes: Vec<Vec<Shape>> = refs.iter().map(|r| resolve_shapes(ctx, r)).collect();
    measure_shapes(&shapes)
}

/// The geometry a pair measurement looks at.
#[derive(Clone, Copy)]
enum Geo {
    Plane { origin: DVec3, normal: DVec3 },
    /// A cylinder's axis.
    Axis { origin: DVec3, dir: DVec3 },
    Line { origin: DVec3, dir: DVec3 },
    /// A circle's or sphere's center.
    Center(DVec3),
    Other,
}

fn geo(s: &Shape) -> Geo {
    match s.shape_type() {
        ShapeType::Face => match s.face_surface() {
            Ok(k::Surface::Plane(p)) => Geo::Plane { origin: p.origin, normal: p.normal.normalize() },
            Ok(k::Surface::Cylinder { origin, axis, .. }) => Geo::Axis { origin, dir: axis.normalize() },
            Ok(k::Surface::Sphere { center, .. }) => Geo::Center(center),
            _ => Geo::Other,
        },
        ShapeType::Edge => match s.edge_curve() {
            Ok(k::Curve::Line { origin, direction }) => Geo::Line { origin, dir: direction.normalize() },
            Ok(k::Curve::Circle { center, .. }) => Geo::Center(center),
            _ => Geo::Other,
        },
        _ => Geo::Other,
    }
}

fn parallel(a: DVec3, b: DVec3) -> bool {
    a.cross(b).length() < 1e-9
}

/// A distance row with its signed components (second minus first).
fn dist_row(label: &str, a: DVec3, b: DVec3) -> (String, Measurement) {
    let v = b - a;
    (label.into(), Measurement::Distance { distance: v.length(), a, b, dx: v.x, dy: v.y, dz: v.z })
}

/// The minimum distance between two shapes, as a row.
fn min_row(a: &Shape, b: &Shape) -> Option<(String, Measurement)> {
    let d = k::distance(a, b).ok()?;
    Some(dist_row("Min dist", d.point_a, d.point_b))
}

/// The maximum distance between two shapes (over points sampled on both),
/// as a row.
fn max_row(a: &Shape, b: &Shape) -> Option<(String, Measurement)> {
    let (pa, pb) = (sample_points(std::slice::from_ref(a)), sample_points(std::slice::from_ref(b)));
    let mut best: Option<(f64, DVec3, DVec3)> = None;
    for p in &pa {
        for q in &pb {
            let d = q.distance(*p);
            if best.is_none_or(|b| d > b.0) {
                best = Some((d, *p, *q));
            }
        }
    }
    best.map(|(_, p, q)| dist_row("Max dist", p, q))
}

fn is_right_angle(t: f64) -> bool {
    (t - std::f64::consts::FRAC_PI_2).abs() < 1e-9
}

/// Rows that join the first one on the status-bar line in a circular
/// context (bundle `isDistanceQuickMeasureInCircularContextType`).
const LINE_DISTANCES: [&str; 6] = ["Min dist", "Max dist", "Center dist", "Parallel axes dist", "Perp axes dist", "Center axes dist"];

/// Rows that make a circular context (a cylinder, circle or sphere is measured).
const CIRCULAR: [&str; 5] = ["Parallel axes dist", "Perp axes angle", "Center axes dist", "Center dist", "Diameter"];

/// The status-bar quick readout of `rows` measured on `count` entities
/// (bundle `getPrimaryMeasurements`): the first row that isn't secondary
/// (the Area / Length totals of a multi-selection are), then, when a
/// cylinder / circle is measured, the following distance rows on the same
/// line (live: "Parallel axes dist: 30.000 mm  Min dist: 22.000 mm  Max
/// dist: 42.942 mm"). Three edges show nothing.
pub fn quick_line(rows: &[(String, Measurement)], count: usize) -> Vec<(String, Measurement)> {
    let secondary = |l: &str| count > 1 && (l == "Area" || l == "Length");
    let Some(first) = rows.iter().position(|(l, _)| !secondary(l)) else { return Vec::new() };
    let mut out = vec![rows[first].clone()];
    if rows.iter().any(|(l, _)| CIRCULAR.contains(&l.as_str())) {
        out.extend(rows[first + 1..].iter().filter(|(l, _)| LINE_DISTANCES.contains(&l.as_str())).cloned());
    }
    out
}

/// Measurements of resolved shapes, one list per selected entity, in the
/// order of Onshape's Measure panel (live psb 10-20 m,
/// dom/measure-readouts-final.txt); [`quick_line`] picks the status-bar
/// readout from them. Pairs use the first shape of each entity; with more
/// entities the first two give the pair rows and faces / edges add the
/// "Area" / "Length" totals.
pub fn measure_shapes(shapes: &[Vec<Shape>]) -> Vec<(String, Measurement)> {
    let mut out: Vec<(String, Measurement)> = Vec::new();
    let flat: Vec<&Shape> = shapes.iter().flatten().collect();
    // Single entity.
    if shapes.len() == 1 {
        for s in &shapes[0] {
            match s.shape_type() {
                ShapeType::Edge => match s.edge_curve() {
                    // Live: Diameter | Center | Length.
                    Ok(k::Curve::Circle { center, radius, .. }) => {
                        out.push(("Diameter".into(), Measurement::Diameter(radius * 2.0)));
                        out.push(("Center".into(), Measurement::Point(center)));
                        if let Ok(l) = s.length() {
                            out.push(("Length".into(), Measurement::Length(l)));
                        }
                    }
                    _ => {
                        if let Ok(l) = s.length() {
                            out.push(("Length".into(), Measurement::Length(l)));
                        }
                    }
                },
                ShapeType::Face => {
                    // Live: Diameter | Area | Radius (cylinder); Area.
                    let round = match s.face_surface() {
                        Ok(k::Surface::Cylinder { radius, .. }) | Ok(k::Surface::Sphere { radius, .. }) => Some(radius),
                        _ => None,
                    };
                    if let Some(r) = round {
                        out.push(("Diameter".into(), Measurement::Diameter(r * 2.0)));
                    }
                    if let Ok(a) = s.area() {
                        out.push(("Area".into(), Measurement::Area(a)));
                    }
                    if let Some(r) = round {
                        out.push(("Radius".into(), Measurement::Radius(r)));
                    }
                }
                ShapeType::Vertex => {
                    if let Ok(p) = s.vertex_point() {
                        out.push(("Point".into(), Measurement::Point(p)));
                    }
                }
                ShapeType::Solid | ShapeType::Compound | ShapeType::CompSolid => {
                    if let Ok(v) = s.volume() {
                        out.push(("Volume".into(), Measurement::Volume(v)));
                    }
                }
                _ => {}
            }
        }
        return out;
    }
    let all_edges = flat.len() >= 2 && flat.iter().all(|s| s.shape_type() == ShapeType::Edge);
    let all_faces = flat.len() >= 2 && flat.iter().all(|s| s.shape_type() == ShapeType::Face);
    let area = || all_faces.then(|| ("Area".to_string(), Measurement::Area(flat.iter().filter_map(|s| s.area().ok()).sum()))).into_iter();
    // Totals over the faces / edges of mixed pairs (an edge + a face lists both).
    let face_area = || {
        let faces: Vec<&&Shape> = flat.iter().filter(|s| s.shape_type() == ShapeType::Face).collect();
        (!faces.is_empty()).then(|| ("Area".to_string(), Measurement::Area(faces.iter().filter_map(|s| s.area().ok()).sum()))).into_iter()
    };
    let edge_length = || {
        let edges: Vec<&&Shape> = flat.iter().filter(|s| s.shape_type() == ShapeType::Edge).collect();
        (!edges.is_empty()).then(|| ("Length".to_string(), Measurement::Length(edges.iter().filter_map(|s| s.length().ok()).sum()))).into_iter()
    };
    // Live: three or more edges list only their total length.
    if shapes.len() > 2 && all_edges {
        out.extend(edge_length());
        return out;
    }
    // (No selection, e.g. the Measure tool opened with nothing selected.)
    let (Some(a), Some(b)) = (shapes.first().and_then(|s| s.first()), shapes.get(1).and_then(|s| s.first())) else { return out };
    let min = || min_row(a, b).into_iter();
    let max = || max_row(a, b).into_iter();
    let angle = k::angle_between(a, b).ok();
    let point = |s: &Shape| (s.shape_type() == ShapeType::Vertex).then(|| s.vertex_point().ok()).flatten();
    match (geo(a), geo(b)) {
        // (1) / (2) planes.
        (Geo::Plane { origin: o1, normal: n1 }, Geo::Plane { origin: o2, normal: n2 }) => {
            if parallel(n1, n2) {
                let d = n1 * (o2 - o1).dot(n1);
                out.push(dist_row("Parallel dist", DVec3::ZERO, d));
                out.extend(area());
                out.extend(min());
                out.extend(max());
            } else {
                // The angle between the faces (live: a 45° chamfer against the top reads 135°).
                let t = std::f64::consts::PI - n1.angle_between(n2);
                out.push((if is_right_angle(t) { "Perp planes angle" } else { "Angle" }.into(), Measurement::Angle(t)));
                out.extend(area());
                out.extend(max());
                out.extend(min());
            }
        }
        // (3) / (4) straight edges.
        (Geo::Line { origin: o1, dir: d1 }, Geo::Line { origin: o2, dir: d2 }) => {
            if parallel(d1, d2) {
                let v = o2 - o1;
                out.push(dist_row("Parallel dist", DVec3::ZERO, v - d1 * v.dot(d1)));
                out.extend(max());
                out.extend(min());
            } else {
                let t = angle.unwrap_or_else(|| d1.angle_between(d2));
                out.push((if is_right_angle(t) { "Perp lines angle" } else { "Angle" }.into(), Measurement::Angle(t)));
                out.extend(min());
                // Skew lines: the distance along their common perpendicular.
                let n = d1.cross(d2).normalize();
                let perp = n * (o2 - o1).dot(n);
                if perp.length() > 1e-9 {
                    out.push(dist_row("Perp dist", DVec3::ZERO, perp));
                }
                out.extend(max());
            }
            out.extend(edge_length());
        }
        // (5) a straight edge and a planar face.
        (Geo::Line { origin: o, dir: d }, Geo::Plane { origin: p, normal: n }) | (Geo::Plane { origin: p, normal: n }, Geo::Line { origin: o, dir: d }) => {
            if d.dot(n).abs() < 1e-9 {
                // Parallel (not captured live): their distance first.
                out.push(dist_row("Parallel dist", DVec3::ZERO, n * (o - p).dot(n)));
                out.extend(min());
                out.extend(face_area());
                out.extend(max());
            } else {
                let t = angle.unwrap_or_else(|| std::f64::consts::FRAC_PI_2 - d.angle_between(n).min(std::f64::consts::PI - d.angle_between(n)));
                if is_right_angle(t) {
                    out.push(("Perp line and plane angle".into(), Measurement::Angle(t)));
                    out.extend(min());
                    out.extend(face_area());
                    out.extend(max());
                } else {
                    out.push(("Angle".into(), Measurement::Angle(t)));
                    // Live: "Curvature deviation: 0.00 %" (a line against a plane).
                    out.push(("Curvature deviation".into(), Measurement::Percent(0.0)));
                    out.extend(face_area());
                    out.extend(max());
                    out.extend(min());
                }
            }
            out.extend(edge_length());
        }
        // (6) two cylinders.
        (Geo::Axis { origin: o1, dir: d1 }, Geo::Axis { origin: o2, dir: d2 }) => {
            if parallel(d1, d2) {
                let v = o2 - o1;
                out.push(dist_row("Parallel axes dist", DVec3::ZERO, v - d1 * v.dot(d1)));
                out.extend(area());
                out.extend(min());
                out.extend(max());
            } else {
                let t = d1.angle_between(d2);
                let t = t.min(std::f64::consts::PI - t);
                out.push((if is_right_angle(t) { "Perp axes angle" } else { "Angle" }.into(), Measurement::Angle(t)));
                out.extend(area());
                let n = d1.cross(d2).normalize();
                out.push(dist_row("Perp axes dist", DVec3::ZERO, n * (o2 - o1).dot(n)));
                out.extend(max());
                out.extend(min());
            }
        }
        // A cylinder and a plane parallel to its axis: "Center axes dist".
        (Geo::Axis { origin: o, dir: d }, Geo::Plane { origin: p, normal: n }) | (Geo::Plane { origin: p, normal: n }, Geo::Axis { origin: o, dir: d }) if d.dot(n).abs() < 1e-9 => {
            out.extend(min());
            out.extend(area());
            out.push(dist_row("Center axes dist", DVec3::ZERO, n * (p - o).dot(n)));
            out.extend(max());
        }
        // (7) two circles / spheres.
        (Geo::Center(c1), Geo::Center(c2)) => {
            out.push(dist_row("Center dist", c1, c2));
            out.extend(max());
            out.extend(min());
            out.extend(edge_length());
            out.extend(area());
        }
        // (8) a vertex and a circle.
        (Geo::Center(c), Geo::Other) | (Geo::Other, Geo::Center(c)) if point(a).or(point(b)).is_some() => {
            let p = point(a).or(point(b)).unwrap_or_default();
            out.extend(min());
            out.extend(max());
            out.push(if point(a).is_some() { dist_row("Center dist", p, c) } else { dist_row("Center dist", c, p) });
            out.extend(edge_length());
            out.extend(face_area());
        }
        // Anything else (vertex + face / edge / vertex, curved faces):
        // Min dist | Area | Max dist | Length; two vertices only Min dist.
        _ => {
            out.extend(min());
            if !(point(a).is_some() && point(b).is_some()) {
                out.extend(face_area());
                out.extend(max());
                out.extend(edge_length());
            }
        }
    }
    out
}

/// Mass properties of whole bodies (selected parts, or all parts).
pub fn mass_properties(ctx: &Context, bodies: &[crate::reference::BodyId], densities: &dyn Fn(&crate::reference::BodyId) -> Option<f64>) -> Option<MassProperties> {
    let mut volume = 0.0;
    let mut area = 0.0;
    let mut mass = 0.0;
    let mut have_density = true;
    let mut com = DVec3::ZERO;
    // (weight, own center, inertia about the own center, weighted).
    let mut parts: Vec<(f64, DVec3, glam::DMat3)> = Vec::new();
    // Only parts (solids) have mass properties: Onshape's dialog can't
    // measure a surface, wire or point (live qa4 round 4 s3: an open loft
    // surface leaves Parts to measure, Volume and Surface area blank).
    for b in ctx.bodies.iter().filter(|b| b.kind == crate::display::BodyKind::Solid && (bodies.is_empty() || bodies.contains(&b.id))) {
        let Ok(p) = k::volume_properties(&b.shape) else { continue };
        let Ok(s) = k::surface_properties(&b.shape) else { continue };
        let rho = densities(&b.id);
        if rho.is_none() {
            have_density = false;
        }
        let w = p.mass * rho.unwrap_or(1.0);
        volume += p.mass;
        area += s.mass;
        mass += w;
        com += p.center * w;
        parts.push((w, p.center, p.inertia * rho.unwrap_or(1.0)));
    }
    if parts.is_empty() {
        return None;
    }
    let total = if mass > 0.0 { mass } else { 1.0 };
    let com = com / total;
    Some(MassProperties {
        volume,
        surface_area: area,
        density: None,
        mass: have_density.then_some(mass),
        center_of_mass: com,
        inertia: combined_inertia(&parts, com),
    })
}

/// Inertia tensors of parts (each about its own center of mass, the
/// kernel's convention: off-diagonals are the negated products) moved to
/// the common center `com` by the parallel-axis theorem and summed:
/// `I + m (|d|² E − d dᵀ)`, `d` = part center − `com`.
pub fn combined_inertia(parts: &[(f64, DVec3, glam::DMat3)], com: DVec3) -> glam::DMat3 {
    let mut out = glam::DMat3::ZERO;
    for &(m, c, i) in parts {
        let d = c - com;
        let outer = glam::DMat3::from_cols(d * d.x, d * d.y, d * d.z);
        out += i + (glam::DMat3::IDENTITY * d.length_squared() - outer) * m;
    }
    out
}

/// Distance between two entities for a Measured variable (variable.fs):
/// the length and the vector from the first entity to the second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityDistance {
    pub distance: f64,
    pub delta: glam::DVec3,
}

/// Minimum (or, with `maximum`, the largest) distance between the shapes of
/// two references. The maximum is taken over points sampled on both (their
/// vertices, mesh nodes and edge polylines).
pub fn entity_distance(ctx: &Context, a: &Reference, b: &Reference, maximum: bool) -> Option<EntityDistance> {
    let (sa, sb) = (resolve_shapes(ctx, a), resolve_shapes(ctx, b));
    if sa.is_empty() || sb.is_empty() {
        return None;
    }
    if !maximum {
        let mut best: Option<EntityDistance> = None;
        for x in &sa {
            for y in &sb {
                if let Ok(d) = k::distance(x, y)
                    && best.is_none_or(|b| d.distance < b.distance)
                {
                    best = Some(EntityDistance { distance: d.distance, delta: d.point_b - d.point_a });
                }
            }
        }
        return best;
    }
    let (pa, pb) = (sample_points(&sa), sample_points(&sb));
    let mut best: Option<EntityDistance> = None;
    for p in &pa {
        for q in &pb {
            let d = q.distance(*p);
            if best.is_none_or(|b| d > b.distance) {
                best = Some(EntityDistance { distance: d, delta: *q - *p });
            }
        }
    }
    best
}

fn sample_points(shapes: &[Shape]) -> Vec<glam::DVec3> {
    let mut out = Vec::new();
    for s in shapes {
        out.extend(s.sub_shapes(ShapeType::Vertex).iter().filter_map(|v| v.vertex_point().ok()));
        if let Ok(t) = k::tessellate(s, 1e-4, 0.2, false) {
            out.extend(t.positions.iter().map(|p| glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)));
            out.extend(t.edge_points.iter().map(|p| glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)));
        }
    }
    out
}

/// Total length of the edges (and wire bodies) of `refs`.
pub fn total_length(ctx: &Context, refs: &[Reference]) -> f64 {
    refs.iter().flat_map(|r| resolve_shapes(ctx, r)).flat_map(|s| if s.shape_type() == ShapeType::Edge { vec![s] } else { s.sub_shapes(ShapeType::Edge) }).filter_map(|e| e.length().ok()).sum()
}

/// Diameter of a circular edge, cylinder or sphere.
pub fn entity_diameter(ctx: &Context, r: &Reference) -> Option<f64> {
    resolve_shapes(ctx, r).iter().find_map(|s| match s.shape_type() {
        ShapeType::Edge => match s.edge_curve().ok()? {
            k::Curve::Circle { radius, .. } => Some(2.0 * radius),
            _ => None,
        },
        ShapeType::Face => match s.face_surface().ok()? {
            k::Surface::Cylinder { radius, .. } | k::Surface::Sphere { radius, .. } => Some(2.0 * radius),
            _ => None,
        },
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::{BodyId, FeatureId};
    use std::sync::Arc;

    fn body(index: u32, shape: Shape) -> Arc<super::super::context::Body> {
        Arc::new(super::super::context::Body::new(BodyId { feature: FeatureId(1), index }, format!("Part {}", index + 1), crate::display::BodyKind::Solid, shape, Vec::new(), 0, FeatureId(1)))
    }

    /// Two 2×4×6 boxes 10 apart along X (unit density): mass 48 each,
    /// centres (1, 2, 3) and (11, 2, 3), joint centre (6, 2, 3). About it:
    /// Ixx = 2·48(4² + 6²)/12 = 416; Iyy = 2·(48(2² + 6²)/12 + 48·5²) = 2720;
    /// Izz = 2·(48(2² + 4²)/12 + 48·5²) = 2560; no products.
    #[test]
    fn mass_properties_move_each_part_to_the_joint_centre() {
        let a = k::make_box(&k::Plane::xy(), 2.0, 4.0, 6.0).unwrap();
        let b = k::make_box(&k::Plane::new(DVec3::new(10.0, 0.0, 0.0), DVec3::Z, DVec3::X), 2.0, 4.0, 6.0).unwrap();
        let ctx = Context { bodies: vec![body(0, a), body(1, b)], ..Default::default() };
        let m = mass_properties(&ctx, &[], &|_| None).unwrap();
        assert!((m.volume - 96.0).abs() < 1e-6);
        assert!((m.center_of_mass - DVec3::new(6.0, 2.0, 3.0)).length() < 1e-9);
        let i = m.inertia;
        assert!((i.x_axis.x - 416.0).abs() < 1e-6, "Ixx {}", i.x_axis.x);
        assert!((i.y_axis.y - 2720.0).abs() < 1e-6, "Iyy {}", i.y_axis.y);
        assert!((i.z_axis.z - 2560.0).abs() < 1e-6, "Izz {}", i.z_axis.z);
        assert!(i.x_axis.y.abs() < 1e-6 && i.x_axis.z.abs() < 1e-6 && i.y_axis.z.abs() < 1e-6);
        // One part alone: its own inertia (no shift).
        let one = mass_properties(&ctx, &[BodyId { feature: FeatureId(1), index: 0 }], &|_| None).unwrap();
        assert!((one.inertia.y_axis.y - 160.0).abs() < 1e-6);
    }

    /// The Measure tool with nothing selected (or a selection that resolves
    /// to nothing) measures nothing; it panicked on `shapes[0]`.
    /// qa5 AN1: a vertex or an edge with a default plane measures their
    /// distance (the plane is unbounded).
    #[test]
    fn vertex_and_edge_with_a_default_plane() {
        let ctx = Context::default();
        let top = resolve_shapes(&ctx, &Reference::Plane(crate::reference::DatumPlane::Top));
        assert_eq!(top.len(), 1);
        let v = k::make_vertex(DVec3::new(0.3, -0.2, 0.05)).unwrap();
        let rows = measure_shapes(&[vec![v], top.clone()]);
        assert!(!rows.is_empty(), "a result row");
        let e = k::make_line(DVec3::new(0.0, 0.0, 0.02), DVec3::new(0.1, 0.0, 0.02)).unwrap();
        assert!(!measure_shapes(&[vec![e], top]).is_empty());
    }

    /// qa4 round 4 S3-a: a surface body has no mass properties (ours gave a
    /// "volume" for an open loft); with a part beside it only the part counts.
    #[test]
    fn surfaces_have_no_mass_properties() {
        let face = k::make_box(&k::Plane::xy(), 2.0, 4.0, 6.0).unwrap().faces()[0].clone();
        let surf = Arc::new(super::super::context::Body::new(BodyId { feature: FeatureId(2), index: 0 }, "Surface 1".into(), crate::display::BodyKind::Surface, face, Vec::new(), 0, FeatureId(2)));
        let ctx = Context { bodies: vec![surf.clone()], ..Default::default() };
        assert!(mass_properties(&ctx, &[], &|_| None).is_none());
        assert!(mass_properties(&ctx, std::slice::from_ref(&surf.id), &|_| None).is_none());
        let solid = body(0, k::make_box(&k::Plane::xy(), 2.0, 4.0, 6.0).unwrap());
        let ctx = Context { bodies: vec![surf, solid], ..Default::default() };
        assert!((mass_properties(&ctx, &[], &|_| None).unwrap().volume - 48.0).abs() < 1e-9);
    }

    #[test]
    fn empty_selection_measures_nothing() {
        assert!(measure_shapes(&[]).is_empty());
        assert!(measure_shapes(&[Vec::new(), Vec::new()]).is_empty());
    }

    /// Live part-studio/81: two perpendicular faces read "Perp planes
    /// angle: 90.000 deg" (not the total area); opposite faces read the
    /// parallel distance; one face its area.
    #[test]
    fn readout_rules_for_face_pairs() {
        let b = k::make_box(&k::Plane::xy(), 2.0, 4.0, 6.0).unwrap();
        let faces = b.faces();
        let normal = |s: &Shape| match s.face_surface() {
            Ok(k::Surface::Plane(p)) => p.normal.normalize(),
            _ => DVec3::ZERO,
        };
        let x: Vec<&Shape> = faces.iter().filter(|f| normal(f).x.abs() > 0.9).collect();
        let z = faces.iter().find(|f| normal(f).z.abs() > 0.9).unwrap();
        let perp = measure_shapes(&[vec![x[0].clone()], vec![z.clone()]]);
        assert_eq!(perp[0].0, "Perp planes angle");
        assert!(matches!(perp[0].1, Measurement::Angle(t) if (t - std::f64::consts::FRAC_PI_2).abs() < 1e-9));
        assert!(perp.iter().any(|(l, _)| l == "Area") && perp.iter().any(|(l, _)| l == "Min dist"));
        let par = measure_shapes(&[vec![x[0].clone()], vec![x[1].clone()]]);
        assert_eq!(par[0].0, "Parallel dist");
        assert!(matches!(par[0].1, Measurement::Distance { distance, .. } if (distance - 2.0).abs() < 1e-9));
        // Panel order (live): Parallel dist | Area | Min dist | Max dist.
        let labels: Vec<&str> = par.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["Parallel dist", "Area", "Min dist", "Max dist"]);
        assert!(matches!(par[3].1, Measurement::Distance { distance, .. } if (distance - (2.0f64 * 2.0 + 4.0 * 4.0 + 6.0 * 6.0).sqrt()).abs() < 1e-6));
        // The status bar shows the first row only (the Area total is secondary).
        assert_eq!(quick_line(&par, 2).len(), 1);
        // Three edges: only their total length, and nothing in the status bar.
        let edges = z.edges();
        let three = measure_shapes(&[vec![edges[0].clone()], vec![edges[1].clone()], vec![edges[2].clone()]]);
        assert_eq!(three.len(), 1);
        assert_eq!(three[0].0, "Length");
        assert!(quick_line(&three, 3).is_empty());
        // An edge of the top face against a side face it is parallel to.
        let top_edge = z.edges().into_iter().find(|e| matches!(e.edge_curve(), Ok(k::Curve::Line { direction, .. }) if direction.x.abs() < 1e-9)).unwrap();
        let edge_par = measure_shapes(&[vec![top_edge], vec![x[0].clone()]]);
        assert_eq!(edge_par[0].0, "Parallel dist");
        let single = measure_shapes(&[vec![z.clone()]]);
        assert_eq!(single[0].0, "Area");
    }

    /// Live: two Ø8 holes 30 apart read "Parallel axes dist: 30.000 mm  Min
    /// dist: 22.000 mm  Max dist: …" on one line; one hole "Diameter" first,
    /// "Radius" last.
    #[test]
    fn cylinders_measure_axes_and_join_distances_on_the_line() {
        let holes = |x: f64| k::make_cylinder(&k::Plane::new(DVec3::new(x, 0.0, 0.0), DVec3::Z, DVec3::X), 0.004, 0.02, std::f64::consts::TAU).unwrap();
        let wall = |s: &Shape| s.faces().into_iter().find(|f| matches!(f.face_surface(), Ok(k::Surface::Cylinder { .. }))).unwrap();
        let (a, b) = (wall(&holes(0.0)), wall(&holes(0.03)));
        let rows = measure_shapes(&[vec![a.clone()], vec![b]]);
        let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["Parallel axes dist", "Area", "Min dist", "Max dist"]);
        assert!(matches!(rows[0].1, Measurement::Distance { distance, .. } if (distance - 0.03).abs() < 1e-9));
        assert!(matches!(rows[2].1, Measurement::Distance { distance, .. } if (distance - 0.022).abs() < 1e-9));
        let line: Vec<String> = quick_line(&rows, 2).into_iter().map(|(l, _)| l).collect();
        assert_eq!(line, ["Parallel axes dist", "Min dist", "Max dist"]);
        let one = measure_shapes(&[vec![a]]);
        let labels: Vec<&str> = one.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["Diameter", "Area", "Radius"]);
    }
}
