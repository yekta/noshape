//! Feature operations. Each returns an [`OpResult`] with the output shape and
//! its [`History`](crate::History).
//!
//! Sub-shape selections (edges to fillet, faces to remove, ...) are passed as
//! [`Shape`]s that must be sub-shapes of the operated shape (obtain them with
//! [`Shape::sub_shapes`] / [`Shape::sub_shape`]).

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::boolean::{BooleanOp, BooleanOptions, boolean};
use crate::construct::{JoinType, make_polyline, make_face, require_nonempty};
use crate::error::{KernelError, Result};
use crate::ffi::ffi;
use crate::geom::{Plane, p3};
use crate::history::OpResult;
use crate::shape::{Shape, ck, vec_to_list};
use crate::transform::Transform;

fn op(r: std::result::Result<ffi::OpOut, cxx::Exception>) -> Result<OpResult> {
    Ok(OpResult::from_ffi(ck(r)?))
}

// ---------------------------------------------------------------- extrude

/// Extrude (prism) a face, shell, wire, edge or vertex by `vector`.
///
/// History (arg 0 = profile): profile edges `Generated` side faces, profile
/// vertices `Generated` lateral edges, `First`/`Last` give the bottom/top copy
/// of every profile face, edge and vertex.
pub fn extrude(profile: &Shape, vector: DVec3) -> Result<OpResult> {
    extrude_between(profile, DVec3::ZERO, vector, 0.0)
}

/// Extrude from `profile + start` to `profile + end` (translation vectors).
/// Covers starting offsets, both-direction and symmetric extrudes
/// (e.g. `start = -d2 * n`, `end = d1 * n`). `draft_angle` (radians, 0 = none)
/// tapers the side faces with BRepOffsetAPI_DraftAngle using the (translated)
/// profile plane as neutral plane; positive narrows the profile along
/// `end - start`.
pub fn extrude_between(profile: &Shape, start: DVec3, end: DVec3, draft_angle: f64) -> Result<OpResult> {
    op(ffi::op_prism(profile.raw(), p3(start), p3(end), draft_angle))
}

/// Symmetric extrude: total length `|vector|`, centered on the profile.
pub fn extrude_symmetric(profile: &Shape, vector: DVec3) -> Result<OpResult> {
    extrude_between(profile, -vector * 0.5, vector * 0.5, 0.0)
}

/// Extrude "up to" a face, shell or solid along `direction`.
///
/// Method: a through-all prism (length from the combined bounding box) is
/// split by the target with BRepAlgoAPI_Splitter, and the pieces adjacent to
/// the start cap are kept. Fails if the target does not fully bound the
/// profile. `start_offset` moves the profile along `direction` first.
/// For "up to next", pass the body the extrusion should stop at.
///
/// History args: 0 = profile, 1 = target. The end cap faces (pieces of the
/// target) are reported as `Last` of the profile.
pub fn extrude_up_to(profile: &Shape, direction: DVec3, target: &Shape, start_offset: f64) -> Result<OpResult> {
    op(ffi::op_prism_until(profile.raw(), p3(direction), target.raw(), start_offset))
}

/// Extrude up to the plane through `point` perpendicular to `direction`
/// (Onshape "up to vertex"); the profile must be planar.
pub fn extrude_up_to_point(profile: &Shape, direction: DVec3, point: DVec3) -> Result<OpResult> {
    let dir = direction.normalize_or_zero();
    if dir == DVec3::ZERO {
        return Err(KernelError::invalid("extrude direction must be non-zero"));
    }
    let plane = profile
        .find_plane(1e-6)?
        .ok_or_else(|| KernelError::invalid("up-to-vertex extrude requires a planar profile"))?;
    let d = (point - plane.origin).dot(dir);
    if d.abs() < 1e-9 {
        return Err(KernelError::invalid("vertex lies in the profile plane"));
    }
    extrude(profile, dir * d)
}

// ---------------------------------------------------------------- revolve

/// Revolve a profile around the axis (`origin`, `axis`) by `angle` radians
/// (>= 2π: full revolution). History like [`extrude`]: edges generate faces,
/// `First`/`Last` give the start/end copies (not for full revolutions).
pub fn revolve(profile: &Shape, origin: DVec3, axis: DVec3, angle: f64) -> Result<OpResult> {
    revolve_between(profile, origin, axis, 0.0, angle)
}

/// Revolve from `start_angle` to `end_angle` (two-direction / symmetric
/// revolves: e.g. `-a/2 .. a/2`).
pub fn revolve_between(profile: &Shape, origin: DVec3, axis: DVec3, start_angle: f64, end_angle: f64) -> Result<OpResult> {
    op(ffi::op_revol(profile.raw(), p3(origin), p3(axis), start_angle, end_angle))
}

// ---------------------------------------------------------------- sweep

/// Orientation control of the swept profile.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum SweepMode {
    /// Corrected Frenet trihedron (default; minimal twist).
    #[default]
    CorrectedFrenet,
    /// Frenet trihedron.
    Frenet,
    /// Fixed trihedron: the profile keeps its orientation
    /// (Onshape "keep profile orientation"); the frame is (`z`, `x`).
    Fixed { z: DVec3, x: DVec3 },
    /// Constant binormal direction (e.g. "lock profile direction").
    Binormal(DVec3),
    /// Discrete trihedron (robust for paths with C0 joints).
    Discrete,
}

/// Corner treatment at path discontinuities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SweepTransition {
    #[default]
    Transformed,
    RoundCorner,
}

/// Options for [`sweep`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepOptions {
    pub mode: SweepMode,
    /// Make a solid when the profile is closed.
    pub solid: bool,
    pub transition: SweepTransition,
    /// Linear scale law from 1 at the start to `scale_end` at the end
    /// (single profile only).
    pub scale_end: f64,
    /// Translate the profile to the start of the path.
    pub with_contact: bool,
    /// Rotate the profile to be normal to the path at the start.
    pub with_correction: bool,
    /// Twist of the profile about the path over its whole length (radians,
    /// right-handed about the path direction), spread evenly by arc length
    /// on top of the rotation-minimizing frame (Onshape's sweep Twist).
    #[serde(default)]
    pub twist: f64,
}

impl Default for SweepOptions {
    fn default() -> Self {
        SweepOptions {
            mode: SweepMode::CorrectedFrenet,
            solid: true,
            transition: SweepTransition::Transformed,
            scale_end: 1.0,
            with_contact: false,
            with_correction: false,
            twist: 0.0,
        }
    }
}

/// Sweep a profile (face, wire, edge) along a path (wire or edge) with
/// BRepOffsetAPI_MakePipeShell (BRepOffsetAPI_MakePipe for faces with holes).
/// History args: 0 = profile, 1 = path. Profile edges `Generate` side faces;
/// caps are `First`/`Last`.
pub fn sweep(profile: &Shape, path: &Shape, opts: &SweepOptions) -> Result<OpResult> {
    sweep_sections(std::slice::from_ref(profile), path, opts)
}

/// Multi-section sweep (loft along a path): sections are placed along `path`.
/// History args: sections in order, then the path.
pub fn sweep_sections(sections: &[Shape], path: &Shape, opts: &SweepOptions) -> Result<OpResult> {
    require_nonempty(sections, "sweep sections")?;
    let res = sweep_with(sections, path, opts);
    // OCCT's corrected Frenet frame fails on helices; their Frenet frame is
    // already the minimal-twist one (the profile turns with the helix, as in
    // Parasolid: a tooth swept along a helix makes a thread).
    if opts.mode == SweepMode::CorrectedFrenet
        && opts.twist == 0.0
        && !res.as_ref().is_ok_and(|r| r.shape.is_valid())
        && let Ok(r) = sweep_with(sections, path, &SweepOptions { mode: SweepMode::Frenet, ..opts.clone() })
        && r.shape.is_valid()
    {
        return Ok(r);
    }
    res
}

/// Sweep with the profile locked to faces (Onshape "Lock profile faces"): the
/// profile keeps its orientation relative to the faces' normal along the
/// path, which must lie on them. `opts.mode` is ignored. History as [`sweep`].
pub fn sweep_locked_to_faces(sections: &[Shape], path: &Shape, faces: &Shape, opts: &SweepOptions) -> Result<OpResult> {
    require_nonempty(sections, "sweep sections")?;
    op(ffi::op_pipe_shell(&vec_to_list(sections), path.raw(), &sweep_opts(opts, 6), faces.raw()))
}

fn sweep_with(sections: &[Shape], path: &Shape, opts: &SweepOptions) -> Result<OpResult> {
    let (mode, dir, xdir) = match opts.mode {
        SweepMode::CorrectedFrenet => (0, DVec3::Z, DVec3::X),
        SweepMode::Frenet => (1, DVec3::Z, DVec3::X),
        SweepMode::Fixed { z, x } => (2, z, x),
        SweepMode::Binormal(b) => (3, b, DVec3::X),
        SweepMode::Discrete => (5, DVec3::Z, DVec3::X),
    };
    let o = ffi::SweepOpts { dir: p3(dir), xdir: p3(xdir), ..sweep_opts(opts, mode) };
    let l = vec_to_list(sections);
    op(ffi::op_pipe_shell(&l, path.raw(), &o, Shape::null().raw()))
}

fn sweep_opts(opts: &SweepOptions, mode: u8) -> ffi::SweepOpts {
    ffi::SweepOpts {
        mode,
        dir: p3(DVec3::Z),
        xdir: p3(DVec3::X),
        make_solid: opts.solid,
        transition: match opts.transition {
            SweepTransition::Transformed => 1,
            SweepTransition::RoundCorner => 2,
        },
        scale_end: opts.scale_end,
        with_contact: opts.with_contact,
        with_correction: opts.with_correction,
        twist: opts.twist,
    }
}

/// Simple pipe sweep (BRepOffsetAPI_MakePipe, corrected Frenet). Accepts faces
/// with holes. History args: 0 = profile, 1 = path.
pub fn sweep_pipe(profile: &Shape, path: &Shape) -> Result<OpResult> {
    op(ffi::op_pipe(profile.raw(), path.raw()))
}

// ---------------------------------------------------------------- loft

/// Options for [`loft`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LoftOptions {
    /// Make a solid (closed sections) instead of a surface.
    pub solid: bool,
    /// Ruled (straight) between sections instead of smooth.
    pub ruled: bool,
    /// Connect the last section back to the first.
    pub closed: bool,
    /// Optional vertex matching: per section, a point; the section wire is
    /// rebuilt to start at its vertex closest to that point. Use NaN for
    /// sections that should not be changed.
    pub start_points: Option<Vec<DVec3>>,
    /// Several connections (Onshape's Match connections), each one point per
    /// section (NaN for point sections); used instead of `start_points` when
    /// there are two or more. The sections are split at the points and
    /// lofted connection to connection.
    pub connections: Vec<Vec<DVec3>>,
    /// Start / end profile conditions (smooth lofts of wire or face sections).
    pub start: LoftEnd,
    pub end: LoftEnd,
    /// Onshape's Path: the path and its section count (two profiles).
    #[serde(skip)]
    pub path: Option<(Shape, usize)>,
}

/// A loft end condition (Onshape's start / end profile condition) with its
/// magnitude (loft.fs createProfileConditions).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum LoftEnd {
    /// No condition (a natural end).
    #[default]
    Free,
    /// Normal to profile: leaves the profile along its plane's normal.
    Normal(f64),
    /// Tangent to profile: leaves the profile in its plane.
    Tangent(f64),
    /// Tangent to direction: leaves along the direction.
    AlongDirection(f64, DVec3),
    /// Normal to direction: leaves in the plane normal to the direction.
    AcrossDirection(f64, DVec3),
}

impl LoftEnd {
    /// (kind: 1 along, 2 in the plane normal to the vector; magnitude;
    /// vector, zero for the profile's plane normal).
    fn ffi(self) -> [f64; 5] {
        let (k, m, v) = match self {
            LoftEnd::Free => (0.0, 0.0, DVec3::ZERO),
            LoftEnd::Normal(m) => (1.0, m, DVec3::ZERO),
            LoftEnd::Tangent(m) => (2.0, m, DVec3::ZERO),
            LoftEnd::AlongDirection(m, d) => (1.0, m, d),
            LoftEnd::AcrossDirection(m, d) => (2.0, m, d),
        };
        [k, m, v.x, v.y, v.z]
    }
}

/// Loft through sections (faces → outer wire, wires, edges; the first/last
/// may be vertices) with BRepOffsetAPI_ThruSections. History args: sections in
/// order; section edges `Generate` lateral faces; caps are `First`/`Last`.
/// Guide curves are not supported (use [`sweep_sections`] with a path).
pub fn loft(sections: &[Shape], opts: &LoftOptions) -> Result<OpResult> {
    if opts.connections.len() >= 2 && opts.connections.iter().any(|c| c.len() != sections.len()) {
        return Err(KernelError::invalid("one point per loft section is required in each connection"));
    }
    let pts: Vec<f64> = match &opts.start_points {
        _ if opts.connections.len() >= 2 => opts.connections.iter().flatten().flat_map(|v| [v.x, v.y, v.z]).collect(),
        Some(p) => {
            if p.len() != sections.len() {
                return Err(KernelError::invalid("one start point per loft section is required"));
            }
            p.iter().flat_map(|v| [v.x, v.y, v.z]).collect()
        }
        None => Vec::new(),
    };
    let l = vec_to_list(sections);
    let conditions: Vec<f64> = opts.start.ffi().into_iter().chain(opts.end.ffi()).collect();
    let (path, count) = match &opts.path {
        Some((p, n)) => (p.clone(), *n as i32),
        None => (Shape::default(), 0),
    };
    op(ffi::op_loft(&l, opts.solid, opts.ruled, opts.closed, &pts, &conditions, path.raw(), count))
}

// ---------------------------------------------------------------- thicken / shell

/// Thicken a face or shell into a solid: `t1` along the face normal and `t2`
/// on the opposite side (MakeOffsetShape + MakeThickSolidBySimple).
pub fn thicken(shape: &Shape, t1: f64, t2: f64) -> Result<OpResult> {
    op(ffi::op_thicken(shape.raw(), t1, t2))
}

/// Which side of the original surface the shell wall is built on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShellSide {
    /// Keep the outer surface, wall grows inward (Onshape default).
    #[default]
    Inside,
    /// Keep the inner surface, wall grows outward.
    Outside,
}

/// Hollow a solid, removing `faces` (may be empty for a closed hollow solid),
/// with wall `thickness` (BRepOffsetAPI_MakeThickSolid::MakeThickSolidByJoin).
pub fn shell(solid: &Shape, faces: &[Shape], thickness: f64, side: ShellSide, join: JoinType) -> Result<OpResult> {
    if !(thickness > 0.0) {
        return Err(KernelError::invalid("shell thickness must be positive"));
    }
    let off = match side {
        ShellSide::Inside => -thickness,
        ShellSide::Outside => thickness,
    };
    let l = vec_to_list(faces);
    op(ffi::op_shell(solid.raw(), &l, off, join.code()))
}

/// How [`shell_solid`] builds the wall (tests compare the methods).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShellMethod {
    /// Offset first, the boolean construction when that fails.
    #[default]
    Auto,
    /// Only OCCT's offset of the whole solid.
    Offset,
    /// Only the boolean construction (thickened faces + fillers).
    Slabs,
}

/// Hollow a solid like Parasolid's shell: offsets of the kept faces are
/// extended until they meet (faces whose offset vanishes disappear, walls that
/// overlap merge), removed faces leave an opening. `faces` may be empty (closed
/// hollow solid with an inner void). History (argument 0 = `solid`): faces on
/// the original faces are `Modified` from them, offset faces are `Generated` by
/// their face, faces on a removed face (the rim) are `Modified` from it.
pub fn shell_solid(solid: &Shape, faces: &[Shape], thickness: f64, side: ShellSide, method: ShellMethod) -> Result<OpResult> {
    if !(thickness > 0.0) {
        return Err(KernelError::invalid("shell thickness must be positive"));
    }
    let l = vec_to_list(faces);
    let m = match method {
        ShellMethod::Auto => 0,
        ShellMethod::Offset => 1,
        ShellMethod::Slabs => 2,
    };
    op(ffi::op_shell_solid(solid.raw(), &l, thickness, side == ShellSide::Outside, m))
}

/// Scale applied to lengths around OCCT's blend algorithms (the checked fillet and chamfer),
/// process-wide. Their approximation tolerances are absolute and tuned for
/// millimetre-sized parts: the same part modelled in metres fails (a top-loop
/// R4 fillet over R3 corner fillets on a 40 x 20 x 10 mm block). A model whose
/// lengths are metres sets 1000; results come back at the model's scale. Default 1.
pub fn set_working_scale(k: f64) -> Result<()> {
    Ok(ffi::set_working_scale(k)?)
}

// ---------------------------------------------------------------- fillet / chamfer

/// Constant-radius fillet of `edges`. History: each filleted edge `Generated`
/// its blend face(s); adjacent faces are `Modified`.
pub fn fillet(shape: &Shape, edges: &[Shape], radius: f64) -> Result<OpResult> {
    let l = vec_to_list(edges);
    op(ffi::op_fillet(shape.raw(), &l, &[radius], false))
}

/// Fillet with one radius per edge.
pub fn fillet_radii(shape: &Shape, edges: &[Shape], radii: &[f64]) -> Result<OpResult> {
    let l = vec_to_list(edges);
    op(ffi::op_fillet(shape.raw(), &l, radii, false))
}

/// [`fillet_radii`] that always gives a valid solid (or fails): blends that
/// consume a face exactly, where OCCT fails or builds an invalid solid, are
/// retried a hair smaller (1e-7, 1e-6, 1e-5 relative) and the sliver faces left removed.
/// An input that is invalid already keeps OCCT's result. One radius applies to
/// every edge.
pub fn fillet_checked(shape: &Shape, edges: &[Shape], radii: &[f64]) -> Result<OpResult> {
    let l = vec_to_list(edges);
    op(ffi::op_fillet(shape.raw(), &l, radii, true))
}

/// Variable-radius fillet of one edge: `(relative parameter in [0, 1],
/// radius)` pairs, increasing parameters.
pub fn fillet_variable(shape: &Shape, edge: &Shape, law: &[(f64, f64)]) -> Result<OpResult> {
    let params: Vec<f64> = law.iter().map(|p| p.0).collect();
    let radii: Vec<f64> = law.iter().map(|p| p.1).collect();
    op(ffi::op_fillet_variable(shape.raw(), edge.raw(), &params, &radii, false))
}

/// [`fillet_variable`] with [`fillet_checked`]'s guarantees.
pub fn fillet_variable_checked(shape: &Shape, edge: &Shape, law: &[(f64, f64)]) -> Result<OpResult> {
    let params: Vec<f64> = law.iter().map(|p| p.0).collect();
    let radii: Vec<f64> = law.iter().map(|p| p.1).collect();
    op(ffi::op_fillet_variable(shape.raw(), edge.raw(), &params, &radii, true))
}

/// Chamfer specification.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ChamferSpec {
    /// Equal distance on both faces.
    Equal(f64),
    /// Distance `d1` on the reference face, `d2` on the other face.
    TwoDistances(f64, f64),
    /// Distance on the reference face and angle (radians).
    DistanceAngle(f64, f64),
}

/// Chamfer `edges`. `reference_faces` (optional, one per edge) chooses the
/// face that `d1` / the distance is measured on for asymmetric chamfers; by
/// default the first adjacent face of each edge is used.
pub fn chamfer(shape: &Shape, edges: &[Shape], spec: ChamferSpec, reference_faces: &[Shape]) -> Result<OpResult> {
    chamfer_impl(shape, edges, spec, reference_faces, false)
}

/// [`chamfer`] with [`fillet_checked`]'s guarantees. A chamfer wider than a
/// face it runs over (or than the radius of an arc it follows) fails with
/// `OperationFailed("chamfer distance too large")`.
pub fn chamfer_checked(shape: &Shape, edges: &[Shape], spec: ChamferSpec, reference_faces: &[Shape]) -> Result<OpResult> {
    chamfer_impl(shape, edges, spec, reference_faces, true)
}

fn chamfer_impl(shape: &Shape, edges: &[Shape], spec: ChamferSpec, reference_faces: &[Shape], checked: bool) -> Result<OpResult> {
    let (mode, d1, d2) = match spec {
        ChamferSpec::Equal(d) => (0, d, 0.0),
        ChamferSpec::TwoDistances(a, b) => (1, a, b),
        ChamferSpec::DistanceAngle(d, a) => (2, d, a),
    };
    let le = vec_to_list(edges);
    let lf = vec_to_list(reference_faces);
    op(ffi::op_chamfer(shape.raw(), &le, &lf, mode, d1, d2, checked))
}

// ---------------------------------------------------------------- draft / direct edit

/// Draft `faces` by `angle` (radians) relative to `pull_direction`, hinged at
/// their intersection with the `neutral` plane (BRepOffsetAPI_DraftAngle).
pub fn draft(shape: &Shape, faces: &[Shape], pull_direction: DVec3, neutral: &Plane, angle: f64) -> Result<OpResult> {
    let l = vec_to_list(faces);
    op(ffi::op_draft(shape.raw(), &l, p3(pull_direction), &neutral.ffi(), angle))
}

/// [`draft`] built by booleans (`cpp/shell.cpp`), for when OCCT's draft fails or
/// gives an invalid solid: faces parallel to the pull direction move inward by
/// h·tan(angle) (h = height above `neutral`, along the pull), faces whose
/// drafted surface vanishes disappear and their neighbours meet (Parasolid's
/// result, e.g. R3 fillets drafted 30° over 10 mm). Planar faces and cylinder
/// bands only. History like [`draft`]: drafted faces `Modified`.
pub fn draft_wedges(shape: &Shape, faces: &[Shape], pull_direction: DVec3, neutral: &Plane, angle: f64) -> Result<OpResult> {
    let l = vec_to_list(faces);
    op(ffi::op_draft_wedges(shape.raw(), &l, p3(pull_direction), &neutral.ffi(), angle))
}

/// Delete faces and heal the gap by extending neighbors
/// (BRepAlgoAPI_Defeaturing).
pub fn delete_faces(shape: &Shape, faces: &[Shape]) -> Result<OpResult> {
    let l = vec_to_list(faces);
    op(ffi::op_defeature(shape.raw(), &l))
}

/// Offset ("move face" offset mode) the selected faces along their normals by
/// `distance`; neighbors are extended/trimmed (BRepOffset_MakeOffset with
/// per-face offsets). Best effort: fails when topology would change.
pub fn offset_faces(shape: &Shape, faces: &[Shape], distance: f64) -> Result<OpResult> {
    let l = vec_to_list(faces);
    op(ffi::op_offset_faces(shape.raw(), &l, &[distance]))
}

/// Like [`offset_faces`] with one distance per face.
pub fn offset_faces_each(shape: &Shape, faces: &[Shape], distances: &[f64]) -> Result<OpResult> {
    let l = vec_to_list(faces);
    op(ffi::op_offset_faces(shape.raw(), &l, distances))
}

/// Replace face geometry with another surface. Not supported by this kernel
/// (OCCT has no direct equivalent); always returns
/// [`KernelError::Unsupported`].
pub fn replace_face(_shape: &Shape, _faces: &[Shape], _template: &Shape) -> Result<OpResult> {
    Err(KernelError::Unsupported("replace face is not supported by the OCCT kernel".into()))
}

/// Offset a face, shell or solid surface by `distance` (offset surface).
pub fn offset_shape(shape: &Shape, distance: f64, join: JoinType) -> Result<OpResult> {
    op(ffi::op_offset_shape(shape.raw(), distance, join.code()))
}

// ---------------------------------------------------------------- sew / fill / enclose

/// Sew faces/shells into a shell (and a solid if `make_solid`, which requires
/// a closed result). History args: the input shapes in order.
pub fn sew(shapes: &[Shape], tolerance: f64, make_solid: bool) -> Result<OpResult> {
    let l = vec_to_list(shapes);
    op(ffi::op_sew(&l, tolerance, make_solid))
}

/// Boundary continuity for [`fill_surface`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Continuity {
    #[default]
    Position,
    Tangent,
    Curvature,
}

/// A boundary constraint for [`fill_surface`].
#[derive(Debug, Clone)]
pub struct FillEdge {
    pub edge: Shape,
    /// Adjacent face, required for tangent / curvature continuity.
    pub support: Option<Shape>,
    pub continuity: Continuity,
}

/// N-sided fill surface bounded by a closed loop of edges
/// (BRepOffsetAPI_MakeFilling), optionally passing through `points`.
/// History args: the boundary edges in order.
pub fn fill_surface(boundary: &[FillEdge], points: &[DVec3]) -> Result<OpResult> {
    fill_surface_guided(boundary, points, &[])
}

/// [`fill_surface`] that also passes through the guide `curves` (edges).
pub fn fill_surface_guided(boundary: &[FillEdge], points: &[DVec3], curves: &[Shape]) -> Result<OpResult> {
    let edges: Vec<Shape> = boundary.iter().map(|b| b.edge.clone()).collect();
    let supports: Vec<Shape> = boundary.iter().map(|b| b.support.clone().unwrap_or_default()).collect();
    let orders: Vec<i32> = boundary
        .iter()
        .map(|b| match b.continuity {
            Continuity::Position => 0,
            Continuity::Tangent => 1,
            Continuity::Curvature => 2,
        })
        .collect();
    let pts: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
    let le = vec_to_list(&edges);
    let ls = vec_to_list(&supports);
    let lg = vec_to_list(curves);
    op(ffi::op_fill(&le, &ls, &orders, &pts, &lg))
}

/// Build solids from the regions fully enclosed by intersecting faces,
/// shells and solids (BOPAlgo_MakerVolume; Onshape "Enclose").
pub fn enclose(shapes: &[Shape]) -> Result<OpResult> {
    let l = vec_to_list(shapes);
    op(ffi::op_enclose(&l))
}

// ---------------------------------------------------------------- rib / holes

/// Rib from an open profile (wire or edge) lying in `plane` whose ends touch
/// or point into the `base` part: the profile ends are extended along their
/// tangents, the region of the plane enclosed by the extended profile and the
/// part is extruded symmetrically by `thickness` (normal to the plane), and
/// fused with the base ("rib parallel to sketch plane", the classic gusset).
///
/// Method: a large planar face is split by the extended profile, each half is
/// extruded and the base subtracted; pieces that stay away from the big face
/// boundary and touch the profile form the rib body. History args:
/// 0 = base, 1 = rib body (an intermediate shape, not an input).
pub fn rib(base: &Shape, profile: &Shape, plane: &Plane, thickness: f64) -> Result<OpResult> {
    if !(thickness > 0.0) {
        return Err(KernelError::invalid("rib thickness must be positive"));
    }
    let edges = profile.edges();
    require_nonempty(&edges, "rib profile edges")?;
    let bb = base.bounding_box(false)?;
    let big = 4.0 * (bb.diagonal() + profile.bounding_box(false)?.diagonal()) + 1.0;
    // Free ends: vertices used by exactly one edge.
    let ve = profile.vertex_edges()?;
    let verts = profile.vertices();
    let mut pieces: Vec<Shape> = edges.clone();
    let mut ends = 0;
    for (vi, es) in ve.iter().enumerate() {
        if es.len() != 1 {
            continue;
        }
        ends += 1;
        let p = verts[vi].vertex_point()?;
        let e = &edges[es[0] as usize];
        let info = e.edge_info()?;
        let (t, sign) = if p.distance(info.start) <= p.distance(info.end) {
            (info.first_param, -1.0)
        } else {
            (info.last_param, 1.0)
        };
        let mut dir = e.tangent_at(t)? * sign;
        // Tangent is along the edge orientation; make it point away from the edge.
        let probe = e.point_at(0.5 * (info.first_param + info.last_param))?;
        if (probe - p).dot(dir) > 0.0 {
            dir = -dir;
        }
        pieces.push(crate::construct::make_line(p, p + dir * (3.0 * big))?);
    }
    if ends != 2 {
        return Err(KernelError::invalid("rib profile must be an open chain with two ends"));
    }
    let c = plane.to_local(bb.center());
    let half = big;
    let square = crate::construct::make_plane_face(plane, c.x - half, c.x + half, c.y - half, c.y + half)?;
    let halves = crate::boolean::split(&[square], &pieces, &BooleanOptions::default())?.shape;
    let regions = halves.faces();
    if regions.len() < 2 {
        return Err(KernelError::OperationFailed("rib profile does not divide the plane".into()));
    }
    let n = plane.unit_normal() * (thickness * 0.5);
    let mut body = Vec::new();
    for r in &regions {
        let slab = extrude_symmetric(r, n * 2.0)?.shape;
        let rest = crate::boolean::cut(&slab, base)?.shape;
        for piece in rest.solids() {
            let pb = piece.bounding_box(true)?;
            let lo = plane.to_local(pb.min);
            let hi = plane.to_local(pb.max);
            let inside = [lo, hi].iter().all(|q| {
                (q.x - c.x).abs() < half * 0.9 && (q.y - c.y).abs() < half * 0.9
            });
            if !inside {
                continue;
            }
            let d = crate::props::distance(&piece, profile)?.distance;
            if d < 1e-6 * (1.0 + bb.diagonal()) {
                body.push(piece);
            }
        }
    }
    if body.is_empty() {
        return Err(KernelError::OperationFailed(
            "rib profile and part do not enclose a region".into(),
        ));
    }
    let rib_body = if body.len() == 1 { body.remove(0) } else { Shape::compound(&body) };
    boolean(BooleanOp::Fuse, std::slice::from_ref(base), &[rib_body], &BooleanOptions::default())
}

/// OCCT's BRepFeat_MakeLinearForm rib (`fuse = false`: groove). Kept for
/// reference; [`rib`] is more robust. History args: 0 = base, 1 = profile.
pub fn rib_linear_form(base: &Shape, profile: &Shape, plane: &Plane, thickness: f64, fuse: bool) -> Result<OpResult> {
    if !(thickness > 0.0) {
        return Err(KernelError::invalid("rib thickness must be positive"));
    }
    let n = plane.unit_normal() * (thickness * 0.5);
    op(ffi::op_rib(base.raw(), profile.raw(), &plane.ffi(), p3(n), p3(-n), fuse))
}

/// Build a revolved cutting tool (e.g. for holes: counterbore, countersink,
/// tapped drill with tip) from a closed polyline profile given in
/// `(radius, depth)` coordinates, revolved 360° around the `frame` normal.
/// Depth is measured along `-frame.normal` (into the material), so a simple
/// hole of diameter `d` and depth `h` is
/// `[(0,0), (d/2,0), (d/2,h), (0,h)]`.
pub fn revolve_profile_tool(profile: &[DVec2], frame: &Plane) -> Result<Shape> {
    if profile.len() < 3 {
        return Err(KernelError::invalid("hole profile needs at least 3 points"));
    }
    if profile.iter().any(|p| p.x < -1e-12) {
        return Err(KernelError::invalid("hole profile radius must be >= 0"));
    }
    let n = frame.unit_normal();
    let x = frame.unit_x();
    let pts: Vec<DVec3> = profile
        .iter()
        .map(|p| frame.origin + x * p.x - n * p.y)
        .collect();
    let wire = make_polyline(&pts, true)?;
    // Plane containing the profile: spanned by x and n.
    let pplane = Plane::new(frame.origin, x.cross(n), x);
    let face = make_face(&pplane, &wire, &[])?;
    let r = revolve(&face, frame.origin, n, std::f64::consts::TAU)?;
    Ok(r.shape)
}

/// Cut a revolved hole tool (see [`revolve_profile_tool`]) at every frame
/// (hole locations; normal points out of the material) from `target`.
/// History args: 0 = target, 1.. = tools.
pub fn cut_holes(target: &Shape, profile: &[DVec2], frames: &[Plane]) -> Result<OpResult> {
    require_nonempty(frames, "hole locations")?;
    let base = revolve_profile_tool(profile, &Plane::xy())?;
    let tools = frames
        .iter()
        .map(|f| {
            crate::transform::transform(&base, &Transform::frame_to_frame(&Plane::xy(), f), true).map(|r| r.shape)
        })
        .collect::<Result<Vec<_>>>()?;
    boolean(BooleanOp::Cut, std::slice::from_ref(target), &tools, &BooleanOptions::default())
}
