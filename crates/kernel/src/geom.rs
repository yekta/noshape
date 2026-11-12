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

/// See [`Shape::helix_fit`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HelixFit {
    pub origin: DVec3,
    pub axis: DVec3,
    pub x: DVec3,
    pub radius: f64,
    pub pitch: f64,
    pub height: f64,
    pub left_handed: bool,
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

/// Control data of a 3D B-spline curve (see [`Shape::edge_bspline`]).
/// `knots` are the distinct knot values with their `mults`; `weights` is
/// `None` for non-rational curves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BSplineCurveData {
    pub poles: Vec<DVec3>,
    pub weights: Option<Vec<f64>>,
    pub knots: Vec<f64>,
    pub mults: Vec<u32>,
    pub degree: u32,
    pub periodic: bool,
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

    /// Whether `p` lies on the (bounded) face within `tol` (cheaper than [`Shape::project_point`] for a point off it).
    pub fn contains_point(&self, p: DVec3, tol: f64) -> Result<bool> {
        ck(ffi::face_contains_point(self.raw(), p3(p), tol))
    }

    /// Whether the face's parameters (`u`, `v`) are inside it or on its boundary (within `tol`): [`Shape::contains_point`]
    /// for a point whose parameters are known, without projecting it (~1 ms on a dense B-spline).
    pub fn contains_uv(&self, u: f64, v: f64, tol: f64) -> Result<bool> {
        ck(ffi::face_contains_uv(self.raw(), u, v, tol))
    }

    /// Closest point on the (bounded) face to `p`, with its (u, v) parameters.
    pub fn project_point(&self, p: DVec3) -> Result<FaceProjection> {
        let r = ck(ffi::face_project_point(self.raw(), p3(p)))?;
        Ok(FaceProjection { point: v3(r.point), u: r.u, v: r.v, distance: r.distance })
    }

    /// Point, first u derivative and outward normal (the face's orientation) at each surface parameter (u, v), in one
    /// call.
    pub fn frames_at_uvs(&self, uvs: &[DVec2]) -> Result<Vec<[DVec3; 3]>> {
        let flat: Vec<f64> = uvs.iter().flat_map(|p| [p.x, p.y]).collect();
        let r = ck(ffi::face_frames_at_uvs(self.raw(), &flat))?;
        Ok(r.as_chunks::<9>().0.iter().map(|c| [DVec3::new(c[0], c[1], c[2]), DVec3::new(c[3], c[4], c[5]), DVec3::new(c[6], c[7], c[8])]).collect())
    }

    /// First and second derivatives and the outward normal (the face's orientation) at each surface parameter
    /// (u, v): [Su, Sv, Suu, Suv, Svv, N].
    pub fn d2_at_uvs(&self, uvs: &[DVec2]) -> Result<Vec<[DVec3; 6]>> {
        let flat: Vec<f64> = uvs.iter().flat_map(|p| [p.x, p.y]).collect();
        let r = ck(ffi::face_d2_at_uvs(self.raw(), &flat))?;
        Ok(r.as_chunks::<18>().0.iter().map(|c| std::array::from_fn(|k| DVec3::new(c[3 * k], c[3 * k + 1], c[3 * k + 2]))).collect())
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

    /// The edge's curve as an exact B-spline restricted to the edge's range
    /// and following the edge's orientation (lines, conics, Bezier and
    /// B-spline curves are converted with `GeomConvert`). Pass the result to
    /// [`crate::make_bspline_edge`] (possibly with edited poles / weights)
    /// to rebuild the edge.
    pub fn edge_bspline(&self) -> Result<BSplineCurveData> {
        let b = ck(ffi::edge_bspline(self.raw()))?;
        let poles = b.poles.as_chunks::<3>().0.iter().map(|c| DVec3::new(c[0], c[1], c[2])).collect();
        Ok(BSplineCurveData {
            poles,
            weights: if b.weights.is_empty() { None } else { Some(b.weights) },
            knots: b.knots,
            mults: b.mults.into_iter().map(|m| m.max(0) as u32).collect(),
            degree: b.degree.max(0) as u32,
            periodic: b.periodic,
        })
    }

    /// A B-spline face's surface as noshape's NURBS, for many evaluations (each bridge call rebuilds the face's
    /// adaptor: ~0.1 ms on the impeller's 111-pole housing); None for other surfaces and periodic ones.
    pub fn face_nurbs(&self) -> Result<Option<noshape_brep::nurbs::NurbsSurface>> {
        if !matches!(self.face_surface()?, crate::Surface::BSpline { .. }) {
            return Ok(None);
        }
        let b = self.face_bspline()?;
        if b.u_periodic || b.v_periodic {
            return Ok(None);
        }
        Ok(Some(noshape_brep::nurbs::NurbsSurface { udegree: b.u_degree as usize, vdegree: b.v_degree as usize, uknots: b.u_knots, vknots: b.v_knots, nu: b.nu, nv: b.nv, poles: b.poles, weights: b.weights.unwrap_or_default() }))
    }

    /// A face's surface as a B-spline in FeatureScript's form (the
    /// counterpart of [`crate::make_bspline_surface`]): full knot vectors, a
    /// periodic direction with its `degree` overlapping rows or columns.
    pub fn face_bspline(&self) -> Result<crate::BSplineSurfaceData> {
        let b = ck(ffi::face_bspline(self.raw()))?;
        Ok(crate::BSplineSurfaceData {
            poles: b.poles.as_chunks::<3>().0.iter().map(|c| DVec3::new(c[0], c[1], c[2])).collect(),
            nu: b.nu as usize,
            nv: b.nv as usize,
            weights: if b.weights.is_empty() { None } else { Some(b.weights) },
            u_knots: b.u_knots,
            v_knots: b.v_knots,
            u_degree: b.u_degree.max(0) as u32,
            v_degree: b.v_degree.max(0) as u32,
            u_periodic: b.u_periodic,
            v_periodic: b.v_periodic,
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

    /// Point and first, second and third derivatives of an edge's curve at curve parameter `t` (the curve's own
    /// direction, whatever the edge's orientation).
    pub fn derivatives_at(&self, t: f64) -> Result<[DVec3; 4]> {
        let v = ck(ffi::edge_derivatives_at(self.raw(), t))?;
        let d = |i: usize| DVec3::new(v[3 * i], v[3 * i + 1], v[3 * i + 2]);
        Ok([d(0), d(1), d(2), d(3)])
    }

    /// The helix an edge winds along (Onshape's 9° arcs or an exact one), fitted: its start on the axis, the axis the
    /// way it rises, the start's direction from it, radius, pitch, height and whether it turns left about the axis.
    pub fn helix_fit(&self) -> Result<Option<HelixFit>> {
        let v = ck(ffi::edge_helix_fit(self.raw()))?;
        let d = |i: usize| DVec3::new(v[i], v[i + 1], v[i + 2]);
        Ok((v.len() == 13).then(|| HelixFit { origin: d(0), axis: d(3), x: d(6), radius: v[9], pitch: v[10], height: v[11], left_handed: v[12] > 0.5 }))
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

    /// Where the line `p + t dir` (t in `[0, len]`, `dir` normalised) crosses
    /// the shape's faces: `(t, transition, on_boundary)`, the transition +1
    /// into the material, -1 out of it, 0 tangent. Unsorted.
    pub fn line_crossings(&self, p: DVec3, dir: DVec3, len: f64) -> Result<Vec<(f64, i8, bool)>> {
        let v = ck(ffi::line_crossings(self.raw(), p3(p), p3(dir), len))?;
        Ok(v.as_chunks::<3>().0.iter().map(|c| (c[0], c[1] as i8, c[2] != 0.0)).collect())
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
