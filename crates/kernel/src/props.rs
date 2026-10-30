//! Mass properties and measurement.

use glam::{DMat3, DVec3};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::ffi::ffi;
use crate::geom::v3;
use crate::shape::{Shape, ck};

/// Global properties of a shape (GProp / BRepGProp).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MassProperties {
    /// Volume (solids), area (faces) or length (edges), depending on the query.
    pub mass: f64,
    /// Center of mass (centroid).
    pub center: DVec3,
    /// Inertia tensor about the center of mass (unit density).
    pub inertia: DMat3,
}

fn conv(p: ffi::PropsF) -> MassProperties {
    let m = p.inertia;
    MassProperties {
        mass: p.mass,
        center: v3(p.center),
        // Row-major from C++ -> column-major glam (tensor is symmetric anyway).
        inertia: DMat3::from_cols_array(&m).transpose(),
    }
}

/// Volume properties of solids (mass = volume).
pub fn volume_properties(shape: &Shape) -> Result<MassProperties> {
    Ok(conv(ck(ffi::props_volume(shape.raw()))?))
}

/// [`volume_properties`] with the centre of mass and inertia integrated
/// tighter (the mass properties shown; slower, not cached).
pub fn mass_properties(shape: &Shape) -> Result<MassProperties> {
    Ok(conv(ck(ffi::props_volume_precise(shape.raw()))?))
}

/// Surface properties (mass = area) of faces/shells/solids.
pub fn surface_properties(shape: &Shape) -> Result<MassProperties> {
    Ok(conv(ck(ffi::props_surface(shape.raw()))?))
}

/// Linear properties (mass = length) of edges/wires.
pub fn linear_properties(shape: &Shape) -> Result<MassProperties> {
    Ok(conv(ck(ffi::props_linear(shape.raw()))?))
}

impl Shape {
    /// Volume of the solids in the shape.
    pub fn volume(&self) -> Result<f64> {
        Ok(volume_properties(self)?.mass)
    }
}

/// Minimum distance between two shapes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Distance {
    pub distance: f64,
    /// Closest point on the first shape.
    pub point_a: DVec3,
    /// Closest point on the second shape.
    pub point_b: DVec3,
    /// Number of solutions found (the first is returned).
    pub solutions: u32,
}

/// Minimum distance with closest points (BRepExtrema_DistShapeShape).
pub fn distance(a: &Shape, b: &Shape) -> Result<Distance> {
    let d = ck(ffi::min_distance(a.raw(), b.raw()))?;
    Ok(Distance { distance: d.distance, point_a: v3(d.p1), point_b: v3(d.p2), solutions: d.count.max(0) as u32 })
}

/// Angle (radians, 0..π) between planar faces (normals) and/or linear edges
/// (directions). For a plane and a line, the angle between line and plane.
pub fn angle_between(a: &Shape, b: &Shape) -> Result<f64> {
    ck(ffi::angle_between(a.raw(), b.raw()))
}
