//! Geometric types and geometry queries on faces, edges and vertices.

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::error::{KernelError, Result};
use crate::ffi::ffi;
use crate::shape::{Shape, ck};

pub(crate) fn p3(v: DVec3) -> ffi::P3 {
    ffi::P3 { x: v.x, y: v.y, z: v.z }
}
pub(crate) fn v3(p: ffi::P3) -> DVec3 {
    DVec3::new(p.x, p.y, p.z)
}

/// A plane / right-handed axis system: `origin`, in-plane `x_dir` and
/// `normal` (the local Z axis). `y_dir = normal × x_dir`.
///
/// Used for sketch planes, primitive placement (the primitive's local frame)
/// and section/mirror planes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Plane {
    pub origin: DVec3,
    pub x_dir: DVec3,
    pub normal: DVec3,
}

impl Plane {
    /// Plane through `origin` with the given normal and in-plane X direction.
    pub fn new(origin: DVec3, normal: DVec3, x_dir: DVec3) -> Plane {
        Plane { origin, x_dir, normal }
    }
    /// World XY plane (normal +Z).
    pub fn xy() -> Plane {
        Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X)
    }
    /// World YZ plane (normal +X).
    pub fn yz() -> Plane {
        Plane::new(DVec3::ZERO, DVec3::X, DVec3::Y)
    }
    /// World XZ plane (normal -Y, x = +X, so that y = +Z as in Onshape's "Front").
    pub fn xz() -> Plane {
        Plane::new(DVec3::ZERO, DVec3::NEG_Y, DVec3::X)
    }
    /// Plane through `origin` with `normal`; X direction chosen automatically.
    pub fn from_normal(origin: DVec3, normal: DVec3) -> Plane {
        let n = normal.normalize_or_zero();
        let x = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
        let x = (x - n * x.dot(n)).normalize_or_zero();
        Plane::new(origin, n, x)
    }
    /// Unit normal.
    pub fn unit_normal(&self) -> DVec3 {
        self.normal.normalize_or_zero()
    }
    /// Unit in-plane X direction (orthogonalized against the normal).
    pub fn unit_x(&self) -> DVec3 {
        let n = self.unit_normal();
        (self.x_dir - n * self.x_dir.dot(n)).normalize_or_zero()
    }
    /// Unit in-plane Y direction (`normal × x`).
    pub fn unit_y(&self) -> DVec3 {
        self.unit_normal().cross(self.unit_x())
    }
    /// Map plane coordinates to world coordinates.
    pub fn to_world(&self, p: DVec2) -> DVec3 {
        self.origin + self.unit_x() * p.x + self.unit_y() * p.y
    }
    /// Map a world point to plane coordinates (orthogonal projection).
    pub fn to_local(&self, p: DVec3) -> DVec2 {
        let d = p - self.origin;
        DVec2::new(d.dot(self.unit_x()), d.dot(self.unit_y()))
    }
    /// Signed distance of a point from the plane (along the normal).
    pub fn signed_distance(&self, p: DVec3) -> f64 {
        (p - self.origin).dot(self.unit_normal())
    }
    /// The same plane translated along its normal.
    pub fn offset(&self, d: f64) -> Plane {
        Plane { origin: self.origin + self.unit_normal() * d, ..*self }
    }
    /// The plane with flipped normal (X kept, so Y flips too).
    pub fn flipped(&self) -> Plane {
        Plane { normal: -self.normal, ..*self }
    }
    pub(crate) fn ffi(&self) -> ffi::PlaneF {
        ffi::PlaneF {
            origin: p3(self.origin),
            xdir: p3(self.x_dir),
            normal: p3(self.normal),
        }
    }
    pub(crate) fn from_ffi(p: &ffi::PlaneF) -> Plane {
        Plane::new(v3(p.origin), v3(p.normal), v3(p.xdir))
    }
}

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: DVec3,
    pub max: DVec3,
}

impl BoundingBox {
    /// Box center.
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }
    /// Box size along each axis.
    pub fn size(&self) -> DVec3 {
        self.max - self.min
    }
    /// Diagonal length.
    pub fn diagonal(&self) -> f64 {
        self.size().length()
    }
}

/// Geometry of a face's underlying surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Surface {
    /// Plane; `plane.normal` is the geometric surface normal (not adjusted for
    /// face orientation — see [`Shape::face_plane`] for that).
    Plane(Plane),
    /// Cylinder with axis through `origin` along `axis`.
    Cylinder { origin: DVec3, axis: DVec3, x_dir: DVec3, radius: f64 },
    /// Cone: reference circle of `ref_radius` centered at `origin` in the plane
    /// normal to `axis`; `half_angle` (radians); `apex` point.
    Cone { origin: DVec3, axis: DVec3, x_dir: DVec3, ref_radius: f64, half_angle: f64, apex: DVec3 },
    /// Sphere.
    Sphere { center: DVec3, axis: DVec3, radius: f64 },
    /// Torus.
    Torus { center: DVec3, axis: DVec3, major_radius: f64, minor_radius: f64 },
    /// Bezier surface.
    Bezier { u_degree: i32, v_degree: i32, nb_u_poles: i32, nb_v_poles: i32, rational: bool },
    /// B-spline surface.
    BSpline {
        u_degree: i32,
        v_degree: i32,
        nb_u_poles: i32,
        nb_v_poles: i32,
        rational: bool,
        u_periodic: bool,
        v_periodic: bool,
    },
    /// Surface of revolution around an axis.
    Revolution { axis_origin: DVec3, axis_dir: DVec3 },
    /// Surface of linear extrusion along `direction`.
    Extrusion { direction: DVec3 },
    /// Offset surface.
    Offset { offset: f64 },
    /// Any other surface type.
    Other,
}

/// Geometry of an edge's underlying curve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Curve {
    /// Line; `direction` follows the edge orientation.
    Line { origin: DVec3, direction: DVec3 },
    /// Circle (or arc) in the plane normal to `axis`.
    Circle { center: DVec3, axis: DVec3, x_dir: DVec3, radius: f64 },
    /// Ellipse (or elliptic arc).
    Ellipse { center: DVec3, axis: DVec3, x_dir: DVec3, major_radius: f64, minor_radius: f64 },
    Hyperbola,
    Parabola,
    Bezier { degree: i32, nb_poles: i32, rational: bool },
    BSpline { degree: i32, nb_poles: i32, rational: bool, periodic: bool },
    Offset,
    Other,
}

/// Summary of an edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeInfo {
    pub curve: Curve,
    /// Start point (taking edge orientation into account).
    pub start: DVec3,
    /// End point (taking edge orientation into account).
    pub end: DVec3,
    /// Curve parameter range of the edge.
    pub first_param: f64,
    pub last_param: f64,
    pub length: f64,
    /// Start and end vertices are the same vertex.
    pub closed: bool,
    /// Degenerated edge (e.g. at a sphere pole); has no 3D curve.
    pub degenerated: bool,
}

/// Result of projecting a point onto a face.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FaceProjection {
    pub point: DVec3,
    pub u: f64,
    pub v: f64,
    pub distance: f64,
}

fn require(s: &Shape, kind: crate::ShapeType, what: &str) -> Result<()> {
    if s.shape_type() != kind {
        return Err(KernelError::invalid(format!("{what} must be a {kind:?}")));
    }
    Ok(())
}

impl Shape {
    /// Underlying surface of a face.
    pub fn face_surface(&self) -> Result<Surface> {
        let i = ck(ffi::surface_info(self.raw()))?;
        let (o, a, x) = (v3(i.origin), v3(i.axis), v3(i.xdir));
        Ok(match i.kind {
            0 => Surface::Plane(Plane::new(o, a, x)),
            1 => Surface::Cylinder { origin: o, axis: a, x_dir: x, radius: i.r1 },
            2 => Surface::Cone {
                origin: o,
                axis: a,
                x_dir: x,
                ref_radius: i.r1,
                half_angle: i.angle,
                apex: o + a * i.r2,
            },
            3 => Surface::Sphere { center: o, axis: a, radius: i.r1 },
            4 => Surface::Torus { center: o, axis: a, major_radius: i.r1, minor_radius: i.r2 },
            5 => Surface::Bezier {
                u_degree: i.u_degree,
                v_degree: i.v_degree,
                nb_u_poles: i.nb_u_poles,
                nb_v_poles: i.nb_v_poles,
                rational: i.rational,
            },
            6 => Surface::BSpline {
                u_degree: i.u_degree,
                v_degree: i.v_degree,
                nb_u_poles: i.nb_u_poles,
                nb_v_poles: i.nb_v_poles,
                rational: i.rational,
                u_periodic: i.u_periodic,
                v_periodic: i.v_periodic,
            },
            7 => Surface::Revolution { axis_origin: o, axis_dir: a },
            8 => Surface::Extrusion { direction: a },
            9 => Surface::Offset { offset: i.r1 },
            _ => Surface::Other,
        })
    }

    /// Plane of a planar face with the normal pointing out of the material
    /// (face orientation applied); `None` for non-planar faces.
    pub fn face_plane(&self) -> Result<Option<Plane>> {
        require(self, crate::ShapeType::Face, "shape")?;
        self.find_plane(1e-7)
    }

    /// If the shape (face, wire, edges, compound) lies in a plane, return it.
    pub fn find_plane(&self, tolerance: f64) -> Result<Option<Plane>> {
        let q = ck(ffi::find_plane(self.raw(), tolerance))?;
        Ok(q.found.then(|| Plane::from_ffi(&q.plane)))
    }

    /// True if the shape lies in a plane (faces: planar surface).
    pub fn is_planar(&self) -> bool {
        matches!(self.find_plane(1e-7), Ok(Some(_)))
    }

    /// Outward normal of a face at surface parameters (u, v), accounting for
    /// face orientation.
    pub fn normal_at_uv(&self, u: f64, v: f64) -> Result<DVec3> {
        Ok(v3(ck(ffi::face_normal_at_uv(self.raw(), u, v))?))
    }

    /// Outward normal of a face at the face point closest to `p`.
    pub fn normal_at_point(&self, p: DVec3) -> Result<DVec3> {
        let pr = self.project_point(p)?;
        self.normal_at_uv(pr.u, pr.v)
    }

    /// Surface point at parameters (u, v).
    pub fn point_at_uv(&self, u: f64, v: f64) -> Result<DVec3> {
        Ok(v3(ck(ffi::face_point_at_uv(self.raw(), u, v))?))
    }

    /// Closest point on the (bounded) face to `p`, with its (u, v) parameters.
    pub fn project_point(&self, p: DVec3) -> Result<FaceProjection> {
        let r = ck(ffi::face_project_point(self.raw(), p3(p)))?;
        Ok(FaceProjection { point: v3(r.point), u: r.u, v: r.v, distance: r.distance })
    }

    /// Parameter bounds of a face: `[umin, umax, vmin, vmax]`.
    pub fn uv_bounds(&self) -> Result<[f64; 4]> {
        let v = ck(ffi::face_uv_bounds(self.raw()))?;
        Ok([v[0], v[1], v[2], v[3]])
    }

    /// Area of a face / total area of all faces.
    pub fn area(&self) -> Result<f64> {
        Ok(ck(ffi::props_surface(self.raw()))?.mass)
    }

    /// Area centroid of a face (or set of faces).
    pub fn centroid(&self) -> Result<DVec3> {
        Ok(v3(ck(ffi::props_surface(self.raw()))?.center))
    }

    /// Curve, endpoints, parameter range and length of an edge.
    pub fn edge_info(&self) -> Result<EdgeInfo> {
        let i = ck(ffi::curve_info(self.raw()))?;
        let (o, a, x) = (v3(i.origin), v3(i.axis), v3(i.xdir));
        let curve = match i.kind {
            0 => Curve::Line { origin: o, direction: a },
            1 => Curve::Circle { center: o, axis: a, x_dir: x, radius: i.r1 },
            2 => Curve::Ellipse { center: o, axis: a, x_dir: x, major_radius: i.r1, minor_radius: i.r2 },
            3 => Curve::Hyperbola,
            4 => Curve::Parabola,
            5 => Curve::Bezier { degree: i.degree, nb_poles: i.nb_poles, rational: i.rational },
            6 => Curve::BSpline { degree: i.degree, nb_poles: i.nb_poles, rational: i.rational, periodic: i.periodic },
            7 => Curve::Offset,
            _ => Curve::Other,
        };
        Ok(EdgeInfo {
            curve,
            start: v3(i.start),
            end: v3(i.end),
            first_param: i.first,
            last_param: i.last,
            length: i.length,
            closed: i.closed,
            degenerated: i.degenerated,
        })
    }

    /// Underlying curve of an edge.
    pub fn edge_curve(&self) -> Result<Curve> {
        Ok(self.edge_info()?.curve)
    }

    /// Edge endpoints `(start, end)` (orientation applied).
    pub fn endpoints(&self) -> Result<(DVec3, DVec3)> {
        let i = self.edge_info()?;
        Ok((i.start, i.end))
    }

    /// Length of an edge, or total length of all edges of a wire/compound.
    pub fn length(&self) -> Result<f64> {
        Ok(ck(ffi::props_linear(self.raw()))?.mass)
    }

    /// Point on an edge at curve parameter `t`.
    pub fn point_at(&self, t: f64) -> Result<DVec3> {
        Ok(v3(ck(ffi::edge_point_at(self.raw(), t))?))
    }

    /// Unit tangent of an edge at curve parameter `t` (edge orientation applied).
    pub fn tangent_at(&self, t: f64) -> Result<DVec3> {
        Ok(v3(ck(ffi::edge_tangent_at(self.raw(), t))?))
    }

    /// True if an edge's start and end vertex are the same (closed curve);
    /// for wires, whether the wire is closed.
    pub fn is_closed(&self) -> Result<bool> {
        match self.shape_type() {
            crate::ShapeType::Edge => Ok(self.edge_info()?.closed),
            crate::ShapeType::Wire => {
                // A wire is closed if every vertex is shared by >= 2 edge uses.
                let m = self.ancestor_map(crate::ShapeType::Vertex, crate::ShapeType::Edge)?;
                let edges = self.edges();
                if edges.len() == 1 {
                    return Ok(edges[0].edge_info()?.closed);
                }
                Ok((0..m.len()).all(|i| m.get(i).len() >= 2))
            }
            _ => Err(KernelError::invalid("is_closed requires an edge or wire")),
        }
    }

    /// Point of a vertex.
    pub fn vertex_point(&self) -> Result<DVec3> {
        Ok(v3(ck(ffi::vertex_point(self.raw()))?))
    }

    /// Bounding box; `optimal` uses exact geometry (slower, tight) instead of
    /// the fast enlarged box.
    pub fn bounding_box(&self, optimal: bool) -> Result<BoundingBox> {
        let v = ck(ffi::bounding_box(self.raw(), optimal))?;
        Ok(BoundingBox { min: DVec3::new(v[0], v[1], v[2]), max: DVec3::new(v[3], v[4], v[5]) })
    }

    /// Classify a point against a solid.
    pub fn classify_point(&self, p: DVec3, tolerance: f64) -> Result<PointClass> {
        Ok(match ck(ffi::classify_point(self.raw(), p3(p), tolerance))? {
            1 => PointClass::Inside,
            -1 => PointClass::Outside,
            0 => PointClass::OnBoundary,
            _ => PointClass::Unknown,
        })
    }
}

/// Point-in-solid classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointClass {
    Inside,
    Outside,
    OnBoundary,
    Unknown,
}
