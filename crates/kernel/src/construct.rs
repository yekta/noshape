//! Construction of vertices, edges, wires and faces (from 2D sketch data or
//! 3D data) and solid primitives.

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::error::{KernelError, Result};
use crate::ffi::ffi;
use crate::geom::{Plane, p3};
use crate::shape::{Shape, ck, list_to_vec, vec_to_list};

/// A 2D sketch segment, in the coordinates of a [`Plane`]. Angles in radians,
/// measured counter-clockwise from the plane X axis (around the plane normal).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Segment2d {
    /// Straight line from `a` to `b`.
    Line { a: DVec2, b: DVec2 },
    /// Circular arc, counter-clockwise from `start_angle` to `end_angle`
    /// (if `end <= start`, 2π is added to `end`).
    Arc { center: DVec2, radius: f64, start_angle: f64, end_angle: f64 },
    /// Circular arc through three points (start, mid, end).
    Arc3 { a: DVec2, mid: DVec2, b: DVec2 },
    /// Full circle.
    Circle { center: DVec2, radius: f64 },
    /// Full ellipse; `rotation` is the angle of the major axis.
    Ellipse { center: DVec2, major_radius: f64, minor_radius: f64, rotation: f64 },
    /// Elliptic arc with parametric angles (counter-clockwise).
    EllipseArc {
        center: DVec2,
        major_radius: f64,
        minor_radius: f64,
        rotation: f64,
        start_angle: f64,
        end_angle: f64,
    },
    /// B-spline (optionally rational / periodic). `knots` are the distinct
    /// knot values with multiplicities `mults` (OCCT convention).
    BSpline {
        poles: Vec<DVec2>,
        weights: Option<Vec<f64>>,
        knots: Vec<f64>,
        mults: Vec<u32>,
        degree: u32,
        periodic: bool,
    },
    /// Interpolating spline through points (GeomAPI_Interpolate).
    Interpolated { points: Vec<DVec2>, periodic: bool },
}

impl Segment2d {
    pub(crate) fn ffi(&self) -> ffi::Seg2d {
        let mut s = ffi::Seg2d {
            kind: 0,
            p: [0.0; 8],
            poles: Vec::new(),
            weights: Vec::new(),
            knots: Vec::new(),
            mults: Vec::new(),
            degree: 0,
            periodic: false,
        };
        match self {
            Segment2d::Line { a, b } => {
                s.kind = 0;
                s.p[..4].copy_from_slice(&[a.x, a.y, b.x, b.y]);
            }
            Segment2d::Arc { center, radius, start_angle, end_angle } => {
                s.kind = 1;
                s.p[..5].copy_from_slice(&[center.x, center.y, *radius, *start_angle, *end_angle]);
            }
            Segment2d::Circle { center, radius } => {
                s.kind = 2;
                s.p[..3].copy_from_slice(&[center.x, center.y, *radius]);
            }
            Segment2d::EllipseArc { center, major_radius, minor_radius, rotation, start_angle, end_angle } => {
                s.kind = 3;
                s.p[..7].copy_from_slice(&[
                    center.x,
                    center.y,
                    *major_radius,
                    *minor_radius,
                    *rotation,
                    *start_angle,
                    *end_angle,
                ]);
            }
            Segment2d::Ellipse { center, major_radius, minor_radius, rotation } => {
                s.kind = 4;
                s.p[..5].copy_from_slice(&[center.x, center.y, *major_radius, *minor_radius, *rotation]);
            }
            Segment2d::BSpline { poles, weights, knots, mults, degree, periodic } => {
                s.kind = 5;
                s.poles = poles.iter().flat_map(|p| [p.x, p.y]).collect();
                s.weights = weights.clone().unwrap_or_default();
                s.knots = knots.clone();
                s.mults = mults.iter().map(|&m| m as i32).collect();
                s.degree = *degree as i32;
                s.periodic = *periodic;
            }
            Segment2d::Arc3 { a, mid, b } => {
                s.kind = 6;
                s.p[..6].copy_from_slice(&[a.x, a.y, mid.x, mid.y, b.x, b.y]);
            }
            Segment2d::Interpolated { points, periodic } => {
                s.kind = 7;
                s.poles = points.iter().flat_map(|p| [p.x, p.y]).collect();
                s.periodic = *periodic;
            }
        }
        s
    }
}

/// Join type for offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum JoinType {
    /// Round (arc) joins.
    #[default]
    Arc,
    /// Sharp (intersection) joins.
    Intersection,
}

impl JoinType {
    pub(crate) fn code(self) -> u8 {
        match self {
            JoinType::Arc => 0,
            JoinType::Intersection => 1,
        }
    }
}

/// A vertex at `p`.
pub fn make_vertex(p: DVec3) -> Result<Shape> {
    Shape::from_res(ffi::make_vertex(p3(p)))
}

/// One edge from a 2D segment on `plane`.
pub fn make_edge_2d(plane: &Plane, seg: &Segment2d) -> Result<Shape> {
    Shape::from_res(ffi::make_edge_2d(&plane.ffi(), &seg.ffi()))
}

/// A wire from connected 2D segments on `plane` (any order, must connect).
pub fn make_wire_2d(plane: &Plane, segs: &[Segment2d]) -> Result<Shape> {
    let v: Vec<ffi::Seg2d> = segs.iter().map(|s| s.ffi()).collect();
    Shape::from_res(ffi::make_wire_2d(&plane.ffi(), &v))
}

/// A wire from edges and/or wires (any order, must connect).
pub fn make_wire(edges: &[Shape]) -> Result<Shape> {
    let l = vec_to_list(edges);
    Shape::from_res(ffi::make_wire(&l))
}

/// A planar face on `plane` bounded by the closed `outer` wire with `holes`.
/// Wire orientations are fixed automatically (outer CCW, holes CW w.r.t. the
/// plane normal), so the face normal equals the plane normal.
pub fn make_face(plane: &Plane, outer: &Shape, holes: &[Shape]) -> Result<Shape> {
    let l = vec_to_list(holes);
    Shape::from_res(ffi::make_face_on_plane(&plane.ffi(), outer.raw(), &l))
}

/// A planar face from 2D loops: `outer` boundary and `holes`.
pub fn make_face_2d(plane: &Plane, outer: &[Segment2d], holes: &[Vec<Segment2d>]) -> Result<Shape> {
    let ow = make_wire_2d(plane, outer)?;
    let hw = holes
        .iter()
        .map(|h| make_wire_2d(plane, h))
        .collect::<Result<Vec<_>>>()?;
    make_face(plane, &ow, &hw)
}

/// A face from a closed wire (planar, or any wire if `only_plane` is false and
/// OCCT can find a supporting surface).
pub fn make_face_from_wire(wire: &Shape, only_plane: bool) -> Result<Shape> {
    Shape::from_res(ffi::make_face_from_wire(wire.raw(), only_plane))
}

/// A rectangular planar face on `plane` covering `[umin, umax] x [vmin, vmax]`.
pub fn make_plane_face(plane: &Plane, umin: f64, umax: f64, vmin: f64, vmax: f64) -> Result<Shape> {
    Shape::from_res(ffi::make_plane_face(&plane.ffi(), umin, umax, vmin, vmax))
}

fn flat(points: &[DVec3]) -> Vec<f64> {
    points.iter().flat_map(|p| [p.x, p.y, p.z]).collect()
}

/// A polyline wire through 3D points (optionally closed).
pub fn make_polyline(points: &[DVec3], closed: bool) -> Result<Shape> {
    Shape::from_res(ffi::make_polyline(&flat(points), closed))
}

/// A straight edge.
pub fn make_line(a: DVec3, b: DVec3) -> Result<Shape> {
    Shape::from_res(ffi::make_line(p3(a), p3(b)))
}

/// A full circle edge (center, axis normal, radius).
pub fn make_circle(center: DVec3, axis: DVec3, radius: f64) -> Result<Shape> {
    let x = Plane::from_normal(center, axis).x_dir;
    Shape::from_res(ffi::make_circle(p3(center), p3(axis), p3(x), radius, 0.0, 0.0, true))
}

/// A circular arc edge in the frame (`center`, `axis`, `x_dir`) from
/// `start_angle` to `end_angle` (counter-clockwise around `axis`).
pub fn make_arc(center: DVec3, axis: DVec3, x_dir: DVec3, radius: f64, start_angle: f64, end_angle: f64) -> Result<Shape> {
    Shape::from_res(ffi::make_circle(p3(center), p3(axis), p3(x_dir), radius, start_angle, end_angle, false))
}

/// A circular arc through three points.
pub fn make_arc_3pts(a: DVec3, mid: DVec3, b: DVec3) -> Result<Shape> {
    Shape::from_res(ffi::make_arc_3pts(p3(a), p3(mid), p3(b)))
}

/// An interpolating B-spline edge through `points` (GeomAPI_Interpolate).
/// `tangents`: empty, `[start, end]`, or one per point (zero vector = free).
pub fn make_interpolated_spline(points: &[DVec3], tangents: &[DVec3], periodic: bool) -> Result<Shape> {
    Shape::from_res(ffi::make_interpolated(&flat(points), &flat(tangents), periodic))
}

/// A 3D B-spline edge from poles / weights / knots / multiplicities.
pub fn make_bspline(
    poles: &[DVec3],
    weights: Option<&[f64]>,
    knots: &[f64],
    mults: &[u32],
    degree: u32,
    periodic: bool,
) -> Result<Shape> {
    let m: Vec<i32> = mults.iter().map(|&v| v as i32).collect();
    Shape::from_res(ffi::make_bspline_3d(
        &flat(poles),
        weights.unwrap_or(&[]),
        knots,
        &m,
        degree as i32,
        periodic,
    ))
}

/// A helix wire around the `frame` Z axis starting at `frame.origin +
/// radius * frame.x_dir`. `taper_angle` (radians) builds a conical helix
/// (positive = radius grows with height). Built as a line on a cylindrical /
/// conical surface, with an approximated 3D B-spline curve.
pub fn make_helix(frame: &Plane, radius: f64, pitch: f64, height: f64, left_handed: bool, taper_angle: f64) -> Result<Shape> {
    Shape::from_res(ffi::make_helix(&frame.ffi(), radius, pitch, height, left_handed, taper_angle))
}

/// Offset a planar wire (or face boundary) in its plane by `distance`
/// (BRepOffsetAPI_MakeOffset). For open wires, `open_result = false` gives a
/// closed contour around the wire (useful for thin features / ribs).
pub fn offset_wire_2d(wire_or_face: &Shape, distance: f64, join: JoinType, open_result: bool) -> Result<Shape> {
    Shape::from_res(ffi::offset_wire_2d(wire_or_face.raw(), distance, join.code(), open_result))
}

/// Connect loose edges into wires (ShapeAnalysis_FreeBounds).
pub fn edges_to_wires(edges: &Shape, tolerance: f64) -> Result<Vec<Shape>> {
    let l = ck(ffi::edges_to_wires(edges.raw(), tolerance))?;
    Ok(list_to_vec(&l))
}

// ---------------------------------------------------------------- primitives

/// Box with one corner at `frame.origin`, extending `dx, dy, dz` along the
/// frame's X, Y and normal axes.
pub fn make_box(frame: &Plane, dx: f64, dy: f64, dz: f64) -> Result<Shape> {
    Shape::from_res(ffi::make_box(&frame.ffi(), dx, dy, dz))
}

/// Axis-aligned box between two corners.
pub fn make_box_corners(a: DVec3, b: DVec3) -> Result<Shape> {
    let min = a.min(b);
    let d = (a - b).abs();
    make_box(&Plane::new(min, DVec3::Z, DVec3::X), d.x, d.y, d.z)
}

/// Cylinder of radius `r` and height `h` along the frame normal; `angle` < 2π
/// gives a sector.
pub fn make_cylinder(frame: &Plane, r: f64, h: f64, angle: f64) -> Result<Shape> {
    Shape::from_res(ffi::make_cylinder(&frame.ffi(), r, h, angle))
}

/// Cone / frustum from radius `r1` (at the base) to `r2` over height `h`.
pub fn make_cone(frame: &Plane, r1: f64, r2: f64, h: f64, angle: f64) -> Result<Shape> {
    Shape::from_res(ffi::make_cone(&frame.ffi(), r1, r2, h, angle))
}

/// Sphere centered at `frame.origin`.
pub fn make_sphere(frame: &Plane, r: f64, angle: f64) -> Result<Shape> {
    Shape::from_res(ffi::make_sphere(&frame.ffi(), r, angle))
}

/// Torus around the frame normal.
pub fn make_torus(frame: &Plane, major_radius: f64, minor_radius: f64, angle: f64) -> Result<Shape> {
    Shape::from_res(ffi::make_torus(&frame.ffi(), major_radius, minor_radius, angle))
}

/// Wedge (BRepPrimAPI_MakeWedge): box `dx, dy, dz` whose top face (at y = dy)
/// has X extent `ltx`.
pub fn make_wedge(frame: &Plane, dx: f64, dy: f64, dz: f64, ltx: f64) -> Result<Shape> {
    Shape::from_res(ffi::make_wedge(&frame.ffi(), dx, dy, dz, ltx))
}

pub(crate) fn require_nonempty<T>(v: &[T], what: &str) -> Result<()> {
    if v.is_empty() {
        return Err(KernelError::invalid(format!("{what} must not be empty")));
    }
    Ok(())
}

/// A wire or face built from 2D segments, with the mapping from each input
/// segment to the output edge it became.
#[derive(Debug, Clone)]
pub struct TrackedProfile {
    /// The wire or face.
    pub shape: Shape,
    /// `segment_edges[i]` = index (in `shape.edges()` MapShapes order) of the
    /// edge created from input segment `i`. For faces, segments are numbered
    /// outer loop first, then each hole loop in order.
    pub segment_edges: Vec<u32>,
}

fn edge_midpoint(e: &Shape) -> Result<DVec3> {
    let i = e.edge_info()?;
    if i.degenerated {
        return Ok(i.start);
    }
    e.point_at(0.5 * (i.first_param + i.last_param))
}

/// Match input edges to edges of `out` by geometry (midpoint + endpoints).
fn match_edges(inputs: &[Shape], out: &Shape) -> Result<Vec<u32>> {
    let outs = out.edges();
    let omid = outs.iter().map(edge_midpoint).collect::<Result<Vec<_>>>()?;
    let mut res = Vec::with_capacity(inputs.len());
    for e in inputs {
        // Fast path: same edge (IsSame) survived.
        if let Some(i) = out.index_of(e, crate::ShapeType::Edge) {
            res.push(i as u32);
            continue;
        }
        let m = edge_midpoint(e)?;
        let (best, _) = omid
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.distance_squared(m)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .ok_or_else(|| KernelError::OperationFailed("profile has no edges".into()))?;
        res.push(best as u32);
    }
    Ok(res)
}

/// Edges for each 2D segment, in input order.
pub fn make_edges_2d(plane: &Plane, segs: &[Segment2d]) -> Result<Vec<Shape>> {
    segs.iter().map(|s| make_edge_2d(plane, s)).collect()
}

/// Like [`make_wire_2d`], also returning which wire edge each segment became.
pub fn make_wire_2d_tracked(plane: &Plane, segs: &[Segment2d]) -> Result<TrackedProfile> {
    let edges = make_edges_2d(plane, segs)?;
    let wire = make_wire(&edges)?;
    let segment_edges = match_edges(&edges, &wire)?;
    Ok(TrackedProfile { shape: wire, segment_edges })
}

/// Like [`make_face_2d`], also returning which face edge each segment became
/// (outer loop segments first, then holes in order).
pub fn make_face_2d_tracked(plane: &Plane, outer: &[Segment2d], holes: &[Vec<Segment2d>]) -> Result<TrackedProfile> {
    let mut all_edges = Vec::new();
    let oe = make_edges_2d(plane, outer)?;
    let ow = make_wire(&oe)?;
    all_edges.extend(oe);
    let mut hws = Vec::new();
    for h in holes {
        let he = make_edges_2d(plane, h)?;
        hws.push(make_wire(&he)?);
        all_edges.extend(he);
    }
    let face = make_face(plane, &ow, &hws)?;
    let segment_edges = match_edges(&all_edges, &face)?;
    Ok(TrackedProfile { shape: face, segment_edges })
}
