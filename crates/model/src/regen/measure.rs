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
    let mut out = Vec::new();
    let shapes: Vec<Vec<Shape>> = refs.iter().map(|r| resolve_shapes(ctx, r)).collect();
    // Single-entity measurements.
    if refs.len() == 1 {
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
        return out;
    }
    // Totals for multiple edges/faces of the same kind.
    let flat: Vec<&Shape> = shapes.iter().flatten().collect();
    if flat.len() >= 2 && flat.iter().all(|s| s.shape_type() == ShapeType::Edge) {
        let total: f64 = flat.iter().filter_map(|s| s.length().ok()).sum();
        out.push(("Total length".into(), Measurement::Length(total)));
    }
    if flat.len() >= 2 && flat.iter().all(|s| s.shape_type() == ShapeType::Face) {
        let total: f64 = flat.iter().filter_map(|s| s.area().ok()).sum();
        out.push(("Total area".into(), Measurement::Area(total)));
    }
    if refs.len() == 2 && !shapes[0].is_empty() && !shapes[1].is_empty() {
        let a = &shapes[0][0];
        let b = &shapes[1][0];
        if let Ok(d) = k::distance(a, b) {
            let v = d.point_b - d.point_a;
            out.push(("Distance".into(), Measurement::Distance { distance: d.distance, a: d.point_a, b: d.point_b, dx: v.x.abs(), dy: v.y.abs(), dz: v.z.abs() }));
        }
        if let Ok(ang) = k::angle_between(a, b) {
            out.push(("Angle".into(), Measurement::Angle(ang)));
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
    let mut inertia = glam::DMat3::ZERO;
    let mut any = false;
    for b in ctx.bodies.iter().filter(|b| bodies.is_empty() || bodies.contains(&b.id)) {
        let Ok(p) = k::volume_properties(&b.shape) else { continue };
        let Ok(s) = k::surface_properties(&b.shape) else { continue };
        any = true;
        let rho = densities(&b.id);
        if rho.is_none() {
            have_density = false;
        }
        let w = p.mass * rho.unwrap_or(1.0);
        volume += p.mass;
        area += s.mass;
        mass += w;
        com += p.center * w;
        inertia += p.inertia * rho.unwrap_or(1.0);
    }
    if !any {
        return None;
    }
    let total = if mass > 0.0 { mass } else { 1.0 };
    Some(MassProperties {
        volume,
        surface_area: area,
        density: None,
        mass: have_density.then_some(mass),
        center_of_mass: com / total,
        inertia,
    })
}
