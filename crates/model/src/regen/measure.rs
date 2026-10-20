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

/// Distance from `p` to the line through `o` along unit `d`.
fn line_distance(p: DVec3, o: DVec3, d: DVec3) -> f64 {
    let v = p - o;
    (v - d * v.dot(d)).length()
}

/// Measurements of resolved shapes, one list per selected entity, in the
/// order Onshape's quick readout and Measure panel use: the server's
/// measurements reordered like the client (`setMeasurements` in the
/// bundle): the first Diameter / Radius / Parallel (axis) distance or
/// angular measurement (not Curvature deviation) comes first; the readout
/// shows the first row. Pair names as live (part-studio/81, assembly/707:
/// "Perp planes angle"; "Min dist"; "Area" for faces, Help: "Perp axes
/// angle", "Center dist").
pub fn measure_shapes(shapes: &[Vec<Shape>]) -> Vec<(String, Measurement)> {
    let mut out = Vec::new();
    // Single-entity measurements.
    if shapes.len() == 1 {
        for s in &shapes[0] {
            match s.shape_type() {
                ShapeType::Edge => {
                    if let Ok(l) = s.length() {
                        out.push(("Length".into(), Measurement::Length(l)));
                    }
                    if let Ok(k::Curve::Circle { radius, .. }) = s.edge_curve() {
                        out.push(("Radius".into(), Measurement::Radius(radius)));
                        out.push(("Diameter".into(), Measurement::Diameter(radius * 2.0)));
                    }
                }
                ShapeType::Face => {
                    if let Ok(a) = s.area() {
                        out.push(("Area".into(), Measurement::Area(a)));
                    }
                    if let Ok(k::Surface::Cylinder { radius, .. }) = s.face_surface() {
                        out.push(("Radius".into(), Measurement::Radius(radius)));
                        out.push(("Diameter".into(), Measurement::Diameter(radius * 2.0)));
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
        // Client order: Diameter / Radius before Length / Area.
        if let Some(i) = out.iter().position(|(_, m)| matches!(m, Measurement::Radius(_) | Measurement::Diameter(_))) {
            out.swap(0, i);
        }
        return out;
    }
    // Pairs: the priority / angular measurement first.
    if shapes.len() == 2
        && let (Some(a), Some(b)) = (shapes[0].first(), shapes[1].first())
    {
        let angle = || k::angle_between(a, b).ok();
        match (geo(a), geo(b)) {
            (Geo::Plane { origin: o1, normal: n1 }, Geo::Plane { origin: o2, normal: n2 }) => {
                if parallel(n1, n2) {
                    out.push(("Parallel dist".into(), Measurement::Length((o2 - o1).dot(n1).abs())));
                } else if let Some(t) = angle() {
                    out.push(("Perp planes angle".into(), Measurement::Angle(t)));
                }
            }
            (Geo::Axis { origin: o1, dir: d1 }, Geo::Axis { origin: o2, dir: d2 }) => {
                if parallel(d1, d2) {
                    out.push(("Parallel axes dist".into(), Measurement::Length(line_distance(o2, o1, d1))));
                } else {
                    let t = d1.angle_between(d2);
                    out.push(("Perp axes angle".into(), Measurement::Angle(t.min(std::f64::consts::PI - t))));
                }
            }
            (Geo::Line { origin: o1, dir: d1 }, Geo::Line { origin: o2, dir: d2 }) => {
                if parallel(d1, d2) {
                    out.push(("Parallel dist".into(), Measurement::Length(line_distance(o2, o1, d1))));
                } else if let Some(t) = angle() {
                    out.push(("Perp lines angle".into(), Measurement::Angle(t)));
                }
            }
            (Geo::Line { origin: o, dir: d }, Geo::Plane { origin: p, normal: n }) | (Geo::Plane { origin: p, normal: n }, Geo::Line { origin: o, dir: d }) => {
                // A line parallel to the plane: their distance (like parallel planes).
                if d.dot(n).abs() < 1e-9 {
                    out.push(("Parallel dist".into(), Measurement::Length((o - p).dot(n).abs())));
                } else if let Some(t) = angle() {
                    out.push(("Perp line and plane angle".into(), Measurement::Angle(t)));
                }
            }
            _ => {}
        }
    }
    // Totals for edges / faces of the same kind (live: two faces read "Area").
    let flat: Vec<&Shape> = shapes.iter().flatten().collect();
    if flat.len() >= 2 && flat.iter().all(|s| s.shape_type() == ShapeType::Edge) {
        let total: f64 = flat.iter().filter_map(|s| s.length().ok()).sum();
        out.push(("Length".into(), Measurement::Length(total)));
    }
    if flat.len() >= 2 && flat.iter().all(|s| s.shape_type() == ShapeType::Face) {
        let total: f64 = flat.iter().filter_map(|s| s.area().ok()).sum();
        out.push(("Area".into(), Measurement::Area(total)));
    }
    if shapes.len() == 2
        && let (Some(a), Some(b)) = (shapes[0].first(), shapes[1].first())
    {
        if let Ok(d) = k::distance(a, b) {
            let v = d.point_b - d.point_a;
            out.push(("Min dist".into(), Measurement::Distance { distance: d.distance, a: d.point_a, b: d.point_b, dx: v.x.abs(), dy: v.y.abs(), dz: v.z.abs() }));
        }
        let center = |s: &Shape| match (geo(s), s.shape_type()) {
            (Geo::Center(c), _) => Some(c),
            (_, ShapeType::Vertex) => s.vertex_point().ok(),
            _ => None,
        };
        // Center distance needs at least one circle / sphere.
        if (matches!(geo(a), Geo::Center(_)) || matches!(geo(b), Geo::Center(_)))
            && let (Some(p), Some(q)) = (center(a), center(b))
        {
            let v = q - p;
            out.push(("Center dist".into(), Measurement::Distance { distance: v.length(), a: p, b: q, dx: v.x.abs(), dy: v.y.abs(), dz: v.z.abs() }));
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
    for b in ctx.bodies.iter().filter(|b| bodies.is_empty() || bodies.contains(&b.id)) {
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
        assert!(matches!(par[0].1, Measurement::Length(d) if (d - 2.0).abs() < 1e-9));
        // An edge of the top face against a side face it is parallel to.
        let top_edge = z.edges().into_iter().find(|e| matches!(e.edge_curve(), Ok(k::Curve::Line { direction, .. }) if direction.x.abs() < 1e-9)).unwrap();
        let edge_par = measure_shapes(&[vec![top_edge], vec![x[0].clone()]]);
        assert_eq!(edge_par[0].0, "Parallel dist");
        let single = measure_shapes(&[vec![z.clone()]]);
        assert_eq!(single[0].0, "Area");
    }
}
